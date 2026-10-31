// CVM, dados abertos de fundos imobiliários (FII) — porta de
// `lume/api/app/sources/cvm_fii.py`. Mesmo portal da DFP (`dados.cvm.gov.br`),
// arquivos e esquema próprios (`Data_Referencia`/`Versao`, não `DT_REFER`/`VERSAO`)
// e outra convenção de nome: o zip leva o **ano corrente** dos dados
// (`inf_mensal_fii_2026.zip` já tem os meses de 2026 publicados até agora).
//
// Dois informes, esquema conferido nos arquivos reais:
// - `INF_MENSAL`: patrimônio, valor da cota, dividend yield do mês, nº de
//   cotistas (`inf_mensal_fii_complemento_*.csv`) e o cadastro
//   (`inf_mensal_fii_geral_*.csv`, com `Nome_Fundo_Classe` mas sem ticker).
// - `INF_TRIMESTRAL`: só o arquivo `imovel` (vacância, inadimplência, endereço,
//   área, % locado e % da receita por imóvel).
//
// Preço de mercado e proventos não vêm daqui (a CVM é o regulador, não a bolsa).
// A resolução ticker→CNPJ cruza a bolsai (nome oficial + CNPJ do administrador)
// com o `geral`.

use std::collections::{BTreeSet, HashMap};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{Datelike, NaiveDate, Utc};

use crate::error::AppError;
use crate::finance_core::bolsai::{self, FiiSummary};
use crate::finance_core::disk_cache;
use crate::finance_core::provider::{cascade, BoxFuture, DataProvider, Sourced};

pub const SOURCE: &str = "cvm_fii";
/// Fonte gravada na resolução ticker→CNPJ (cruza a CVM com a bolsai).
pub const RESOLUTION_SOURCE: &str = "cvm_fii+bolsai";
const BASE_URL: &str = "https://dados.cvm.gov.br/dados/FII/DOC";

/// A CVM publica o informe mensal uma vez por mês. O Lume guardava o zip do ano
/// corrente em disco para sempre, então um zip baixado em janeiro nunca recebia
/// os meses seguintes; aqui o do ano corrente expira em 3 dias.
const CURRENT_YEAR_MAX_AGE: Duration = Duration::from_secs(3 * 24 * 3600);
const PAST_YEAR_MAX_AGE: Duration = Duration::from_secs(30 * 24 * 3600);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Report {
    Mensal,
    Trimestral,
}

impl Report {
    fn slug(self) -> &'static str {
        match self {
            Report::Mensal => "mensal",
            Report::Trimestral => "trimestral",
        }
    }
}

type Record = HashMap<String, String>;

fn unavailable(detail: impl std::fmt::Display) -> AppError {
    AppError::ProviderUnavailable(format!("CVM FII: {detail}"))
}

fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

/// Só os dígitos: o CSV traz o CNPJ pontuado e quem chama pode usar qualquer formato.
pub fn normalize_cnpj(cnpj: &str) -> String {
    cnpj.chars().filter(char::is_ascii_digit).collect()
}

fn normalize_name(name: &str) -> String {
    name.split_whitespace().collect::<Vec<_>>().join(" ").to_uppercase()
}

// ---------------------------------------------------------------------------
// Leitura
// ---------------------------------------------------------------------------

/// Registros do arquivo do zip cujo nome começa com `prefix`. Com `cnpj_digits`,
/// só os do fundo (filtrados durante a leitura); sem ele, todos (cadastro `geral`).
fn read_table(zip_path: &Path, prefix: &str, cnpj_digits: Option<&str>) -> Result<Vec<Record>, AppError> {
    let file = File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(BufReader::new(file)).map_err(unavailable)?;
    let name = archive
        .file_names()
        .filter_map(Result::ok)
        .find(|n| n.starts_with(prefix))
        .map(|n| n.into_owned())
        .ok_or_else(|| unavailable(format!("no file '{prefix}*' in the zip")))?;
    let entry = archive.by_name(&name).map_err(unavailable)?;
    let mut reader = csv::ReaderBuilder::new().delimiter(b';').flexible(true).from_reader(entry);
    let headers: Vec<String> = reader.byte_headers().map_err(unavailable)?.iter().map(latin1).collect();
    let cnpj_column = headers.iter().position(|h| h == "CNPJ_Fundo_Classe");

    let mut records = Vec::new();
    for record in reader.byte_records() {
        let record = record.map_err(unavailable)?;
        if let (Some(wanted), Some(column)) = (cnpj_digits, cnpj_column) {
            if normalize_cnpj(&record.get(column).map(latin1).unwrap_or_default()) != wanted {
                continue;
            }
        }
        records.push(headers.iter().cloned().zip(record.iter().map(latin1)).collect());
    }
    Ok(records)
}

fn field<'a>(row: &'a Record, name: &str) -> Result<&'a str, AppError> {
    row.get(name).map(String::as_str).ok_or_else(|| unavailable(format!("missing column {name}")))
}

fn optional_f64(row: &Record, name: &str) -> Result<Option<f64>, AppError> {
    let raw = field(row, name)?.trim();
    if raw.is_empty() {
        return Ok(None);
    }
    raw.parse().map(Some).map_err(|_| unavailable(format!("invalid number in {name}: '{raw}'")))
}

fn required_f64(row: &Record, name: &str) -> Result<f64, AppError> {
    optional_f64(row, name)?.ok_or_else(|| unavailable(format!("empty {name}")))
}

fn date(row: &Record, name: &str) -> Result<NaiveDate, AppError> {
    let raw = field(row, name)?.trim();
    NaiveDate::parse_from_str(raw, "%Y-%m-%d").map_err(|_| unavailable(format!("invalid date in {name}: '{raw}'")))
}

fn version(row: &Record) -> Result<i32, AppError> {
    let raw = field(row, "Versao")?.trim();
    raw.parse().map_err(|_| unavailable(format!("invalid Versao '{raw}'")))
}

/// Retificação (`Versao` maior) para a mesma `Data_Referencia` substitui a
/// anterior, nunca soma nem duplica.
fn latest_version(rows: Vec<Record>) -> Result<Vec<Record>, AppError> {
    let mut max = None;
    for row in &rows {
        let v = version(row)?;
        max = Some(max.map_or(v, |m: i32| m.max(v)));
    }
    let Some(max) = max else { return Ok(Vec::new()) };
    rows.into_iter().filter_map(|r| version(&r).map(|v| (v == max).then_some(r)).transpose()).collect()
}

/// Só as linhas da `Data_Referencia` mais recente (comparação de texto ISO) e,
/// dentro dela, da versão mais alta: 1 linha por fundo no mensal, 1 por imóvel
/// (todas do mesmo trimestre) no trimestral.
fn latest_reference(rows: Vec<Record>) -> Result<Vec<Record>, AppError> {
    let Some(latest) = rows.iter().filter_map(|r| r.get("Data_Referencia")).max().cloned() else {
        return Ok(Vec::new());
    };
    latest_version(rows.into_iter().filter(|r| r.get("Data_Referencia") == Some(&latest)).collect())
}

// ---------------------------------------------------------------------------
// Resultados
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct MonthlyIndicators {
    pub reference_date: NaiveDate,
    pub patrimonio_liquido: f64,
    pub valor_patrimonial_cota: f64,
    pub numero_cotistas: Option<i32>,
    pub dividend_yield_mes: Option<f64>,
    pub rentabilidade_efetiva_mes: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    pub nome_imovel: String,
    pub reference_date: NaiveDate,
    pub endereco: Option<String>,
    pub area_m2: Option<f64>,
    /// Percentuais já vêm como fração (0 a 1) da CVM.
    pub percentual_vacancia: Option<f64>,
    pub percentual_inadimplencia: Option<f64>,
    pub percentual_receitas_fii: Option<f64>,
    pub percentual_locado: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FundResolution {
    /// Só dígitos.
    pub cnpj: String,
    pub fund_name: String,
}

pub fn monthly_of(rows: Vec<Record>) -> Result<Option<MonthlyIndicators>, AppError> {
    let latest = latest_reference(rows)?;
    let Some(row) = latest.first() else { return Ok(None) };
    let cotistas = field(row, "Total_Numero_Cotistas")?.trim();
    Ok(Some(MonthlyIndicators {
        reference_date: date(row, "Data_Referencia")?,
        patrimonio_liquido: required_f64(row, "Patrimonio_Liquido")?,
        valor_patrimonial_cota: required_f64(row, "Valor_Patrimonial_Cotas")?,
        numero_cotistas: if cotistas.is_empty() {
            None
        } else {
            Some(cotistas.parse().map_err(|_| unavailable(format!("invalid Total_Numero_Cotistas '{cotistas}'")))?)
        },
        dividend_yield_mes: optional_f64(row, "Percentual_Dividend_Yield_Mes")?,
        rentabilidade_efetiva_mes: optional_f64(row, "Percentual_Rentabilidade_Efetiva_Mes")?,
    }))
}

pub fn properties_of(rows: Vec<Record>) -> Result<Vec<Property>, AppError> {
    let mut properties = latest_reference(rows)?
        .iter()
        .map(|row| {
            let endereco = field(row, "Endereco")?.trim();
            Ok(Property {
                nome_imovel: field(row, "Nome_Imovel")?.to_string(),
                reference_date: date(row, "Data_Referencia")?,
                endereco: (!endereco.is_empty()).then(|| endereco.to_string()),
                area_m2: optional_f64(row, "Area")?,
                percentual_vacancia: optional_f64(row, "Percentual_Vacancia")?,
                percentual_inadimplencia: optional_f64(row, "Percentual_Inadimplencia")?,
                percentual_receitas_fii: optional_f64(row, "Percentual_Receitas_FII")?,
                percentual_locado: optional_f64(row, "Percentual_Locado")?,
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    properties.sort_by(|a, b| a.nome_imovel.cmp(&b.nome_imovel));
    Ok(properties)
}

/// Casa o fundo exigindo o **administrador** (restringe o universo: um
/// administrador gere dezenas de fundos) **e** o `Nome_Fundo_Classe` (só espaços
/// e caixa normalizados, sem tirar acento). Zero ou mais de um candidato é
/// `NoData`: nunca chuta.
pub fn resolve_of(geral: &[Record], summary: &FiiSummary) -> Result<FundResolution, AppError> {
    let admin = normalize_cnpj(&summary.administrator_cnpj);
    let name = normalize_name(&summary.name);
    let mut candidates = BTreeSet::new();
    for row in geral {
        if normalize_cnpj(field(row, "CNPJ_Administrador")?) == admin
            && normalize_name(field(row, "Nome_Fundo_Classe")?) == name
        {
            candidates.insert(normalize_cnpj(field(row, "CNPJ_Fundo_Classe")?));
        }
    }
    match candidates.len() {
        1 => Ok(FundResolution { cnpj: candidates.into_iter().next().expect("one candidate"), fund_name: summary.name.clone() }),
        0 => Err(AppError::NoData(format!("CVM FII: no fund matches '{}' for administrator {admin}", summary.name))),
        n => Err(AppError::NoData(format!("CVM FII: {n} funds match '{}'; refusing to guess", summary.name))),
    }
}

// ---------------------------------------------------------------------------
// Download e orquestração
// ---------------------------------------------------------------------------

pub trait ZipFetcher: Send + Sync {
    fn zip<'a>(&'a self, report: Report, year: i32) -> BoxFuture<'a, Result<PathBuf, AppError>>;
}

pub struct CvmZips;

impl ZipFetcher for CvmZips {
    fn zip<'a>(&'a self, report: Report, year: i32) -> BoxFuture<'a, Result<PathBuf, AppError>> {
        Box::pin(async move {
            let slug = report.slug();
            let name = format!("inf_{slug}_fii_{year}.zip");
            let url = format!("{BASE_URL}/INF_{}/DADOS/{name}", slug.to_uppercase());
            let max_age = if year >= Utc::now().year() { CURRENT_YEAR_MAX_AGE } else { PAST_YEAR_MAX_AGE };
            disk_cache::get_or_download("CVM FII", &format!("cvm_fii/{name}"), &url, Some(max_age)).await
        })
    }
}

/// Ano corrente primeiro; o anterior só se o do ano corrente não existir ainda
/// (ex.: primeiros dias de janeiro, antes da CVM abrir o zip novo).
async fn resolve_zip(fetcher: &dyn ZipFetcher, report: Report) -> Result<PathBuf, AppError> {
    let year = Utc::now().year();
    match fetcher.zip(report, year).await {
        Ok(path) => Ok(path),
        Err(_) => fetcher.zip(report, year - 1).await,
    }
}

async fn table(path: PathBuf, prefix: &'static str, cnpj_digits: Option<String>) -> Result<Vec<Record>, AppError> {
    tokio::task::spawn_blocking(move || read_table(&path, prefix, cnpj_digits.as_deref()))
        .await
        .map_err(|e| unavailable(format!("parsing task failed: {e}")))?
}

pub async fn monthly_with(fetcher: &dyn ZipFetcher, cnpj: &str) -> Result<MonthlyIndicators, AppError> {
    let digits = normalize_cnpj(cnpj);
    let path = resolve_zip(fetcher, Report::Mensal).await?;
    let rows = table(path, "inf_mensal_fii_complemento_", Some(digits.clone())).await?;
    monthly_of(rows)?.ok_or_else(|| AppError::NoData(format!("CVM FII: no monthly report for CNPJ {digits}")))
}

pub async fn properties_with(fetcher: &dyn ZipFetcher, cnpj: &str) -> Result<Vec<Property>, AppError> {
    let path = resolve_zip(fetcher, Report::Trimestral).await?;
    properties_of(table(path, "inf_trimestral_fii_imovel_", Some(normalize_cnpj(cnpj))).await?)
}

pub async fn resolve_with(fetcher: &dyn ZipFetcher, summary: &FiiSummary) -> Result<FundResolution, AppError> {
    let path = resolve_zip(fetcher, Report::Mensal).await?;
    resolve_of(&table(path, "inf_mensal_fii_geral_", None).await?, summary)
}

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------

pub trait FiiDataProvider: DataProvider {
    fn monthly_indicators<'a>(&'a self, cnpj: &'a str) -> BoxFuture<'a, Result<MonthlyIndicators, AppError>>;
    fn properties<'a>(&'a self, cnpj: &'a str) -> BoxFuture<'a, Result<Vec<Property>, AppError>>;
}

pub struct CvmFii;

impl DataProvider for CvmFii {
    fn id(&self) -> &'static str {
        SOURCE
    }
}

impl FiiDataProvider for CvmFii {
    fn monthly_indicators<'a>(&'a self, cnpj: &'a str) -> BoxFuture<'a, Result<MonthlyIndicators, AppError>> {
        Box::pin(async move { monthly_with(&CvmZips, cnpj).await })
    }
    fn properties<'a>(&'a self, cnpj: &'a str) -> BoxFuture<'a, Result<Vec<Property>, AppError>> {
        Box::pin(async move { properties_with(&CvmZips, cnpj).await })
    }
}

fn providers() -> Vec<Box<dyn FiiDataProvider>> {
    vec![Box::new(CvmFii)]
}

pub async fn monthly_indicators(cnpj: &str) -> Result<Sourced<MonthlyIndicators>, AppError> {
    let providers = providers();
    let attempts = providers.iter().map(|p| (p.id(), p.monthly_indicators(cnpj))).collect();
    cascade(&format!("FII monthly indicators for {cnpj}"), attempts).await
}

pub async fn properties(cnpj: &str) -> Result<Sourced<Vec<Property>>, AppError> {
    let providers = providers();
    let attempts = providers.iter().map(|p| (p.id(), p.properties(cnpj))).collect();
    cascade(&format!("FII properties for {cnpj}"), attempts).await
}

/// Ticker → CNPJ do fundo: resumo da bolsai e casamento com o cadastro da CVM.
pub async fn resolve_cnpj(ticker: &str) -> Result<Sourced<FundResolution>, AppError> {
    let summary = bolsai::fii_summary(ticker).await?;
    let resolution = resolve_with(&CvmZips, &summary.value).await?;
    Ok(Sourced { value: resolution, source: RESOLUTION_SOURCE })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    const CNPJ_PUNCTUATED: &str = "11.728.688/0001-47";
    const CNPJ_DIGITS: &str = "11728688000147";
    const ADMIN_PUNCTUATED: &str = "27.809.513/0001-30";
    const FUND_NAME: &str = "CSHG LOGISTICA FUNDO DE INVESTIMENTO IMOBILIARIO";

    fn year() -> i32 {
        Utc::now().year()
    }

    /// CSV latin-1 a partir de linhas de pares coluna=valor (colunas na ordem do cabeçalho).
    fn csv_file(name: &str, header: &[&str], rows: &[Vec<(&str, &str)>]) -> (String, Vec<u8>) {
        let mut lines = vec![header.join(";")];
        for row in rows {
            let cells: Vec<&str> =
                header.iter().map(|h| row.iter().find(|(k, _)| k == h).map(|(_, v)| *v).unwrap_or("")).collect();
            lines.push(cells.join(";"));
        }
        (name.to_string(), lines.join("\n").chars().map(|c| c as u8).collect())
    }

    fn zip_bytes(files: &[(String, Vec<u8>)]) -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(&mut buf);
        for (name, data) in files {
            writer
                .start_file(name, zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored))
                .unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap();
        buf.into_inner()
    }

    struct Fake {
        zips: HashMap<(Report, i32), PathBuf>,
    }

    impl Fake {
        fn new(zips: Vec<(Report, i32, Vec<(String, Vec<u8>)>)>) -> Self {
            let dir = std::env::temp_dir()
                .join(format!("anchor-fii-test-{}-{}", std::process::id(), COUNTER.fetch_add(1, Ordering::SeqCst)));
            std::fs::create_dir_all(&dir).unwrap();
            let zips = zips
                .into_iter()
                .map(|(report, year, files)| {
                    let path = dir.join(format!("{}_{year}.zip", report.slug()));
                    std::fs::write(&path, zip_bytes(&files)).unwrap();
                    ((report, year), path)
                })
                .collect();
            Fake { zips }
        }
    }

    impl ZipFetcher for Fake {
        fn zip<'a>(&'a self, report: Report, year: i32) -> BoxFuture<'a, Result<PathBuf, AppError>> {
            Box::pin(async move {
                self.zips.get(&(report, year)).cloned().ok_or_else(|| unavailable(format!("download failed for {year}")))
            })
        }
    }

    const COMPLEMENTO: [&str; 8] = [
        "CNPJ_Fundo_Classe", "Data_Referencia", "Versao", "Total_Numero_Cotistas", "Patrimonio_Liquido",
        "Valor_Patrimonial_Cotas", "Percentual_Rentabilidade_Efetiva_Mes", "Percentual_Dividend_Yield_Mes",
    ];
    const IMOVEL: [&str; 10] = [
        "CNPJ_Fundo_Classe", "Data_Referencia", "Versao", "Nome_Imovel", "Endereco", "Area",
        "Percentual_Vacancia", "Percentual_Inadimplencia", "Percentual_Receitas_FII", "Percentual_Locado",
    ];
    const GERAL: [&str; 3] = ["CNPJ_Fundo_Classe", "CNPJ_Administrador", "Nome_Fundo_Classe"];

    fn mensal_fake() -> Fake {
        let file = csv_file(
            &format!("inf_mensal_fii_complemento_{}.csv", year()),
            &COMPLEMENTO,
            &[
                vec![("CNPJ_Fundo_Classe", CNPJ_PUNCTUATED), ("Data_Referencia", "2026-01-01"), ("Versao", "1"),
                     ("Total_Numero_Cotistas", "3639"), ("Patrimonio_Liquido", "258202136.67"),
                     ("Valor_Patrimonial_Cotas", "92.2101419138767"), ("Percentual_Rentabilidade_Efetiva_Mes", "0.005242"),
                     ("Percentual_Dividend_Yield_Mes", "0.004342")],
                vec![("CNPJ_Fundo_Classe", CNPJ_PUNCTUATED), ("Data_Referencia", "2026-02-01"), ("Versao", "1"),
                     ("Total_Numero_Cotistas", "3578"), ("Patrimonio_Liquido", "258340696.87"),
                     ("Valor_Patrimonial_Cotas", "92.2596250663804"), ("Percentual_Rentabilidade_Efetiva_Mes", "0.004874"),
                     ("Percentual_Dividend_Yield_Mes", "0.004338")],
            ],
        );
        Fake::new(vec![(Report::Mensal, year(), vec![file])])
    }

    #[tokio::test]
    async fn monthly_picks_the_latest_reference_date() {
        let m = monthly_with(&mensal_fake(), CNPJ_PUNCTUATED).await.unwrap();
        assert_eq!(m.reference_date, NaiveDate::from_ymd_opt(2026, 2, 1).unwrap());
        assert_eq!(m.patrimonio_liquido, 258340696.87);
        assert_eq!(m.valor_patrimonial_cota, 92.2596250663804);
        assert_eq!(m.numero_cotistas, Some(3578));
        assert_eq!(m.dividend_yield_mes, Some(0.004338));
        assert_eq!(m.rentabilidade_efetiva_mes, Some(0.004874));
    }

    #[tokio::test]
    async fn monthly_accepts_a_digits_only_cnpj_and_rejects_unknown_funds() {
        assert!(monthly_with(&mensal_fake(), CNPJ_DIGITS).await.is_ok());
        assert!(matches!(monthly_with(&mensal_fake(), "99999999000199").await, Err(AppError::NoData(_))));
    }

    #[tokio::test]
    async fn monthly_falls_back_to_last_year_and_fails_when_neither_exists() {
        assert_eq!(normalize_cnpj(CNPJ_PUNCTUATED), CNPJ_DIGITS);
        assert!(matches!(monthly_with(&Fake::new(vec![]), CNPJ_DIGITS).await, Err(AppError::ProviderUnavailable(_))));
        let file = csv_file(
            &format!("inf_mensal_fii_complemento_{}.csv", year() - 1),
            &COMPLEMENTO,
            &[vec![("CNPJ_Fundo_Classe", CNPJ_PUNCTUATED), ("Data_Referencia", "2025-12-01"), ("Versao", "1"),
                   ("Patrimonio_Liquido", "1"), ("Valor_Patrimonial_Cotas", "2")]],
        );
        let m = monthly_with(&Fake::new(vec![(Report::Mensal, year() - 1, vec![file])]), CNPJ_DIGITS).await.unwrap();
        assert_eq!(m.numero_cotistas, None); // vazio vira None
        assert_eq!(m.dividend_yield_mes, None);
    }

    #[tokio::test]
    async fn an_amended_filing_replaces_the_original_for_the_same_month() {
        let file = csv_file(
            &format!("inf_mensal_fii_complemento_{}.csv", year()),
            &COMPLEMENTO,
            &[
                vec![("CNPJ_Fundo_Classe", CNPJ_PUNCTUATED), ("Data_Referencia", "2026-02-01"), ("Versao", "2"),
                     ("Patrimonio_Liquido", "200"), ("Valor_Patrimonial_Cotas", "9")],
                vec![("CNPJ_Fundo_Classe", CNPJ_PUNCTUATED), ("Data_Referencia", "2026-02-01"), ("Versao", "1"),
                     ("Patrimonio_Liquido", "100"), ("Valor_Patrimonial_Cotas", "8")],
            ],
        );
        let m = monthly_with(&Fake::new(vec![(Report::Mensal, year(), vec![file])]), CNPJ_DIGITS).await.unwrap();
        assert_eq!(m.patrimonio_liquido, 200.0);
    }

    #[tokio::test]
    async fn an_unparseable_required_number_is_provider_unavailable() {
        let file = csv_file(
            &format!("inf_mensal_fii_complemento_{}.csv", year()),
            &COMPLEMENTO,
            &[vec![("CNPJ_Fundo_Classe", CNPJ_PUNCTUATED), ("Data_Referencia", "2026-02-01"), ("Versao", "1"),
                   ("Patrimonio_Liquido", "abc"), ("Valor_Patrimonial_Cotas", "9")]],
        );
        let err = monthly_with(&Fake::new(vec![(Report::Mensal, year(), vec![file])]), CNPJ_DIGITS).await.unwrap_err();
        assert!(matches!(err, AppError::ProviderUnavailable(_)), "{err}");
    }

    fn trimestral_fake() -> Fake {
        let prop = |date: &'static str, vacancia: &'static str, inad: &'static str, name: &'static str| {
            vec![("CNPJ_Fundo_Classe", CNPJ_PUNCTUATED), ("Data_Referencia", date), ("Versao", "1"), ("Nome_Imovel", name),
                 ("Endereco", "Av. Ayrton Senna, 3000"), ("Area", "56484.83"), ("Percentual_Vacancia", vacancia),
                 ("Percentual_Inadimplencia", inad), ("Percentual_Receitas_FII", "0.967897"), ("Percentual_Locado", "")]
        };
        let file = csv_file(
            &format!("inf_trimestral_fii_imovel_{}.csv", year()),
            &IMOVEL,
            &[
                prop("2026-03-31", "0.103933", "0.106526", "Via Parque Shopping"),
                prop("2026-06-30", "0.169062", "0.014417", "Via Parque Shopping"),
                prop("2026-06-30", "0.0", "0.0", "Alfa Galpão"),
            ],
        );
        Fake::new(vec![(Report::Trimestral, year(), vec![file])])
    }

    #[tokio::test]
    async fn properties_come_from_the_latest_quarter_sorted_by_name() {
        let props = properties_with(&trimestral_fake(), CNPJ_PUNCTUATED).await.unwrap();
        assert_eq!(props.len(), 2);
        assert_eq!(props[0].nome_imovel, "Alfa Galpão");
        assert_eq!(props[1].nome_imovel, "Via Parque Shopping");
        assert_eq!(props[1].reference_date, NaiveDate::from_ymd_opt(2026, 6, 30).unwrap());
        assert_eq!(props[1].percentual_vacancia, Some(0.169062));
        assert_eq!(props[1].percentual_locado, None);
        assert_eq!(props[1].endereco.as_deref(), Some("Av. Ayrton Senna, 3000"));
        assert_eq!(props[1].area_m2, Some(56484.83));
    }

    #[tokio::test]
    async fn a_fund_without_properties_gets_an_empty_list_not_an_error() {
        assert!(properties_with(&trimestral_fake(), "99999999000199").await.unwrap().is_empty());
    }

    fn summary(name: &str, admin: &str) -> FiiSummary {
        FiiSummary { ticker: "HGLG11".into(), name: name.into(), administrator_cnpj: admin.into() }
    }

    fn geral_fake(rows: Vec<Vec<(&'static str, &'static str)>>) -> Fake {
        let file = csv_file(&format!("inf_mensal_fii_geral_{}.csv", year()), &GERAL, &rows);
        Fake::new(vec![(Report::Mensal, year(), vec![file])])
    }

    fn one_fund() -> Vec<Vec<(&'static str, &'static str)>> {
        vec![vec![("CNPJ_Fundo_Classe", CNPJ_PUNCTUATED), ("CNPJ_Administrador", ADMIN_PUNCTUATED), ("Nome_Fundo_Classe", FUND_NAME)]]
    }

    #[tokio::test]
    async fn resolve_matches_administrator_and_name() {
        let got = resolve_with(&geral_fake(one_fund()), &summary(FUND_NAME, ADMIN_PUNCTUATED)).await.unwrap();
        assert_eq!(got, FundResolution { cnpj: CNPJ_DIGITS.into(), fund_name: FUND_NAME.into() });
        // Espaços e caixa são normalizados.
        let loose = summary(&format!("  {}  ", FUND_NAME.to_lowercase().replace(' ', "   ")), "27809513000130");
        assert!(resolve_with(&geral_fake(one_fund()), &loose).await.is_ok());
    }

    #[tokio::test]
    async fn resolve_refuses_to_guess() {
        // Administrador diferente.
        let err = resolve_with(&geral_fake(one_fund()), &summary(FUND_NAME, "99.999.999/0001-99")).await.unwrap_err();
        assert!(matches!(err, AppError::NoData(_)));
        // Nome diferente.
        let err = resolve_with(&geral_fake(one_fund()), &summary("OUTRO FUNDO", ADMIN_PUNCTUATED)).await.unwrap_err();
        assert!(matches!(err, AppError::NoData(_)));
        // Dois fundos com o mesmo administrador e nome: ambíguo.
        let mut two = one_fund();
        two.push(vec![("CNPJ_Fundo_Classe", "11111111000199"), ("CNPJ_Administrador", ADMIN_PUNCTUATED), ("Nome_Fundo_Classe", FUND_NAME)]);
        let err = resolve_with(&geral_fake(two), &summary(FUND_NAME, ADMIN_PUNCTUATED)).await.unwrap_err();
        assert!(matches!(&err, AppError::NoData(m) if m.contains("refusing to guess")), "{err}");
    }

    #[tokio::test]
    async fn the_same_fund_listed_twice_is_still_one_candidate() {
        let mut dup = one_fund();
        dup.push(dup[0].clone());
        assert!(resolve_with(&geral_fake(dup), &summary(FUND_NAME, ADMIN_PUNCTUATED)).await.is_ok());
    }

    #[tokio::test]
    async fn a_zip_without_the_expected_file_is_provider_unavailable() {
        let fake = Fake::new(vec![(Report::Mensal, year(), vec![csv_file("other.csv", &GERAL, &[])])]);
        assert!(matches!(monthly_with(&fake, CNPJ_DIGITS).await, Err(AppError::ProviderUnavailable(_))));
    }

    // Ao vivo (rede): `cargo test --lib -- --ignored --nocapture finance_core::cvm_fii`.
    #[tokio::test]
    #[ignore]
    async fn live_hglg11_and_a_paper_fund() {
        let m = monthly_indicators(CNPJ_DIGITS).await.unwrap();
        println!("monthly {:?}", m.value);
        let p = properties(CNPJ_DIGITS).await.unwrap();
        println!("properties {} {:?}", p.value.len(), p.value.first());
        // O cadastro real tem que casar exatamente 1 fundo para um nome/administrador reais
        // (conferidos no `geral` de 2026: o HGLG11 agora se chama PÁTRIA LOG, e a linha do
        // fundo se repete uma vez por mês, o que não pode virar "ambíguo").
        let summary = summary("PÁTRIA LOG - FUNDO DE INVESTIMENTO IMOBILIÁRIO", "45.246.410/0001-55");
        let resolved = resolve_with(&CvmZips, &summary).await.unwrap();
        println!("resolved {resolved:?}");
        assert_eq!(resolved.cnpj, CNPJ_DIGITS);
    }
}
