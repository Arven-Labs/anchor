// CVM, dados abertos de empresas (IPE, Informações Periódicas e Eventuais) —
// porta de `lume/api/app/sources/cvm_ipe.py`. Mesmo desenho do `cvm_dfp`: um zip
// por ano com um CSV `;` em latin-1 de todas as companhias abertas:
// `https://dados.cvm.gov.br/dados/CIA_ABERTA/DOC/IPE/DADOS/ipe_cia_aberta_{ano}.zip`
//
// Diferenças em relação ao DFP:
// - o zip leva o ano de **publicação**, não o fiscal, e cresce o tempo todo
//   (empresas entregam documentos novos); por isso o cache em disco expira em
//   horas no ano corrente, em vez dos 30 dias do DFP;
// - o aviso de dividendos de janeiro pode estar no zip de qualquer um dos dois
//   anos, então lê o ano atual e o anterior.
//
// Esquema conferido ao vivo no arquivo de 2026 (~13 MB, 33 mil linhas):
// `CNPJ_Companhia;Nome_Companhia;Codigo_CVM;Data_Referencia;Categoria;Tipo;
// Especie;Assunto;Data_Entrega;Tipo_Apresentacao;Protocolo_Entrega;Versao;
// Link_Download`. A categoria **"Relatório Proventos"** (481 linhas) é um formulário
// padronizado da CVM, e filtrar só por ela já é preciso (as categorias livres,
// como "Fato Relevante", falam de dividendos só de vez em quando). O CSV não traz
// valor, data de pagamento nem data-com: isso fica no PDF de `Link_Download`.
//
// `Protocolo_Entrega` vem em branco em toda linha dessa categoria (conferido ao
// vivo), então a identidade do aviso é o `Link_Download`, que carrega um
// `numSequencia` único por entrega. `Codigo_CVM` vem com zeros à esquerda
// (`001023`): compara como inteiro.

use std::collections::HashSet;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{Datelike, NaiveDate, Utc};

use crate::error::AppError;
use crate::finance_core::cvm_dfp::latin1;
use crate::finance_core::disk_cache;
use crate::finance_core::provider::{cascade, BoxFuture, DataProvider, Sourced};

pub const SOURCE: &str = "cvm_ipe";
const ZIP_URL: &str = "https://dados.cvm.gov.br/dados/CIA_ABERTA/DOC/IPE/DADOS";
const DIVIDEND_CATEGORY: &str = "Relatório Proventos";

/// O zip do ano corrente ganha linhas o dia todo; o do ano anterior só muda de leve.
const CURRENT_ZIP_MAX_AGE: Duration = Duration::from_secs(12 * 3600);
const PREVIOUS_ZIP_MAX_AGE: Duration = Duration::from_secs(7 * 24 * 3600);

#[derive(Debug, Clone, PartialEq)]
pub struct DividendNotice {
    /// O protocolo da entrega ou, quando vem em branco (o normal nessa categoria), o link.
    pub protocolo_entrega: String,
    pub data_entrega: NaiveDate,
    pub link_download: String,
}

fn unavailable(detail: impl std::fmt::Display) -> AppError {
    AppError::ProviderUnavailable(format!("CVM IPE: {detail}"))
}

/// Avisos de `cvm_code` dentro de um zip (primeiro arquivo do zip).
fn read_notices(zip_path: &Path, cvm_code: i32) -> Result<Vec<DividendNotice>, AppError> {
    let file = File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(BufReader::new(file)).map_err(unavailable)?;
    let entry = archive.by_index(0).map_err(unavailable)?;
    let mut reader = csv::ReaderBuilder::new().delimiter(b';').flexible(true).from_reader(entry);

    let headers = reader.byte_headers().map_err(unavailable)?.clone();
    let required = |name: &str| {
        headers
            .iter()
            .position(|h| h == name.as_bytes())
            .ok_or_else(|| unavailable(format!("missing column {name}")))
    };
    let (c_cvm, c_category, c_delivered, c_protocol, c_link) = (
        required("Codigo_CVM")?,
        required("Categoria")?,
        required("Data_Entrega")?,
        required("Protocolo_Entrega")?,
        required("Link_Download")?,
    );

    let mut notices = Vec::new();
    for record in reader.byte_records() {
        let record = record.map_err(unavailable)?;
        let field = |i: usize| record.get(i).map(latin1).unwrap_or_default();
        if field(c_category) != DIVIDEND_CATEGORY || field(c_cvm).trim().parse::<i32>().ok() != Some(cvm_code) {
            continue;
        }
        let delivered = field(c_delivered);
        let data_entrega = NaiveDate::parse_from_str(delivered.trim(), "%Y-%m-%d")
            .map_err(|e| unavailable(format!("invalid Data_Entrega '{delivered}': {e}")))?;
        let link_download = field(c_link);
        let protocolo = field(c_protocol);
        notices.push(DividendNotice {
            protocolo_entrega: if protocolo.is_empty() { link_download.clone() } else { protocolo },
            data_entrega,
            link_download,
        });
    }
    Ok(notices)
}

// ---------------------------------------------------------------------------
// Download e orquestração
// ---------------------------------------------------------------------------

pub trait ZipFetcher: Send + Sync {
    /// Caminho local do zip publicado no ano `year`; erro se não existe/não baixou.
    fn zip<'a>(&'a self, year: i32) -> BoxFuture<'a, Result<PathBuf, AppError>>;
}

pub struct CvmIpeZips;

impl ZipFetcher for CvmIpeZips {
    fn zip<'a>(&'a self, year: i32) -> BoxFuture<'a, Result<PathBuf, AppError>> {
        Box::pin(async move {
            let name = format!("ipe_cia_aberta_{year}.zip");
            let max_age = if year >= Utc::now().year() { CURRENT_ZIP_MAX_AGE } else { PREVIOUS_ZIP_MAX_AGE };
            disk_cache::get_or_download("CVM IPE", &format!("cvm_ipe/{name}"), &format!("{ZIP_URL}/{name}"), Some(max_age)).await
        })
    }
}

async fn year_notices(fetcher: &dyn ZipFetcher, year: i32, cvm_code: i32) -> Result<Vec<DividendNotice>, AppError> {
    let path = fetcher.zip(year).await?;
    tokio::task::spawn_blocking(move || read_notices(&path, cvm_code))
        .await
        .map_err(|e| unavailable(format!("parsing task failed: {e}")))?
}

/// Avisos do ano `current_year` e do anterior, do mais antigo ao mais novo.
/// Empresa sem aviso é uma lista vazia, não `NoData`. Um ano pode falhar (o
/// anterior costuma bastar), mas se não sobrou nenhuma linha e algum ano falhou,
/// o erro sobe, pra não confundir "a CVM caiu" com "sem avisos".
pub async fn notices_with(
    fetcher: &dyn ZipFetcher,
    cvm_code: i32,
    current_year: i32,
) -> Result<Vec<DividendNotice>, AppError> {
    let mut notices = Vec::new();
    let mut errors = Vec::new();
    for year in [current_year, current_year - 1] {
        match year_notices(fetcher, year, cvm_code).await {
            Ok(found) => notices.extend(found),
            Err(err) => errors.push(err),
        }
    }
    if notices.is_empty() && !errors.is_empty() {
        return Err(errors.remove(0));
    }
    let mut seen = HashSet::new();
    notices.retain(|n| seen.insert(n.protocolo_entrega.clone()));
    notices.sort_by(|a, b| a.data_entrega.cmp(&b.data_entrega).then_with(|| a.protocolo_entrega.cmp(&b.protocolo_entrega)));
    Ok(notices)
}

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------

pub trait DividendNoticeProvider: DataProvider {
    fn dividend_notices<'a>(&'a self, cvm_code: i32) -> BoxFuture<'a, Result<Vec<DividendNotice>, AppError>>;
}

pub struct CvmIpe;

impl DataProvider for CvmIpe {
    fn id(&self) -> &'static str {
        SOURCE
    }
}

impl DividendNoticeProvider for CvmIpe {
    fn dividend_notices<'a>(&'a self, cvm_code: i32) -> BoxFuture<'a, Result<Vec<DividendNotice>, AppError>> {
        Box::pin(async move { notices_with(&CvmIpeZips, cvm_code, Utc::now().year()).await })
    }
}

fn providers() -> Vec<Box<dyn DividendNoticeProvider>> {
    vec![Box::new(CvmIpe)]
}

pub async fn dividend_notices(cvm_code: i32) -> Result<Sourced<Vec<DividendNotice>>, AppError> {
    let providers = providers();
    let attempts = providers.iter().map(|p| (p.id(), p.dividend_notices(cvm_code))).collect();
    cascade(&format!("dividend notices for CVM {cvm_code}"), attempts).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::io::Write;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    const HEADER: &str = "CNPJ_Companhia;Nome_Companhia;Codigo_CVM;Data_Referencia;Categoria;Tipo;Especie;Assunto;Data_Entrega;Tipo_Apresentacao;Protocolo_Entrega;Versao;Link_Download";

    fn row(cvm: &str, category: &str, delivered: &str, protocol: &str, link: &str) -> String {
        format!("00.000.000/0001-91;BANCO DO BRASIL S.A.;{cvm};2026-02-10;{category};;;JCP;{delivered};AP - Apresentação;{protocol};1;{link}")
    }

    fn dividend(cvm: &str, delivered: &str, link: &str) -> String {
        row(cvm, DIVIDEND_CATEGORY, delivered, "", link)
    }

    /// Texto de teste só tem caracteres < U+0100, então codifica 1:1 em latin-1.
    fn csv_bytes(lines: &[String]) -> Vec<u8> {
        let mut all = vec![HEADER.to_string()];
        all.extend_from_slice(lines);
        all.join("\n").chars().map(|c| c as u8).collect()
    }

    fn zip_bytes(name: &str, data: &[u8]) -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(&mut buf);
        writer
            .start_file(name, zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored))
            .unwrap();
        writer.write_all(data).unwrap();
        writer.finish().unwrap();
        buf.into_inner()
    }

    /// Fetcher falso: um zip por ano; ano ausente = falha de download.
    struct Fake {
        zips: HashMap<i32, PathBuf>,
    }

    impl Fake {
        fn new(zips: Vec<(i32, Vec<u8>)>) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "anchor-ipe-test-{}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::SeqCst)
            ));
            std::fs::create_dir_all(&dir).unwrap();
            let zips = zips
                .into_iter()
                .map(|(year, csv)| {
                    let path = dir.join(format!("ipe_cia_aberta_{year}.zip"));
                    std::fs::write(&path, zip_bytes(&format!("ipe_cia_aberta_{year}.csv"), &csv)).unwrap();
                    (year, path)
                })
                .collect();
            Fake { zips }
        }
    }

    impl ZipFetcher for Fake {
        fn zip<'a>(&'a self, year: i32) -> BoxFuture<'a, Result<PathBuf, AppError>> {
            Box::pin(async move {
                self.zips.get(&year).cloned().ok_or_else(|| unavailable(format!("download failed for {year}")))
            })
        }
    }

    const BB: i32 = 1023;
    const LINK_A: &str = "https://www.rad.cvm.gov.br/ENET/frmDownloadDocumento.aspx?numSequencia=1&numProtocolo=1";
    const LINK_B: &str = "https://www.rad.cvm.gov.br/ENET/frmDownloadDocumento.aspx?numSequencia=2&numProtocolo=2";
    const LINK_C: &str = "https://www.rad.cvm.gov.br/ENET/frmDownloadDocumento.aspx?numSequencia=3&numProtocolo=3";

    #[tokio::test]
    async fn keeps_only_dividend_notices_of_the_company() {
        let fake = Fake::new(vec![(
            2026,
            csv_bytes(&[
                dividend("001023", "2026-02-11", LINK_A),
                dividend("009512", "2026-02-12", LINK_B),
                row("001023", "Fato Relevante", "2026-02-13", "999", LINK_C),
            ]),
        )]);
        let notices = notices_with(&fake, BB, 2026).await.unwrap();
        assert_eq!(notices.len(), 1);
        assert_eq!(notices[0].link_download, LINK_A);
        assert_eq!(notices[0].data_entrega, NaiveDate::from_ymd_opt(2026, 2, 11).unwrap());
    }

    #[tokio::test]
    async fn blank_protocol_falls_back_to_the_link() {
        let fake = Fake::new(vec![(2026, csv_bytes(&[dividend("1023", "2026-02-11", LINK_A)]))]);
        assert_eq!(notices_with(&fake, BB, 2026).await.unwrap()[0].protocolo_entrega, LINK_A);
    }

    #[tokio::test]
    async fn filled_protocol_is_kept() {
        let line = row("1023", DIVIDEND_CATEGORY, "2026-02-11", "111", LINK_A);
        let fake = Fake::new(vec![(2026, csv_bytes(&[line]))]);
        assert_eq!(notices_with(&fake, BB, 2026).await.unwrap()[0].protocolo_entrega, "111");
    }

    #[tokio::test]
    async fn merges_both_years_oldest_first() {
        let fake = Fake::new(vec![
            (2026, csv_bytes(&[dividend("1023", "2026-02-11", LINK_B)])),
            (2025, csv_bytes(&[dividend("1023", "2025-12-20", LINK_A)])),
        ]);
        let notices = notices_with(&fake, BB, 2026).await.unwrap();
        let links: Vec<_> = notices.iter().map(|n| n.link_download.as_str()).collect();
        assert_eq!(links, [LINK_A, LINK_B]);
    }

    #[tokio::test]
    async fn the_same_filing_in_both_years_counts_once() {
        let line = dividend("1023", "2026-01-05", LINK_A);
        let fake = Fake::new(vec![(2026, csv_bytes(&[line.clone()])), (2025, csv_bytes(&[line]))]);
        assert_eq!(notices_with(&fake, BB, 2026).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn one_missing_year_is_tolerated() {
        let fake = Fake::new(vec![(2025, csv_bytes(&[dividend("1023", "2025-12-20", LINK_A)]))]);
        assert_eq!(notices_with(&fake, BB, 2026).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn both_years_failing_is_provider_unavailable() {
        let fake = Fake::new(vec![]);
        assert!(matches!(notices_with(&fake, BB, 2026).await, Err(AppError::ProviderUnavailable(_))));
    }

    #[tokio::test]
    async fn company_without_notices_is_an_empty_list() {
        let fake = Fake::new(vec![
            (2026, csv_bytes(&[dividend("9512", "2026-02-11", LINK_A)])),
            (2025, csv_bytes(&[])),
        ]);
        assert!(notices_with(&fake, BB, 2026).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn an_empty_year_next_to_a_failed_one_is_still_an_error() {
        // Sem nenhuma linha e com um ano fora do ar não dá pra afirmar "sem avisos".
        let fake = Fake::new(vec![(2026, csv_bytes(&[]))]);
        assert!(matches!(notices_with(&fake, BB, 2026).await, Err(AppError::ProviderUnavailable(_))));
    }

    #[tokio::test]
    async fn a_missing_column_is_provider_unavailable() {
        let csv = "Codigo_CVM;Categoria\n1023;Relatório Proventos".chars().map(|c| c as u8).collect();
        let fake = Fake::new(vec![(2026, csv), (2025, Vec::new())]);
        let err = notices_with(&fake, BB, 2026).await.unwrap_err();
        assert!(matches!(&err, AppError::ProviderUnavailable(m) if m.contains("missing column")), "{err}");
    }

    #[tokio::test]
    async fn an_invalid_delivery_date_is_provider_unavailable() {
        let fake = Fake::new(vec![(2026, csv_bytes(&[dividend("1023", "11/02/2026", LINK_A)])), (2025, csv_bytes(&[]))]);
        assert!(matches!(notices_with(&fake, BB, 2026).await, Err(AppError::ProviderUnavailable(_))));
    }

    // Ao vivo (rede, ~13 MB): `cargo test --lib -- --ignored --nocapture live_dividend_notices`.
    #[tokio::test]
    #[ignore]
    async fn live_dividend_notices() {
        let notices = dividend_notices(1023).await.unwrap();
        assert_eq!(notices.source, SOURCE);
        println!("Banco do Brasil: {} notice(s), last {:?}", notices.value.len(), notices.value.last());
        assert!(!notices.value.is_empty());
        assert!(notices.value.iter().all(|n| n.link_download.starts_with("https://www.rad.cvm.gov.br/")));
        assert!(notices.value.windows(2).all(|w| w[0].data_entrega <= w[1].data_entrega));
    }
}
