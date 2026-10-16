// CVM, dados abertos de empresas (DFP, Demonstrações Financeiras Padronizadas) —
// porta de `lume/api/app/sources/cvm_dfp.py`. Não é uma API por empresa: a CVM
// publica **um zip por ano fiscal** com as demonstrações de todas as ~870
// companhias abertas:
// `https://dados.cvm.gov.br/dados/CIA_ABERTA/DOC/DFP/DADOS/dfp_cia_aberta_{ano}.zip`
//
// Esquema conferido nos arquivos reais: vários CSVs `;` em latin-1, um por
// demonstração (`DRE_con`, `BPA_con`/`BPP_con`, `DFC_MI_con`, `DMPL_con`). Cada
// linha é uma conta (`CD_CONTA`, código fixo igual pra ~850 das ~870 empresas)
// de uma empresa (`CD_CVM`, string com zeros à esquerda: comparar como inteiro)
// num período (`ORDEM_EXERC` = 'ÚLTIMO' / 'PENÚLTIMO': o arquivo anual traz os
// dois últimos exercícios).
//
// Nem toda conta é tão padronizada: EBIT, dívida, caixa e as peças do ΔNWC usam
// o mesmo código em ~850 empresas e são lidas direto. D&A e Capex não (o código
// varia), então saem de busca por palavra em `DS_CONTA` e dão `None` em vez de
// arriscar um número errado quando não há um grupo de linhas confiável.
//
// O parsing é síncrono (CPU) e roda em `spawn_blocking`; o download é assíncrono
// e fica atrás de `ZipFetcher`, pra os testes usarem zips montados em memória.

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{Datelike, Utc};

use crate::error::AppError;
use crate::finance_core::disk_cache;
use crate::finance_core::provider::{cascade, BoxFuture, DataProvider, Sourced};

pub const SOURCE: &str = "cvm_dfp";
const ZIP_URL: &str = "https://dados.cvm.gov.br/dados/CIA_ABERTA/DOC/DFP/DADOS";

const LATEST: &str = "ÚLTIMO";
const PRIOR: &str = "PENÚLTIMO";
const PAYOUT_YEARS_AVERAGED: i32 = 5;
/// Taxa efetiva perto de lucro zero explode (um caso real deu 263%); isto é a
/// rede de segurança pros positivos quase zero (zero/negativo já é tratado antes).
const MAX_PLAUSIBLE_TAX_RATE: f64 = 100.0;
const PATRIMONIO_LIQUIDO_CONSOLIDADO: &str = "Patrimônio Líquido Consolidado";
/// Os dois últimos anos fiscais podem ganhar retificações; os mais antigos não mudam.
const RECENT_ZIP_MAX_AGE: Duration = Duration::from_secs(30 * 24 * 3600);

// ---------------------------------------------------------------------------
// Linhas
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    cd_conta: String,
    ds_conta: String,
    ordem: String,
    versao: i32,
    escala: String,
    vl_conta: String,
    coluna_df: Option<String>,
}

fn unavailable(detail: impl std::fmt::Display) -> AppError {
    AppError::ProviderUnavailable(format!("CVM DFP: {detail}"))
}

/// latin-1: cada byte é um code point.
fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

/// Linhas de `file_name` dentro do zip que pertencem a `cvm_code`.
fn read_rows(zip_path: &Path, file_name: &str, cvm_code: i32) -> Result<Vec<Row>, AppError> {
    let file = File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(BufReader::new(file)).map_err(unavailable)?;
    let entry = archive.by_name(file_name).map_err(|e| unavailable(format!("{file_name}: {e}")))?;
    let mut reader = csv::ReaderBuilder::new().delimiter(b';').flexible(true).from_reader(entry);

    let headers = reader.byte_headers().map_err(unavailable)?.clone();
    let column = |name: &str| headers.iter().position(|h| h == name.as_bytes());
    let required = |name: &str| column(name).ok_or_else(|| unavailable(format!("{file_name}: missing column {name}")));
    let (c_cvm, c_conta, c_desc, c_ordem, c_versao, c_escala, c_valor) = (
        required("CD_CVM")?,
        required("CD_CONTA")?,
        required("DS_CONTA")?,
        required("ORDEM_EXERC")?,
        required("VERSAO")?,
        required("ESCALA_MOEDA")?,
        required("VL_CONTA")?,
    );
    let c_coluna = column("COLUNA_DF");

    let mut rows = Vec::new();
    for record in reader.byte_records() {
        let record = record.map_err(unavailable)?;
        let field = |i: usize| record.get(i).map(latin1).unwrap_or_default();
        if field(c_cvm).trim().parse::<i32>().ok() != Some(cvm_code) {
            continue;
        }
        let versao = field(c_versao)
            .trim()
            .parse::<i32>()
            .map_err(|_| unavailable(format!("{file_name}: invalid VERSAO")))?;
        rows.push(Row {
            cd_conta: field(c_conta),
            ds_conta: field(c_desc),
            ordem: field(c_ordem),
            versao,
            escala: field(c_escala),
            vl_conta: field(c_valor),
            coluna_df: c_coluna.map(field),
        });
    }
    Ok(rows)
}

// ---------------------------------------------------------------------------
// Extração de contas (funções puras sobre as linhas da empresa)
// ---------------------------------------------------------------------------

/// Uma empresa pode ter mais de uma `VERSAO` (arquivo retificado) no mesmo ano:
/// fica só a mais recente, senão contas duplicadas inflariam qualquer soma.
fn latest_version<'a>(rows: Vec<&'a Row>) -> Vec<&'a Row> {
    let Some(max) = rows.iter().map(|r| r.versao).max() else { return Vec::new() };
    rows.into_iter().filter(|r| r.versao == max).collect()
}

fn to_millions_brl(row: &Row) -> Result<f64, AppError> {
    let scale = match row.escala.as_str() {
        "MIL" => 1_000.0,
        "MILHAO" => 1_000_000.0,
        "UNIDADE" => 1.0,
        other => return Err(unavailable(format!("unknown ESCALA_MOEDA '{other}'"))),
    };
    let value = row.vl_conta.trim().parse::<f64>().map_err(|_| unavailable(format!("invalid VL_CONTA '{}'", row.vl_conta)))?;
    Ok(value * scale / 1_000_000.0)
}

/// Conta de código estável (EBIT, dívida, caixa...). `Ok(None)` se a conta não
/// existe pra essa empresa/período.
fn find_exact(rows: &[Row], cd_conta: &str, ordem: &str) -> Result<Option<f64>, AppError> {
    let candidates = latest_version(rows.iter().filter(|r| r.cd_conta == cd_conta && r.ordem == ordem).collect());
    candidates.first().map(|r| to_millions_brl(r)).transpose()
}

/// Contas por prefixo de código + palavra no texto (D&A, Capex: códigos não
/// padronizados). Descarta as linhas "pai" (cujo código é prefixo de outra
/// linha casada) pra não somar subtotal junto do detalhe. `None` se nada casar;
/// nunca chuta.
///
/// `Sign::Absolute` devolve o módulo da soma: certo para saídas de caixa
/// (dividendos, capex, D&A), que a CVM lança negativas. `Sign::Keep` mantém o
/// sinal, necessário para lucro e patrimônio: o Python do Lume aplicava `abs()`
/// sempre, e por isso um prejuízo virava lucro e os guardas `lucro <= 0` e
/// `patrimônio <= 0` nunca disparavam (ROE positivo para empresa com prejuízo).
#[derive(Clone, Copy, PartialEq)]
enum Sign {
    Absolute,
    Keep,
}

fn find_by_keyword(
    rows: &[&Row],
    code_prefix: &str,
    keywords: &[&str],
    ordem: &str,
    sign: Sign,
) -> Result<Option<f64>, AppError> {
    let matched = latest_version(
        rows.iter()
            .copied()
            .filter(|r| {
                r.cd_conta.starts_with(code_prefix)
                    && r.ordem == ordem
                    && keywords.iter().any(|k| r.ds_conta.to_lowercase().contains(k))
            })
            .collect(),
    );
    if matched.is_empty() {
        return Ok(None);
    }
    let leaves: Vec<&Row> = matched
        .iter()
        .copied()
        .filter(|r| {
            let child_prefix = format!("{}.", r.cd_conta);
            !matched.iter().any(|other| other.cd_conta != r.cd_conta && other.cd_conta.starts_with(&child_prefix))
        })
        .collect();
    let mut total = 0.0;
    for row in leaves {
        total += to_millions_brl(row)?;
    }
    Ok(Some(if sign == Sign::Absolute { total.abs() } else { total }))
}

fn refs(rows: &[Row]) -> Vec<&Row> {
    rows.iter().collect()
}

/// Imposto sobre o lucro ÷ lucro antes do imposto (`3.08` / `3.07`), em %.
/// `Ok(None)` se o lucro é zero/negativo ou a razão sai de uma faixa plausível.
/// `Err(Missing)` (via `Option` externo) quando falta uma das contas.
fn effective_tax_rate(dre: &[Row]) -> Result<Option<Option<f64>>, AppError> {
    let (Some(pretax), Some(tax)) = (find_exact(dre, "3.07", LATEST)?, find_exact(dre, "3.08", LATEST)?) else {
        return Ok(None);
    };
    if pretax <= 0.0 {
        return Ok(Some(None));
    }
    let rate = -tax / pretax * 100.0;
    Ok(Some((0.0..=MAX_PLAUSIBLE_TAX_RATE).contains(&rate).then_some(rate)))
}

/// ΔNWC = (a receber + estoque − fornecedores) do ano menos o do anterior.
fn nwc_change(bpa: &[Row], bpp: &[Row]) -> Result<Option<f64>, AppError> {
    let nwc_at = |ordem: &str| -> Result<Option<f64>, AppError> {
        let (Some(receivables), Some(inventory), Some(payables)) = (
            find_exact(bpa, "1.01.03", ordem)?,
            find_exact(bpa, "1.01.04", ordem)?,
            find_exact(bpp, "2.01.02", ordem)?,
        ) else {
            return Ok(None);
        };
        Ok(Some(receivables + inventory - payables))
    };
    Ok(match (nwc_at(LATEST)?, nwc_at(PRIOR)?) {
        (Some(now), Some(before)) => Some(now - before),
        _ => None,
    })
}

/// Dividendos + JCP, por palavra-chave na coluna "Patrimônio Líquido
/// Consolidado" do DMPL (o arquivo repete cada linha por componente do PL; a
/// coluna evita multiplicar a soma). Cai na coluna vazia só quando a principal
/// soma exatamente `0.0` (`None` = nada casou; `0.0` = casou e deu zero, só esse é suspeito).
fn extract_distributions(dmpl: &[Row]) -> Result<Option<f64>, AppError> {
    const KEYWORDS: [&str; 2] = ["dividendo", "juros sobre capital"];
    let in_column = |column: &str| -> Vec<&Row> {
        dmpl.iter().filter(|r| r.coluna_df.as_deref() == Some(column)).collect()
    };
    let main = find_by_keyword(&in_column(PATRIMONIO_LIQUIDO_CONSOLIDADO), "5.", &KEYWORDS, LATEST, Sign::Absolute)?;
    if main != Some(0.0) {
        return Ok(main);
    }
    find_by_keyword(&in_column(""), "5.", &KEYWORDS, LATEST, Sign::Absolute)
}

// ---------------------------------------------------------------------------
// Resultados
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct Roe {
    pub reference_year: i32,
    pub roe: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DcfFundamentals {
    pub reference_year: i32,
    pub ebit: f64,
    pub tax_rate: Option<f64>,
    pub depreciation_amortization: Option<f64>,
    pub capex: Option<f64>,
    pub nwc_change: f64,
    pub total_debt: f64,
    pub cash: f64,
    pub revenue: f64,
    pub inventory: f64,
}

fn roe_of(dre: &[Row], bpp: &[Row], year: i32) -> Result<Option<Roe>, AppError> {
    if dre.is_empty() || bpp.is_empty() {
        return Ok(None);
    }
    let net_income = find_by_keyword(&refs(dre), "3.", &["consolidado do período"], LATEST, Sign::Keep)?;
    let equity = find_by_keyword(&refs(bpp), "2.", &["patrimônio líquido consolidado"], LATEST, Sign::Keep)?;
    Ok(match (net_income, equity) {
        (Some(income), Some(equity)) if equity > 0.0 => Some(Roe { reference_year: year, roe: income / equity * 100.0 }),
        _ => None,
    })
}

/// Lucro e distribuições do ano pro payout; `None` se o ano não é utilizável.
fn payout_year(dre: &[Row], dmpl: &[Row]) -> Result<Option<(f64, f64)>, AppError> {
    if dre.is_empty() || dmpl.is_empty() {
        return Ok(None);
    }
    let net_income = find_by_keyword(&refs(dre), "3.", &["consolidado do período"], LATEST, Sign::Keep)?;
    let distributions = extract_distributions(dmpl)?;
    Ok(match (net_income, distributions) {
        (Some(income), Some(dist)) if income > 0.0 => Some((income, dist)),
        _ => None,
    })
}

fn dcf_of(dre: &[Row], bpa: &[Row], bpp: &[Row], dfc: &[Row], year: i32) -> Result<Option<DcfFundamentals>, AppError> {
    if dre.is_empty() {
        return Ok(None);
    }
    // Falta de qualquer conta fixa descarta o resultado inteiro (como o LookupError do Lume);
    // só D&A, Capex e taxa efetiva podem faltar isoladamente.
    let Some(tax_rate) = effective_tax_rate(dre)? else { return Ok(None) };
    let (Some(ebit), Some(revenue), Some(nwc)) = (find_exact(dre, "3.05", LATEST)?, find_exact(dre, "3.01", LATEST)?, nwc_change(bpa, bpp)?) else {
        return Ok(None);
    };
    let (Some(debt_short), Some(debt_long), Some(cash_a), Some(cash_b), Some(inventory)) = (
        find_exact(bpp, "2.01.04", LATEST)?,
        find_exact(bpp, "2.02.01", LATEST)?,
        find_exact(bpa, "1.01.01", LATEST)?,
        find_exact(bpa, "1.01.02", LATEST)?,
        find_exact(bpa, "1.01.04", LATEST)?,
    ) else {
        return Ok(None);
    };
    Ok(Some(DcfFundamentals {
        reference_year: year,
        ebit,
        tax_rate,
        depreciation_amortization: find_by_keyword(&refs(dfc), "6.01.01", &["depreciaç", "amortiza", "exaust"], LATEST, Sign::Absolute)?,
        capex: find_by_keyword(&refs(dfc), "6.02", &["imobilizado", "intangív", "intangiv"], LATEST, Sign::Absolute)?,
        nwc_change: nwc,
        total_debt: debt_short + debt_long,
        cash: cash_a + cash_b,
        revenue,
        inventory,
    }))
}

// ---------------------------------------------------------------------------
// Download e orquestração
// ---------------------------------------------------------------------------

pub trait ZipFetcher: Send + Sync {
    /// Caminho local do zip do ano fiscal `year`; erro se não existe/não baixou.
    fn zip<'a>(&'a self, year: i32) -> BoxFuture<'a, Result<PathBuf, AppError>>;
}

pub struct CvmZips;

impl ZipFetcher for CvmZips {
    fn zip<'a>(&'a self, year: i32) -> BoxFuture<'a, Result<PathBuf, AppError>> {
        Box::pin(async move {
            let name = format!("dfp_cia_aberta_{year}.zip");
            let max_age = (year >= Utc::now().year() - 2).then_some(RECENT_ZIP_MAX_AGE);
            disk_cache::get_or_download("CVM DFP", &format!("cvm_dfp/{name}"), &format!("{ZIP_URL}/{name}"), max_age).await
        })
    }
}

/// O zip leva o nome do ano fiscal, não o da publicação: em meados de 2026 o
/// mais recente é o `2025` (exercício encerrado em 31/12/2025, entregue no
/// começo de 2026). Tenta o ano anterior e, se falhar (ex.: janeiro, antes da
/// CVM publicar), o retrasado.
async fn latest_zip(fetcher: &dyn ZipFetcher) -> Result<(i32, PathBuf), AppError> {
    let candidate = Utc::now().year() - 1;
    match fetcher.zip(candidate).await {
        Ok(path) => Ok((candidate, path)),
        Err(_) => Ok((candidate - 1, fetcher.zip(candidate - 1).await?)),
    }
}

async fn rows(path: &Path, statement: &str, year: i32, cvm_code: i32) -> Result<Vec<Row>, AppError> {
    let path = path.to_path_buf();
    let file_name = format!("dfp_cia_aberta_{statement}_{year}.csv");
    tokio::task::spawn_blocking(move || read_rows(&path, &file_name, cvm_code))
        .await
        .map_err(|e| unavailable(format!("parsing task failed: {e}")))?
}

fn no_data(cvm_code: i32, what: &str) -> AppError {
    AppError::NoData(format!("CVM DFP: no {what} for CVM code {cvm_code}"))
}

pub async fn roe_with(fetcher: &dyn ZipFetcher, cvm_code: i32) -> Result<Roe, AppError> {
    let (year, path) = latest_zip(fetcher).await?;
    let dre = rows(&path, "DRE_con", year, cvm_code).await?;
    let bpp = rows(&path, "BPP_con", year, cvm_code).await?;
    roe_of(&dre, &bpp, year)?.ok_or_else(|| no_data(cvm_code, "ROE"))
}

/// Payout médio dos últimos 5 anos fiscais: soma distribuições e lucros ano a
/// ano e divide uma vez no fim (média simples de payouts anuais deixaria um ano
/// distorcido pesar igual aos outros). Anos sem dado utilizável são pulados.
pub async fn payout_with(fetcher: &dyn ZipFetcher, cvm_code: i32) -> Result<f64, AppError> {
    let (latest_year, latest_path) = latest_zip(fetcher).await?;
    let (mut income_total, mut distributions_total, mut years_found) = (0.0, 0.0, 0);
    for year in (latest_year - PAYOUT_YEARS_AVERAGED + 1..=latest_year).rev() {
        let path = if year == latest_year { latest_path.clone() } else { fetcher.zip(year).await? };
        let dre = rows(&path, "DRE_con", year, cvm_code).await?;
        let dmpl = rows(&path, "DMPL_con", year, cvm_code).await?;
        if let Some((income, distributions)) = payout_year(&dre, &dmpl)? {
            income_total += income;
            distributions_total += distributions;
            years_found += 1;
        }
    }
    if years_found == 0 {
        return Err(no_data(cvm_code, "payout"));
    }
    Ok(distributions_total / income_total * 100.0)
}

pub async fn dcf_with(fetcher: &dyn ZipFetcher, cvm_code: i32) -> Result<DcfFundamentals, AppError> {
    let (year, path) = latest_zip(fetcher).await?;
    let dre = rows(&path, "DRE_con", year, cvm_code).await?;
    let bpa = rows(&path, "BPA_con", year, cvm_code).await?;
    let bpp = rows(&path, "BPP_con", year, cvm_code).await?;
    let dfc = rows(&path, "DFC_MI_con", year, cvm_code).await?;
    dcf_of(&dre, &bpa, &bpp, &dfc, year)?.ok_or_else(|| no_data(cvm_code, "DCF fundamentals"))
}

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------

pub trait CompanyFundamentalsProvider: DataProvider {
    fn roe<'a>(&'a self, cvm_code: i32) -> BoxFuture<'a, Result<Roe, AppError>>;
    fn payout<'a>(&'a self, cvm_code: i32) -> BoxFuture<'a, Result<f64, AppError>>;
    fn dcf_fundamentals<'a>(&'a self, cvm_code: i32) -> BoxFuture<'a, Result<DcfFundamentals, AppError>>;
}

pub struct CvmDfp;

impl DataProvider for CvmDfp {
    fn id(&self) -> &'static str {
        SOURCE
    }
}

impl CompanyFundamentalsProvider for CvmDfp {
    fn roe<'a>(&'a self, cvm_code: i32) -> BoxFuture<'a, Result<Roe, AppError>> {
        Box::pin(async move { roe_with(&CvmZips, cvm_code).await })
    }
    fn payout<'a>(&'a self, cvm_code: i32) -> BoxFuture<'a, Result<f64, AppError>> {
        Box::pin(async move { payout_with(&CvmZips, cvm_code).await })
    }
    fn dcf_fundamentals<'a>(&'a self, cvm_code: i32) -> BoxFuture<'a, Result<DcfFundamentals, AppError>> {
        Box::pin(async move { dcf_with(&CvmZips, cvm_code).await })
    }
}

fn providers() -> Vec<Box<dyn CompanyFundamentalsProvider>> {
    vec![Box::new(CvmDfp)]
}

pub async fn roe(cvm_code: i32) -> Result<Sourced<Roe>, AppError> {
    let providers = providers();
    let attempts = providers.iter().map(|p| (p.id(), p.roe(cvm_code))).collect();
    cascade(&format!("ROE for CVM {cvm_code}"), attempts).await
}

pub async fn payout(cvm_code: i32) -> Result<Sourced<f64>, AppError> {
    let providers = providers();
    let attempts = providers.iter().map(|p| (p.id(), p.payout(cvm_code))).collect();
    cascade(&format!("payout for CVM {cvm_code}"), attempts).await
}

pub async fn dcf_fundamentals(cvm_code: i32) -> Result<Sourced<DcfFundamentals>, AppError> {
    let providers = providers();
    let attempts = providers.iter().map(|p| (p.id(), p.dcf_fundamentals(cvm_code))).collect();
    cascade(&format!("DCF fundamentals for CVM {cvm_code}"), attempts).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::io::Write;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    /// Mesmos campos dos CSVs reais (conferidos ao vivo pelo Lume), no mínimo
    /// necessário pra exercitar o parsing.
    const FIELDS: [&str; 15] = [
        "CNPJ_CIA", "DT_REFER", "VERSAO", "DENOM_CIA", "CD_CVM", "GRUPO_DFP", "MOEDA", "ESCALA_MOEDA",
        "ORDEM_EXERC", "DT_INI_EXERC", "DT_FIM_EXERC", "CD_CONTA", "DS_CONTA", "VL_CONTA", "ST_CONTA_FIXA",
    ];
    const DMPL_FIELDS: [&str; 16] = [
        "CNPJ_CIA", "DT_REFER", "VERSAO", "DENOM_CIA", "CD_CVM", "GRUPO_DFP", "MOEDA", "ESCALA_MOEDA",
        "ORDEM_EXERC", "DT_INI_EXERC", "DT_FIM_EXERC", "COLUNA_DF", "CD_CONTA", "DS_CONTA", "VL_CONTA",
        "ST_CONTA_FIXA",
    ];

    fn t(conta: &str, desc: &str, valor: &str) -> Row {
        Row {
            cd_conta: conta.into(),
            ds_conta: desc.into(),
            ordem: "ÚLTIMO".into(),
            versao: 1,
            escala: "MIL".into(),
            vl_conta: valor.into(),
            coluna_df: Some("Patrimônio Líquido Consolidado".into()),
        }
    }

    fn csv_bytes(fields: &[&str], rows: &[Row]) -> Vec<u8> {
        let mut lines = vec![fields.join(";")];
        for r in rows {
            let line: Vec<String> = fields
                .iter()
                .map(|f| match *f {
                    "CD_CVM" => "004170".to_string(),
                    "VERSAO" => r.versao.to_string(),
                    "ORDEM_EXERC" => r.ordem.clone(),
                    "CD_CONTA" => r.cd_conta.clone(),
                    "DS_CONTA" => r.ds_conta.clone(),
                    "VL_CONTA" => r.vl_conta.clone(),
                    "ESCALA_MOEDA" => r.escala.clone(),
                    "COLUNA_DF" => r.coluna_df.clone().unwrap_or_default(),
                    _ => String::new(),
                })
                .collect();
            lines.push(line.join(";"));
        }
        // Texto de teste só tem caracteres < U+0100, então codifica 1:1 em latin-1.
        lines.join("\n").chars().map(|c| c as u8).collect()
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

    /// Fetcher falso: um zip por ano; ano ausente = falha de download.
    struct Fake {
        zips: HashMap<i32, PathBuf>,
        requested: std::sync::Mutex<Vec<i32>>,
    }

    impl Fake {
        fn new(zips: Vec<(i32, Vec<(String, Vec<u8>)>)>) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "anchor-cvm-test-{}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::SeqCst)
            ));
            std::fs::create_dir_all(&dir).unwrap();
            let zips = zips
                .into_iter()
                .map(|(year, files)| {
                    let path = dir.join(format!("dfp_cia_aberta_{year}.zip"));
                    std::fs::write(&path, zip_bytes(&files)).unwrap();
                    (year, path)
                })
                .collect();
            Fake { zips, requested: std::sync::Mutex::new(Vec::new()) }
        }
    }

    impl ZipFetcher for Fake {
        fn zip<'a>(&'a self, year: i32) -> BoxFuture<'a, Result<PathBuf, AppError>> {
            self.requested.lock().unwrap().push(year);
            Box::pin(async move {
                self.zips.get(&year).cloned().ok_or_else(|| unavailable(format!("download failed for {year}")))
            })
        }
    }

    fn year() -> i32 {
        Utc::now().year() - 1
    }

    fn file(statement: &str, year: i32, fields: &[&str], rows: &[Row]) -> (String, Vec<u8>) {
        (format!("dfp_cia_aberta_{statement}_{year}.csv"), csv_bytes(fields, rows))
    }

    fn roe_fake() -> Fake {
        let y = year();
        Fake::new(vec![(
            y,
            vec![
                file("DRE_con", y, &FIELDS, &[t("3.01", "Lucro Líquido Consolidado do Período", "100000")]),
                file("BPP_con", y, &FIELDS, &[t("2.01", "Patrimônio Líquido Consolidado", "500000")]),
            ],
        )])
    }

    #[tokio::test]
    async fn roe_is_net_income_over_equity() {
        let roe = roe_with(&roe_fake(), 4170).await.unwrap();
        assert_eq!(roe, Roe { reference_year: year(), roe: 20.0 });
    }

    #[tokio::test]
    async fn unknown_company_is_no_data() {
        assert!(matches!(roe_with(&roe_fake(), 999999).await, Err(AppError::NoData(_))));
    }

    #[tokio::test]
    async fn non_positive_equity_is_no_data() {
        let y = year();
        let fake = Fake::new(vec![(
            y,
            vec![
                file("DRE_con", y, &FIELDS, &[t("3.01", "Lucro Líquido Consolidado do Período", "100000")]),
                file("BPP_con", y, &FIELDS, &[t("2.01", "Patrimônio Líquido Consolidado", "-5")]),
            ],
        )]);
        assert!(matches!(roe_with(&fake, 4170).await, Err(AppError::NoData(_))));
    }

    #[tokio::test]
    async fn falls_back_to_the_year_before_when_the_latest_zip_is_missing() {
        let y = year() - 1;
        let fake = Fake::new(vec![(
            y,
            vec![
                file("DRE_con", y, &FIELDS, &[t("3.01", "Lucro Líquido Consolidado do Período", "100000")]),
                file("BPP_con", y, &FIELDS, &[t("2.01", "Patrimônio Líquido Consolidado", "500000")]),
            ],
        )]);
        assert_eq!(roe_with(&fake, 4170).await.unwrap().reference_year, y);
    }

    #[tokio::test]
    async fn download_failure_of_every_year_is_provider_unavailable() {
        let fake = Fake::new(vec![]);
        assert!(matches!(roe_with(&fake, 4170).await, Err(AppError::ProviderUnavailable(_))));
    }

    #[tokio::test]
    async fn a_missing_csv_inside_the_zip_is_provider_unavailable() {
        let y = year();
        let fake = Fake::new(vec![(y, vec![file("DRE_con", y, &FIELDS, &[])])]);
        assert!(matches!(roe_with(&fake, 4170).await, Err(AppError::ProviderUnavailable(_))));
    }

    fn dcf_files(y: i32) -> Vec<(String, Vec<u8>)> {
        let prior = |conta, valor| Row { ordem: "PENÚLTIMO".into(), ..t(conta, "", valor) };
        vec![
            file("DRE_con", y, &FIELDS, &[t("3.01", "", "100000"), t("3.05", "", "50000"), t("3.07", "", "40000"), t("3.08", "", "-10000")]),
            file(
                "BPA_con",
                y,
                &FIELDS,
                &[
                    t("1.01.01", "", "20000"),
                    t("1.01.02", "", "5000"),
                    t("1.01.03", "", "10000"),
                    t("1.01.04", "", "8000"),
                    prior("1.01.03", "9000"),
                    prior("1.01.04", "7000"),
                ],
            ),
            file(
                "BPP_con",
                y,
                &FIELDS,
                &[t("2.01.02", "", "6000"), prior("2.01.02", "5000"), t("2.01.04", "", "15000"), t("2.02.01", "", "25000")],
            ),
            file(
                "DFC_MI_con",
                y,
                &FIELDS,
                &[t("6.01.01.01", "Depreciação e amortização", "3000"), t("6.02.01", "Aquisição de imobilizado", "-4000")],
            ),
        ]
    }

    #[tokio::test]
    async fn dcf_computes_all_nine_fields() {
        let y = year();
        let dcf = dcf_with(&Fake::new(vec![(y, dcf_files(y))]), 4170).await.unwrap();
        assert_eq!(dcf.reference_year, y);
        assert_eq!(dcf.ebit, 50.0);
        assert_eq!(dcf.tax_rate, Some(25.0));
        assert_eq!(dcf.depreciation_amortization, Some(3.0));
        assert_eq!(dcf.capex, Some(4.0));
        assert!((dcf.nwc_change - 1.0).abs() < 1e-9); // (10+8−6) − (9+7−5)
        assert_eq!(dcf.total_debt, 40.0);
        assert_eq!(dcf.cash, 25.0);
        assert_eq!(dcf.revenue, 100.0);
        assert_eq!(dcf.inventory, 8.0);
    }

    #[tokio::test]
    async fn dcf_for_an_unknown_company_is_no_data() {
        let y = year();
        let fake = Fake::new(vec![(y, dcf_files(y))]);
        assert!(matches!(dcf_with(&fake, 999999).await, Err(AppError::NoData(_))));
    }

    #[tokio::test]
    async fn dcf_without_a_fixed_account_is_no_data_but_without_da_capex_is_fine() {
        let y = year();
        // Sem D&A nem Capex no DFC: os dois viram None, o resto segue.
        let mut files = dcf_files(y);
        files[3] = file("DFC_MI_con", y, &FIELDS, &[]);
        let dcf = dcf_with(&Fake::new(vec![(y, files)]), 4170).await.unwrap();
        assert_eq!((dcf.depreciation_amortization, dcf.capex), (None, None));
        // Sem a conta de caixa 1.01.02: descarta tudo.
        let mut files = dcf_files(y);
        files[1] = file("BPA_con", y, &FIELDS, &[t("1.01.01", "", "20000")]);
        assert!(matches!(dcf_with(&Fake::new(vec![(y, files)]), 4170).await, Err(AppError::NoData(_))));
    }

    #[test]
    fn tax_rate_is_unavailable_when_pretax_income_is_not_positive_or_implausible() {
        let dre = |pretax, tax| vec![t("3.07", "", pretax), t("3.08", "", tax)];
        assert_eq!(effective_tax_rate(&dre("40000", "-10000")).unwrap(), Some(Some(25.0)));
        assert_eq!(effective_tax_rate(&dre("0", "-10")).unwrap(), Some(None));
        assert_eq!(effective_tax_rate(&dre("-5", "-10")).unwrap(), Some(None));
        assert_eq!(effective_tax_rate(&dre("100", "-263")).unwrap(), Some(None)); // 263%
        assert_eq!(effective_tax_rate(&dre("100", "10")).unwrap(), Some(None)); // crédito de imposto (<0%)
        assert_eq!(effective_tax_rate(&[t("3.07", "", "100")]).unwrap(), None); // falta 3.08
    }

    #[test]
    fn scales_and_the_latest_amended_version() {
        let mut rows = vec![t("3.01", "", "1")];
        rows[0].escala = "MILHAO".into();
        assert_eq!(find_exact(&rows, "3.01", LATEST).unwrap(), Some(1.0));
        rows[0].escala = "UNIDADE".into();
        rows[0].vl_conta = "2500000".into();
        assert_eq!(find_exact(&rows, "3.01", LATEST).unwrap(), Some(2.5));
        // Retificação (versão 2) vence a original mesmo vindo antes no arquivo.
        let amended = vec![Row { versao: 2, ..t("3.01", "", "7000") }, t("3.01", "", "1000")];
        assert_eq!(find_exact(&amended, "3.01", LATEST).unwrap(), Some(7.0));
        let mut bad = t("3.01", "", "1");
        bad.escala = "BILHAO".into();
        assert!(find_exact(&[bad], "3.01", LATEST).is_err());
    }

    #[test]
    fn net_income_keeps_its_sign_so_a_loss_is_not_reported_as_profit() {
        let rows = vec![t("3.11", "Lucro/Prejuízo Consolidado do Período", "-30000")];
        assert_eq!(find_by_keyword(&refs(&rows), "3.", &["consolidado do período"], LATEST, Sign::Keep).unwrap(), Some(-30.0));
        assert_eq!(find_by_keyword(&refs(&rows), "3.", &["consolidado do período"], LATEST, Sign::Absolute).unwrap(), Some(30.0));
    }

    #[test]
    fn keyword_search_drops_parent_rows_to_avoid_double_counting() {
        let rows = vec![
            t("6.02", "Aquisição de imobilizado e intangível", "-9000"), // pai: subtotal
            t("6.02.01", "Aquisição de imobilizado", "-4000"),
            t("6.02.02", "Aquisição de intangível", "-3000"),
        ];
        let got = find_by_keyword(&refs(&rows), "6.02", &["imobilizado", "intangív", "intangiv"], LATEST, Sign::Absolute).unwrap();
        assert_eq!(got, Some(7.0)); // só as folhas, em módulo
        assert_eq!(find_by_keyword(&refs(&rows), "6.02", &["inexistente"], LATEST, Sign::Absolute).unwrap(), None);
    }

    #[test]
    fn distributions_fall_back_to_the_blank_column_only_when_the_main_one_sums_to_zero() {
        let blank = |valor| Row { coluna_df: Some(String::new()), ..t("5.04.06", "Dividendos", valor) };
        let main = |valor| t("5.04.06", "Dividendos", valor);
        // Coluna principal soma 0 → usa a vazia.
        assert_eq!(extract_distributions(&[main("0"), blank("-8000")]).unwrap(), Some(8.0));
        // Coluna principal tem valor → ignora a vazia.
        assert_eq!(extract_distributions(&[main("-2000"), blank("-8000")]).unwrap(), Some(2.0));
        // Nada casou na principal (None) → não cai na vazia.
        let other = Row { ds_conta: "Capital social".into(), ..main("-1") };
        assert_eq!(extract_distributions(&[other, blank("-8000")]).unwrap(), None);
    }

    fn payout_zip(y: i32, income: Option<&'static str>, distributions: Option<&'static str>) -> (i32, Vec<(String, Vec<u8>)>) {
        let dre: Vec<Row> = income.map(|v| vec![t("3.01", "Lucro Líquido Consolidado do Período", v)]).unwrap_or_default();
        let dmpl: Vec<Row> = distributions.map(|v| vec![t("5.04.06", "Dividendos", v)]).unwrap_or_default();
        (y, vec![file("DRE_con", y, &FIELDS, &dre), file("DMPL_con", y, &DMPL_FIELDS, &dmpl)])
    }

    #[tokio::test]
    async fn payout_sums_across_years_and_skips_years_without_data() {
        let y = year();
        let fake = Fake::new(vec![
            payout_zip(y, Some("100000"), Some("-20000")),
            payout_zip(y - 1, Some("100000"), Some("-40000")),
            payout_zip(y - 2, None, None),
            payout_zip(y - 3, None, None),
            payout_zip(y - 4, None, None),
        ]);
        // (20 + 40) ÷ (100 + 100) × 100 = 30%
        assert_eq!(payout_with(&fake, 4170).await.unwrap(), 30.0);
        assert_eq!(fake.requested.lock().unwrap().iter().filter(|&&r| r == y).count(), 1); // o mais recente não é baixado de novo
    }

    #[tokio::test]
    async fn payout_with_no_usable_year_is_no_data_and_a_failed_year_is_an_error() {
        let y = year();
        let empty = Fake::new((0..5).map(|i| payout_zip(y - i, None, None)).collect());
        assert!(matches!(payout_with(&empty, 4170).await, Err(AppError::NoData(_))));
        // Falta o zip de y−3: erro de download, não "sem dado".
        let gap = Fake::new(vec![
            payout_zip(y, Some("1"), Some("-1")),
            payout_zip(y - 1, Some("1"), Some("-1")),
            payout_zip(y - 2, Some("1"), Some("-1")),
            payout_zip(y - 4, Some("1"), Some("-1")),
        ]);
        assert!(matches!(payout_with(&gap, 4170).await, Err(AppError::ProviderUnavailable(_))));
    }

    #[tokio::test]
    async fn loss_making_years_do_not_count_toward_the_payout() {
        let y = year();
        let fake = Fake::new(
            std::iter::once(payout_zip(y, Some("-100000"), Some("-20000")))
                .chain(std::iter::once(payout_zip(y - 1, Some("100000"), Some("-50000"))))
                .chain((2..5).map(|i| payout_zip(y - i, None, None)))
                .collect(),
        );
        assert_eq!(payout_with(&fake, 4170).await.unwrap(), 50.0);
    }

    // Ao vivo (rede, baixa até ~75 MB na primeira vez):
    // `cargo test --lib -- --ignored --nocapture finance_core::cvm_dfp`.
    #[tokio::test]
    #[ignore]
    async fn live_known_companies() {
        for (name, cvm) in [("PETR", 9512), ("VALE", 4170), ("BBAS", 1023), ("ITUB", 19348)] {
            let started = std::time::Instant::now();
            let roe = roe(cvm).await;
            let payout = payout(cvm).await;
            let dcf = dcf_fundamentals(cvm).await;
            println!("{name} {cvm}: roe={roe:?} payout={payout:?} dcf={dcf:?} ({:?})", started.elapsed());
            assert!(roe.is_ok(), "{name}");
        }
    }
}
