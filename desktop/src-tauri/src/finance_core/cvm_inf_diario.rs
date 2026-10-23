// CVM, informe diário de fundos de investimento (`dados.cvm.gov.br/dados/FI/DOC/INF_DIARIO`): o valor da cota de cada
// fundo em cada dia útil, para o come-cotas e o preço da posição dos fundos (`fund_quota_values`).
//
// Conferido nos arquivos reais (2026-10-23):
// - Um zip por mês (`inf_diario_fi_YYYYMM.zip`, de 202101 em diante, ~7,5 MB e ~35 MB de CSV). Antes de 2021 só há
//   zips anuais em `HIST/`, que não são lidos aqui.
// - CSV `;` com CRLF e dois cabeçalhos: o antigo (`TP_FUNDO;CNPJ_FUNDO;DT_COMPTC;VL_TOTAL;VL_QUOTA;...`) e, desde a
//   Resolução CVM 175, `TP_FUNDO_CLASSE;CNPJ_FUNDO_CLASSE;ID_SUBCLASSE;DT_COMPTC;...`. As colunas são lidas por nome.
// - Um mesmo CNPJ tem uma linha por subclasse em cada dia, cada uma com a sua cota; a linha da classe tem
//   `ID_SUBCLASSE` vazio. Quem chama escolhe a subclasse (ou a classe) por fundo.
// - O mês corrente aparece depois do primeiro dia útil e atrasa 1 a 2 dias úteis; antes disso a CVM responde 404.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{Datelike, NaiveDate, Utc};

use crate::error::AppError;
use crate::finance_core::cvm_fii::normalize_cnpj;
use crate::finance_core::disk_cache;
use crate::finance_core::provider::BoxFuture;

const BASE_URL: &str = "https://dados.cvm.gov.br/dados/FI/DOC/INF_DIARIO/DADOS";
/// O primeiro mês publicado em arquivo mensal.
pub const FIRST_MONTH: (i32, u32) = (2021, 1);

/// O mês corrente e o anterior ainda recebem dias novos e retificações: o zip expira em 12 horas. Os fechados, em 30 dias.
const RECENT_MAX_AGE: Duration = Duration::from_secs(12 * 3600);
const CLOSED_MAX_AGE: Duration = Duration::from_secs(30 * 24 * 3600);

fn unavailable(detail: impl std::fmt::Display) -> AppError {
    AppError::ProviderUnavailable(format!("CVM informe diário: {detail}"))
}

fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

/// O que procurar no informe: um fundo (CNPJ só com dígitos) e a subclasse (`None` = a linha da classe).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub cnpj: String,
    pub subclass: Option<String>,
}

/// O que um mês do informe tem para os `targets` pedidos (na mesma ordem).
#[derive(Debug, Default, PartialEq)]
pub struct Extract {
    /// Por alvo: as cotas positivas do mês (data e valor), na ordem do arquivo.
    pub quotas: Vec<Vec<(NaiveDate, f64)>>,
    /// Por CNPJ: as subclasses (não vazias) que o informe traz no mês.
    pub subclasses: HashMap<String, BTreeSet<String>>,
    /// Os CNPJs que têm alguma linha no mês, com ou sem cota usável.
    pub found: HashSet<String>,
}

/// Os meses `(ano, mês)` de `from` a `to`, sem passar do primeiro mês publicado (`FIRST_MONTH`).
pub fn months(from: NaiveDate, to: NaiveDate) -> Vec<(i32, u32)> {
    let mut current = (from.year(), from.month()).max(FIRST_MONTH);
    let last = (to.year(), to.month());
    let mut out = Vec::new();
    while current <= last {
        out.push(current);
        current = if current.1 == 12 { (current.0 + 1, 1) } else { (current.0, current.1 + 1) };
    }
    out
}

/// O mês corrente ou o anterior a `today`?
pub fn is_recent(month: (i32, u32), today: NaiveDate) -> bool {
    let this = (today.year(), today.month());
    let previous = if this.1 == 1 { (this.0 - 1, 12) } else { (this.0, this.1 - 1) };
    month >= previous
}

/// As cotas de `targets` num zip do informe diário. Lê o CSV em fluxo e guarda só as linhas dos CNPJs pedidos.
pub fn extract(zip_path: &Path, targets: &[Target]) -> Result<Extract, AppError> {
    let mut out = Extract { quotas: vec![Vec::new(); targets.len()], ..Extract::default() };
    if targets.is_empty() {
        return Ok(out);
    }
    let mut by_cnpj: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, target) in targets.iter().enumerate() {
        by_cnpj.entry(target.cnpj.as_str()).or_default().push(i);
    }

    let file = File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(BufReader::new(file)).map_err(unavailable)?;
    let name = archive
        .file_names()
        .find(|n| n.starts_with("inf_diario_fi_") && n.ends_with(".csv"))
        .map(str::to_string)
        .ok_or_else(|| unavailable("no inf_diario_fi_*.csv in the zip"))?;
    let entry = archive.by_name(&name).map_err(unavailable)?;
    let mut reader = csv::ReaderBuilder::new().delimiter(b';').flexible(true).from_reader(entry);

    // Um BOM UTF-8 (EF BB BF) vira "ï»¿" em latin-1 e grudaria na primeira coluna.
    let headers: Vec<String> = reader
        .byte_headers()
        .map_err(unavailable)?
        .iter()
        .map(|h| latin1(h).trim_start_matches("\u{ef}\u{bb}\u{bf}").to_string())
        .collect();
    let column = |names: &[&str]| headers.iter().position(|h| names.contains(&h.as_str()));
    let missing = |what: &str| unavailable(format!("missing column {what}"));
    let cnpj_col = column(&["CNPJ_FUNDO_CLASSE", "CNPJ_FUNDO"]).ok_or_else(|| missing("CNPJ_FUNDO_CLASSE"))?;
    let date_col = column(&["DT_COMPTC"]).ok_or_else(|| missing("DT_COMPTC"))?;
    let quota_col = column(&["VL_QUOTA"]).ok_or_else(|| missing("VL_QUOTA"))?;
    let subclass_col = column(&["ID_SUBCLASSE"]);

    for record in reader.byte_records() {
        let record = record.map_err(unavailable)?;
        let Some(raw_cnpj) = record.get(cnpj_col) else { continue };
        let cnpj = normalize_cnpj(&latin1(raw_cnpj));
        let Some(indices) = by_cnpj.get(cnpj.as_str()) else { continue };

        let subclass = subclass_col.and_then(|c| record.get(c)).map(|s| latin1(s).trim().to_string()).unwrap_or_default();
        if !subclass.is_empty() {
            out.subclasses.entry(cnpj.clone()).or_default().insert(subclass.clone());
        }
        out.found.insert(cnpj);

        let date = record.get(date_col).map(|d| latin1(d)).and_then(|d| NaiveDate::parse_from_str(d.trim(), "%Y-%m-%d").ok());
        let value = record.get(quota_col).map(|v| latin1(v)).and_then(|v| v.trim().parse::<f64>().ok());
        let (Some(date), Some(value)) = (date, value) else { continue };
        if !value.is_finite() || value <= 0.0 {
            continue;
        }
        for &i in indices {
            let wanted = &targets[i].subclass;
            if wanted.as_deref().unwrap_or("") == subclass {
                out.quotas[i].push((date, value));
            }
        }
    }
    Ok(out)
}

pub trait ZipFetcher: Send + Sync {
    fn month<'a>(&'a self, year: i32, month: u32) -> BoxFuture<'a, Result<PathBuf, AppError>>;
}

pub struct CvmDailyZips;

impl ZipFetcher for CvmDailyZips {
    fn month<'a>(&'a self, year: i32, month: u32) -> BoxFuture<'a, Result<PathBuf, AppError>> {
        Box::pin(async move {
            let name = format!("inf_diario_fi_{year}{month:02}.zip");
            let max_age = if is_recent((year, month), Utc::now().date_naive()) { RECENT_MAX_AGE } else { CLOSED_MAX_AGE };
            disk_cache::get_or_download(
                "CVM informe diário",
                &format!("cvm_inf_diario/{name}"),
                &format!("{BASE_URL}/{name}"),
                Some(max_age),
            )
            .await
        })
    }
}

/// Baixa (ou lê do cache) o zip do mês e extrai os `targets` fora da thread assíncrona.
pub async fn extract_month(
    fetcher: &dyn ZipFetcher,
    month: (i32, u32),
    targets: Vec<Target>,
) -> Result<Extract, AppError> {
    let path = fetcher.month(month.0, month.1).await?;
    tokio::task::spawn_blocking(move || extract(&path, &targets))
        .await
        .map_err(|e| unavailable(format!("parsing task failed: {e}")))?
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    const NEW_HEADER: &str =
        "TP_FUNDO_CLASSE;CNPJ_FUNDO_CLASSE;ID_SUBCLASSE;DT_COMPTC;VL_TOTAL;VL_QUOTA;VL_PATRIM_LIQ;CAPTC_DIA;RESG_DIA;NR_COTST";
    const OLD_HEADER: &str = "TP_FUNDO;CNPJ_FUNDO;DT_COMPTC;VL_TOTAL;VL_QUOTA;VL_PATRIM_LIQ;CAPTC_DIA;RESG_DIA;NR_COTST";
    pub(crate) const CLASS_CNPJ: &str = "00.017.024/0001-53";
    pub(crate) const CLASS_DIGITS: &str = "00017024000153";
    pub(crate) const SUBCLASS_DIGITS: &str = "00888897000131";

    /// Linhas copiadas do informe de 2026-10 (`inf_diario_fi_202610.csv`), CRLF como o arquivo real.
    pub(crate) fn new_csv() -> String {
        [
            NEW_HEADER,
            "CLASSES - FIF;00.017.024/0001-53;;2026-10-01;1119426.37;44.990762600000;1221524.87;0.00;2902.97;1",
            "CLASSES - FIF;00.017.024/0001-53;;2026-10-02;1120004.10;45.009590600000;1222036.06;0.00;0.00;1",
            "CLASSES - FIF;00.888.897/0001-31;RBMFN1747320951;2026-10-01;47114895.69;74.383553275471;47091275.92;74000.00;17000.00;1",
            "CLASSES - FIF;00.888.897/0001-31;MZMRC1747322915;2026-10-01;700260258.28;74.040455402933;700168039.98;971319.87;6732106.10;10909",
            "CLASSES - FIF;00.888.897/0001-31;RBMFN1747320951;2026-10-02;47315013.59;74.510349769592;47288549.16;165000.00;48000.00;1",
            "CLASSES - FIF;97.929.213/0001-34;;2026-10-20;70175226.87;15.909504423400;70122822.66;0.00;0.00;7",
            "",
        ]
        .join("\r\n")
    }

    /// Um CSV do informe (cabeçalho atual) com as `rows` dadas, no formato das linhas reais.
    pub(crate) fn csv_of(rows: &[&str]) -> String {
        let mut lines = vec![NEW_HEADER];
        lines.extend_from_slice(rows);
        lines.push("");
        lines.join("\r\n")
    }

    pub(crate) fn zip_bytes(name: &str, csv: &str) -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(&mut buf);
        writer
            .start_file(name, zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored))
            .unwrap();
        writer.write_all(csv.as_bytes()).unwrap();
        writer.finish().unwrap();
        buf.into_inner()
    }

    pub(crate) fn write_zip(csv: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("anchor-inf-diario-test-{}-{}", std::process::id(), COUNTER.fetch_add(1, Ordering::SeqCst)));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("inf_diario_fi_202610.zip");
        std::fs::write(&path, zip_bytes("inf_diario_fi_202610.csv", csv)).unwrap();
        path
    }

    /// Um `ZipFetcher` sem rede: um zip por mês, e mês que não está na lista responde como a CVM (`NoData`, o 404). Com
    /// `fail`, todo mês falha como uma CVM fora do ar.
    pub(crate) struct FakeZips {
        months: HashMap<(i32, u32), PathBuf>,
        fail: bool,
        pub calls: AtomicUsize,
    }

    impl FakeZips {
        pub fn new(months: &[((i32, u32), String)]) -> Self {
            let months = months
                .iter()
                .map(|(month, csv)| {
                    let path = write_zip(csv);
                    (*month, path)
                })
                .collect();
            FakeZips { months, fail: false, calls: AtomicUsize::new(0) }
        }

        pub fn down() -> Self {
            FakeZips { months: HashMap::new(), fail: true, calls: AtomicUsize::new(0) }
        }
    }

    impl ZipFetcher for FakeZips {
        fn month<'a>(&'a self, year: i32, month: u32) -> BoxFuture<'a, Result<PathBuf, AppError>> {
            Box::pin(async move {
                self.calls.fetch_add(1, Ordering::SeqCst);
                if self.fail {
                    return Err(unavailable("download failed"));
                }
                self.months.get(&(year, month)).cloned().ok_or_else(|| AppError::NoData(format!("HTTP 404 for {year}-{month}")))
            })
        }
    }

    fn target(cnpj: &str, subclass: Option<&str>) -> Target {
        Target { cnpj: cnpj.to_string(), subclass: subclass.map(str::to_string) }
    }

    fn d(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, day).unwrap()
    }

    #[test]
    fn the_class_row_is_the_one_without_a_subclass() {
        let got = extract(&write_zip(&new_csv()), &[target(CLASS_DIGITS, None)]).unwrap();
        assert_eq!(got.quotas, vec![vec![(d(1), 44.9907626), (d(2), 45.0095906)]]);
        assert!(got.found.contains(CLASS_DIGITS));
        assert!(got.subclasses.is_empty());
    }

    #[test]
    fn a_chosen_subclass_reads_only_its_own_rows() {
        let got = extract(&write_zip(&new_csv()), &[target(SUBCLASS_DIGITS, Some("RBMFN1747320951"))]).unwrap();
        assert_eq!(got.quotas, vec![vec![(d(1), 74.383553275471), (d(2), 74.510349769592)]]);
    }

    #[test]
    fn a_cnpj_with_only_subclasses_has_no_class_quota_and_lists_the_options() {
        let got = extract(&write_zip(&new_csv()), &[target(SUBCLASS_DIGITS, None)]).unwrap();
        assert_eq!(got.quotas, vec![Vec::new()]);
        assert!(got.found.contains(SUBCLASS_DIGITS));
        let options: Vec<_> = got.subclasses[SUBCLASS_DIGITS].iter().cloned().collect();
        assert_eq!(options, vec!["MZMRC1747322915".to_string(), "RBMFN1747320951".to_string()]);
    }

    #[test]
    fn one_pass_serves_several_funds_and_ignores_the_others() {
        let targets = [target(CLASS_DIGITS, None), target("97929213000134", None), target("99999999000199", None)];
        let got = extract(&write_zip(&new_csv()), &targets).unwrap();
        assert_eq!(got.quotas[0].len(), 2);
        assert_eq!(got.quotas[1], vec![(d(20), 15.9095044234)]);
        assert!(got.quotas[2].is_empty());
        assert!(!got.found.contains("99999999000199"), "um fundo que não está no informe não é 'encontrado'");
    }

    #[test]
    fn the_header_before_resolution_175_is_read_by_column_name() {
        let csv = [
            OLD_HEADER,
            "FI;00.017.024/0001-53;2023-01-02;1119426.37;41.123456000000;1221524.87;0.00;2902.97;1",
            "",
        ]
        .join("\r\n");
        let got = extract(&write_zip(&csv), &[target(CLASS_DIGITS, None)]).unwrap();
        assert_eq!(got.quotas, vec![vec![(NaiveDate::from_ymd_opt(2023, 1, 2).unwrap(), 41.123456)]]);
    }

    #[test]
    fn a_byte_order_mark_does_not_hide_the_first_column() {
        // Cabeçalho antigo: a coluna do CNPJ é a segunda; com o BOM na primeira nada muda. Para o BOM atrapalhar, a
        // coluna lida teria que ser a primeira, então o teste põe o CNPJ nela.
        let csv = [
            "\u{feff}CNPJ_FUNDO_CLASSE;DT_COMPTC;VL_QUOTA",
            "00.017.024/0001-53;2026-10-01;44.99",
            "",
        ]
        .join("\r\n");
        let got = extract(&write_zip(&csv), &[target(CLASS_DIGITS, None)]).unwrap();
        assert_eq!(got.quotas, vec![vec![(d(1), 44.99)]]);
    }

    #[test]
    fn unusable_quotas_are_skipped_but_the_fund_still_counts_as_found() {
        let csv = [
            NEW_HEADER,
            "CLASSES - FIF;00.017.024/0001-53;;2026-10-01;1;0.000000000000;1;0;0;1",
            "CLASSES - FIF;00.017.024/0001-53;;2026-10-02;1;;1;0;0;1",
            "CLASSES - FIF;00.017.024/0001-53;;2026-10-05;1;abc;1;0;0;1",
            "CLASSES - FIF;00.017.024/0001-53;;2026-10-06;1;-3.5;1;0;0;1",
            "CLASSES - FIF;00.017.024/0001-53;;not-a-date;1;12.0;1;0;0;1",
            "CLASSES - FIF;00.017.024/0001-53;;2026-10-07;1;12.5;1;0;0;1",
            "",
        ]
        .join("\r\n");
        let got = extract(&write_zip(&csv), &[target(CLASS_DIGITS, None)]).unwrap();
        assert_eq!(got.quotas, vec![vec![(d(7), 12.5)]]);
        assert!(got.found.contains(CLASS_DIGITS));
    }

    #[test]
    fn a_zip_without_the_expected_columns_or_file_is_provider_unavailable() {
        let no_quota = "TP_FUNDO;CNPJ_FUNDO;DT_COMPTC\r\nFI;00.017.024/0001-53;2023-01-02\r\n";
        let err = extract(&write_zip(no_quota), &[target(CLASS_DIGITS, None)]).unwrap_err();
        assert!(matches!(&err, AppError::ProviderUnavailable(m) if m.contains("VL_QUOTA")), "{err}");

        let dir = std::env::temp_dir().join(format!("anchor-inf-diario-test-other-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("x.zip");
        std::fs::write(&path, zip_bytes("other.csv", "a;b\r\n")).unwrap();
        assert!(matches!(extract(&path, &[target(CLASS_DIGITS, None)]), Err(AppError::ProviderUnavailable(_))));
    }

    #[test]
    fn no_targets_means_nothing_to_read() {
        let got = extract(Path::new("/does/not/exist.zip"), &[]).unwrap();
        assert_eq!(got, Extract::default());
    }

    #[test]
    fn months_cover_the_range_and_never_go_before_the_first_published_month() {
        let day = |y, m| NaiveDate::from_ymd_opt(y, m, 15).unwrap();
        assert_eq!(months(day(2025, 11), day(2026, 2)), vec![(2025, 11), (2025, 12), (2026, 1), (2026, 2)]);
        assert_eq!(months(day(2019, 6), day(2021, 2)), vec![(2021, 1), (2021, 2)]);
        assert_eq!(months(day(2026, 3), day(2026, 3)), vec![(2026, 3)]);
        assert!(months(day(2026, 4), day(2026, 3)).is_empty());
    }

    #[test]
    fn only_the_current_and_the_previous_month_count_as_recent() {
        let today = NaiveDate::from_ymd_opt(2026, 1, 20).unwrap();
        assert!(is_recent((2026, 1), today));
        assert!(is_recent((2025, 12), today));
        assert!(!is_recent((2025, 11), today));
    }

    // Ao vivo (rede): `cargo test --lib -- --ignored --nocapture cvm_inf_diario`.
    #[tokio::test]
    #[ignore]
    async fn live_current_month() {
        let today = Utc::now().date_naive();
        let got = extract_month(&CvmDailyZips, (today.year(), today.month()), vec![target("00017024000153", None)]).await.unwrap();
        println!("{} quotas, last {:?}", got.quotas[0].len(), got.quotas[0].last());
        assert!(got.found.contains("00017024000153"));
    }
}
