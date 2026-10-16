// SEC EDGAR (fundamentos de empresas dos EUA, o equivalente da CVM pra DCF/FCFF)
// — porta de `lume/api/app/sources/sec_edgar.py`. Grátis e sem chave; a SEC só
// exige um `User-Agent` com e-mail de contato (política, não autenticação) e
// um teto de ~10 req/s.
//
// Diferente da CVM (um zip por ano, contas com código numérico), a SEC serve o
// XBRL por empresa: `GET data.sec.gov/api/xbrl/companyconcept/CIK{cik}/us-gaap/{tag}.json`
// devolve o **histórico completo de uma tag** (`Revenues`, `NetIncomeLoss`...) em
// uma chamada, inclusive o ano atual e o anterior, o que dispensa uma segunda
// chamada pro ΔNWC.
//
// **Achado crítico do Lume (conferido com a Apple)**: filtrar só por
// `form == "10-K"` e `fp == "FY"` não basta para conceitos de duração (os que
// têm `start`): a Apple usa a mesma tag nas notas de "dados trimestrais
// selecionados" do 10-K, então `Revenues` veio com linhas de trimestre ou 9
// meses mesmo com `form`/`fp` certos. Por isso a janela `end − start` precisa
// ficar entre 350 e 380 dias. Conceitos instantâneos (sem `start`, ex.
// `StockholdersEquity`) não precisam desse filtro.
//
// **Retificação (10-K/A)**: mais de uma linha pode ter o mesmo `end`; desempata
// o `filed` mais recente, não a primeira vista.
//
// **Lacuna de taxonomia dos bancos**: no JPMorgan, `OperatingIncomeLoss`,
// `InventoryNet`, contas a receber/pagar, capex, `LongTermDebtNoncurrent` e as
// tags de receita dão 404 (banco não reporta EBIT/estoque como indústria). O
// DCF devolve `NoData` pra eles; LPA/VPA/ROE continuam funcionando.
//
// **Escala**: todo campo monetário agregado sai em **milhões de dólares**, como
// o `cvm_dfp` faz com reais; LPA, VPA, ROE e taxa (razões) não são escalados.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use chrono::NaiveDate;
use serde::Deserialize;

use crate::error::AppError;
use crate::finance_core::api_keys::{self, SEC_EDGAR_EMAIL};
use crate::finance_core::cache::TtlCache;
use crate::finance_core::cvm_dfp::DcfFundamentals;
use crate::finance_core::provider::{cascade, BoxFuture, DataProvider, Sourced};
use crate::finance_core::{disk_cache, http};

pub const SOURCE: &str = "sec_edgar";
const TICKERS_URL: &str = "https://www.sec.gov/files/company_tickers.json";
const CONCEPT_URL: &str = "https://data.sec.gov/api/xbrl/companyconcept";

/// ~9 req/s, abaixo do teto de 10 req/s da SEC.
const REQUEST_INTERVAL: Duration = Duration::from_millis(110);
/// `company_tickers.json` não tem versão no nome (diferente do zip da CVM, que
/// leva o ano): cache eterno deixaria um IPO novo sem resolver pra sempre.
const TICKERS_MAX_AGE: Duration = Duration::from_secs(7 * 24 * 3600);
/// Várias chamadas do mesmo ticker (fundamentos, payout, DCF) repetem tags.
const CONCEPT_TTL: Duration = Duration::from_secs(3600);
const PAYOUT_YEARS_AVERAGED: usize = 5;
const MAX_PLAUSIBLE_TAX_RATE: f64 = 100.0;
const ANNUAL_DURATION_DAYS: std::ops::RangeInclusive<i64> = 350..=380;
const DIVIDEND_TAGS: [&str; 2] = ["PaymentsOfDividendsCommonStock", "PaymentsOfDividends"];
const UNITS: [&str; 3] = ["USD", "USD/shares", "shares"];
const PRETAX_INCOME_TAG: &str = "IncomeLossFromContinuingOperationsBeforeIncomeTaxesExtraordinaryItemsNoncontrollingInterest";

// ---------------------------------------------------------------------------
// Fatos do XBRL
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Fact {
    pub start: Option<String>,
    pub end: String,
    pub val: f64,
    pub fy: Option<i32>,
    pub fp: Option<String>,
    pub form: Option<String>,
    pub filed: String,
}

#[derive(Deserialize)]
struct ConceptPayload {
    #[serde(default)]
    units: BTreeMap<String, Vec<Fact>>,
}

fn unavailable(detail: impl std::fmt::Display) -> AppError {
    AppError::ProviderUnavailable(format!("SEC EDGAR: {detail}"))
}

/// Linhas da primeira unidade presente (`USD`, `USD/shares` ou `shares`: a chave
/// varia por tag). `None` se a empresa não reporta nenhuma delas.
pub fn parse_concept(body: &str) -> Result<Option<Vec<Fact>>, AppError> {
    let mut payload: ConceptPayload = serde_json::from_str(body).map_err(unavailable)?;
    Ok(UNITS.iter().find_map(|unit| payload.units.remove(*unit)))
}

#[derive(Clone, Copy)]
enum Pick {
    /// Ano fiscal mais recente de um conceito de duração.
    Duration,
    /// Conceito instantâneo: `0` é o ano fiscal mais recente, `1` o anterior.
    Instant(usize),
}

fn date(raw: &str) -> Result<NaiveDate, AppError> {
    NaiveDate::parse_from_str(raw, "%Y-%m-%d").map_err(|_| unavailable(format!("invalid date '{raw}'")))
}

fn is_annual_duration(fact: &Fact) -> Result<bool, AppError> {
    let Some(start) = &fact.start else { return Ok(true) };
    let days = (date(&fact.end)? - date(start)?).num_days();
    Ok(ANNUAL_DURATION_DAYS.contains(&days))
}

fn annual_rows(rows: &[Fact]) -> Result<Vec<&Fact>, AppError> {
    let mut annual = Vec::new();
    for fact in rows {
        if fact.form.as_deref() == Some("10-K") && fact.fp.as_deref() == Some("FY") && is_annual_duration(fact)? {
            annual.push(fact);
        }
    }
    Ok(annual)
}

/// Maior elemento por `key`, ficando com o **primeiro** em empate (como o `max`
/// do Python; o `max_by_key` do Rust ficaria com o último).
fn first_max_by<'a, T, K: Ord>(items: impl IntoIterator<Item = &'a T>, key: impl Fn(&T) -> K) -> Option<&'a T> {
    let mut best: Option<&T> = None;
    for item in items {
        if best.map_or(true, |b| key(item) > key(b)) {
            best = Some(item);
        }
    }
    best
}

fn pick(rows: &[Fact], how: Pick) -> Result<Option<Fact>, AppError> {
    let candidates = annual_rows(rows)?;
    if candidates.is_empty() {
        return Ok(None);
    }
    let end = match how {
        Pick::Duration => candidates.iter().map(|f| f.end.clone()).max().expect("non-empty"),
        Pick::Instant(rank) => {
            let ends: BTreeSet<&String> = candidates.iter().map(|f| &f.end).collect();
            let Some(end) = ends.into_iter().rev().nth(rank) else { return Ok(None) };
            end.clone()
        }
    };
    let same_end = candidates.into_iter().filter(|f| f.end == end);
    Ok(first_max_by(same_end, |f| f.filed.clone()).cloned())
}

// ---------------------------------------------------------------------------
// Fonte dos fatos (injetável nos testes)
// ---------------------------------------------------------------------------

pub trait ConceptSource: Send + Sync {
    /// Linhas do conceito `tag`; `Ok(None)` se a empresa não reporta a tag
    /// (404, normal pra bancos em vários campos).
    fn concept<'a>(&'a self, cik: i64, tag: &'a str) -> BoxFuture<'a, Result<Option<Vec<Fact>>, AppError>>;
}

/// Próximo horário livre: nunca antes de `now` nem antes de `previous` (o horário
/// reservado pela chamada anterior).
fn next_slot(previous: Option<Instant>, now: Instant) -> Instant {
    previous.map_or(now, |p| p.max(now))
}

static NEXT_SLOT: Mutex<Option<Instant>> = Mutex::new(None);

/// Toda chamada à SEC passa por aqui, concorrente ou não: o lock só reserva o
/// horário e a espera acontece fora dele.
async fn throttle() {
    let slot = {
        let mut guard = NEXT_SLOT.lock().expect("throttle lock");
        let slot = next_slot(*guard, Instant::now());
        *guard = Some(slot + REQUEST_INTERVAL);
        slot
    };
    tokio::time::sleep_until(tokio::time::Instant::from_std(slot)).await;
}

fn user_agent() -> Result<String, AppError> {
    Ok(format!("anchor ({})", api_keys::resolve(&SEC_EDGAR_EMAIL)?))
}

type ConceptKey = (i64, String);
static CONCEPTS: OnceLock<TtlCache<ConceptKey, Option<Vec<Fact>>>> = OnceLock::new();

pub struct SecHttp;

impl ConceptSource for SecHttp {
    fn concept<'a>(&'a self, cik: i64, tag: &'a str) -> BoxFuture<'a, Result<Option<Vec<Fact>>, AppError>> {
        Box::pin(async move {
            let cached = CONCEPTS
                .get_or_init(TtlCache::new)
                .get_or_refresh((cik, tag.to_string()), CONCEPT_TTL, || async {
                    let agent = user_agent()?;
                    let url = format!("{CONCEPT_URL}/CIK{cik:010}/us-gaap/{tag}.json");
                    throttle().await;
                    match http::get_text_with_headers("SEC EDGAR", &url, http::DEFAULT_TIMEOUT, &[("User-Agent", &agent)]).await {
                        Ok(body) => parse_concept(&body),
                        // A empresa não reporta esta tag: resposta válida, vale guardar.
                        Err(AppError::NoData(_)) => Ok(None),
                        Err(err) => Err(err),
                    }
                })
                .await?;
            Ok(cached.value)
        })
    }
}

// ---------------------------------------------------------------------------
// Ticker → CIK
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct TickerEntry {
    cik_str: i64,
    ticker: String,
}

/// CIK do `ticker` (sem caixa). A lista é um objeto cujas chaves são índices
/// `"0"`, `"1"`...: em ticker repetido vale o de menor índice, como a ordem do
/// arquivo no Python.
pub fn find_cik(body: &str, ticker: &str) -> Result<Option<i64>, AppError> {
    let entries: BTreeMap<String, TickerEntry> = serde_json::from_str(body).map_err(unavailable)?;
    Ok(entries
        .iter()
        .filter(|(_, entry)| entry.ticker.eq_ignore_ascii_case(ticker))
        .min_by_key(|(key, _)| key.parse::<u64>().unwrap_or(u64::MAX))
        .map(|(_, entry)| entry.cik_str))
}

pub async fn resolve_cik(ticker: &str) -> Result<i64, AppError> {
    let agent = user_agent()?;
    throttle().await;
    let path = disk_cache::get_or_download_with_headers(
        "SEC EDGAR",
        "sec_edgar/company_tickers.json",
        TICKERS_URL,
        Some(TICKERS_MAX_AGE),
        &[("User-Agent", &agent)],
    )
    .await?;
    let body = tokio::task::spawn_blocking(move || std::fs::read_to_string(path))
        .await
        .map_err(|e| unavailable(format!("read task failed: {e}")))??;
    find_cik(&body, ticker)?.ok_or_else(|| AppError::NoData(format!("SEC EDGAR: unknown ticker '{ticker}'")))
}

// ---------------------------------------------------------------------------
// Extração (assíncrona só por causa da fonte; cada função devolve `None` quando
// falta um campo obrigatório, que o chamador transforma em `NoData`)
// ---------------------------------------------------------------------------

/// A linha mais recente **entre todas as tags que resolveram**, não a primeira
/// tag com dado: uma tag pode ter dado real que pára anos antes de uma tag nova
/// que cobre o mesmo conceito.
async fn try_tags(src: &dyn ConceptSource, cik: i64, tags: &[&str], how: Pick) -> Result<Option<Fact>, AppError> {
    let mut best: Option<Fact> = None;
    for tag in tags {
        let Some(rows) = src.concept(cik, tag).await? else { continue };
        let Some(row) = pick(&rows, how)? else { continue };
        if best.as_ref().map_or(true, |b| (&row.end, &row.filed) > (&b.end, &b.filed)) {
            best = Some(row);
        }
    }
    Ok(best)
}

async fn value(src: &dyn ConceptSource, cik: i64, tags: &[&str], how: Pick) -> Result<Option<f64>, AppError> {
    Ok(try_tags(src, cik, tags, how).await?.map(|row| row.val))
}

fn millions(v: f64) -> f64 {
    v / 1_000_000.0
}

#[derive(Debug, Clone, PartialEq)]
pub struct UsFundamentals {
    pub lpa: f64,
    pub vpa: f64,
    pub roe: f64,
    pub shares_outstanding: f64,
}

pub async fn fundamentals_with(src: &dyn ConceptSource, cik: i64) -> Result<Option<UsFundamentals>, AppError> {
    let Some(eps) = value(src, cik, &["EarningsPerShareDiluted"], Pick::Duration).await? else { return Ok(None) };
    let Some(equity) = value(src, cik, &["StockholdersEquity"], Pick::Instant(0)).await? else { return Ok(None) };
    let Some(net_income) = value(src, cik, &["NetIncomeLoss"], Pick::Duration).await? else { return Ok(None) };
    let Some(shares) = value(src, cik, &["CommonStockSharesOutstanding"], Pick::Instant(0)).await? else { return Ok(None) };
    if equity <= 0.0 || shares <= 0.0 {
        return Ok(None);
    }
    Ok(Some(UsFundamentals { lpa: eps, vpa: equity / shares, roe: net_income / equity * 100.0, shares_outstanding: shares }))
}

async fn instant_at(src: &dyn ConceptSource, cik: i64, tag: &str, rank: usize) -> Result<Option<f64>, AppError> {
    let Some(rows) = src.concept(cik, tag).await? else { return Ok(None) };
    Ok(pick(&rows, Pick::Instant(rank))?.map(|row| row.val))
}

/// ΔNWC = (a receber + estoque − fornecedores) do ano menos o do anterior, em milhões.
async fn nwc_change(src: &dyn ConceptSource, cik: i64) -> Result<Option<f64>, AppError> {
    let mut nwc = [0.0; 2];
    for (rank, slot) in nwc.iter_mut().enumerate() {
        let (Some(receivables), Some(inventory), Some(payables)) = (
            instant_at(src, cik, "AccountsReceivableNetCurrent", rank).await?,
            instant_at(src, cik, "InventoryNet", rank).await?,
            instant_at(src, cik, "AccountsPayableCurrent", rank).await?,
        ) else {
            return Ok(None);
        };
        *slot = receivables + inventory - payables;
    }
    Ok(Some(millions(nwc[0] - nwc[1])))
}

/// Imposto ÷ lucro antes do imposto, em %. `None` se falta uma das contas, o
/// lucro é ≤ 0 ou a razão sai de 0–100%.
async fn effective_tax_rate(src: &dyn ConceptSource, cik: i64) -> Result<Option<f64>, AppError> {
    let pretax = value(src, cik, &[PRETAX_INCOME_TAG], Pick::Duration).await?;
    let tax = value(src, cik, &["IncomeTaxExpenseBenefit"], Pick::Duration).await?;
    let (Some(pretax), Some(tax)) = (pretax, tax) else { return Ok(None) };
    if pretax <= 0.0 {
        return Ok(None);
    }
    let rate = tax / pretax * 100.0;
    Ok((0.0..=MAX_PLAUSIBLE_TAX_RATE).contains(&rate).then_some(rate))
}

pub async fn dcf_with(src: &dyn ConceptSource, cik: i64) -> Result<Option<DcfFundamentals>, AppError> {
    let Some(ebit) = try_tags(src, cik, &["OperatingIncomeLoss"], Pick::Duration).await? else { return Ok(None) };
    let Some(long_term_debt) = value(src, cik, &["LongTermDebtNoncurrent"], Pick::Instant(0)).await? else { return Ok(None) };
    let current_debt = value(src, cik, &["LongTermDebtCurrent"], Pick::Instant(0)).await?;
    let Some(cash) = value(src, cik, &["CashAndCashEquivalentsAtCarryingValue"], Pick::Instant(0)).await? else { return Ok(None) };
    let Some(revenue) = value(
        src,
        cik,
        &["Revenues", "RevenueFromContractWithCustomerExcludingAssessedTax"],
        Pick::Duration,
    )
    .await?
    else {
        return Ok(None);
    };
    let Some(inventory) = value(src, cik, &["InventoryNet"], Pick::Instant(0)).await? else { return Ok(None) };
    let Some(nwc) = nwc_change(src, cik).await? else { return Ok(None) };
    let Some(reference_year) = ebit.fy else { return Err(unavailable("EBIT row without fiscal year")) };

    let depreciation = value(
        src,
        cik,
        &[
            "DepreciationDepletionAndAmortization",
            "DepreciationAmortizationAndAccretionNet",
            "DepreciationAndAmortization",
            "Depreciation",
        ],
        Pick::Duration,
    )
    .await?;
    let capex = value(src, cik, &["PaymentsToAcquirePropertyPlantAndEquipment"], Pick::Duration).await?;

    Ok(Some(DcfFundamentals {
        reference_year,
        ebit: millions(ebit.val),
        tax_rate: effective_tax_rate(src, cik).await?,
        depreciation_amortization: depreciation.map(millions),
        capex: capex.map(millions),
        nwc_change: nwc,
        total_debt: millions(long_term_debt + current_debt.unwrap_or(0.0)),
        cash: millions(cash),
        revenue: millions(revenue),
        inventory: millions(inventory),
    }))
}

/// Valor anual por ano fiscal: ordena por `filed` (estável) e o último vence,
/// então a retificação mais recente substitui a original.
fn by_fiscal_year(rows: &[Fact]) -> Result<BTreeMap<i32, f64>, AppError> {
    let mut annual = annual_rows(rows)?;
    annual.sort_by(|a, b| a.filed.cmp(&b.filed));
    Ok(annual.into_iter().filter_map(|f| f.fy.map(|fy| (fy, f.val))).collect())
}

/// Payout médio dos últimos 5 anos fiscais que têm lucro e dividendos: soma os
/// dividendos e os lucros ano a ano e divide uma vez no fim (mesmo método do
/// `cvm_dfp`). Anos de prejuízo ficam de fora.
pub async fn payout_with(src: &dyn ConceptSource, cik: i64) -> Result<Option<f64>, AppError> {
    let Some(net_income_rows) = src.concept(cik, "NetIncomeLoss").await? else { return Ok(None) };
    if net_income_rows.is_empty() {
        return Ok(None);
    }
    let net_income = by_fiscal_year(&net_income_rows)?;

    // Mescla os anos das duas tags de dividendos em vez de "a primeira tag com
    // dado vence": uma tag pode ter dado só até um ano antigo e ainda assim
    // "ganhar". A ordem de `DIVIDEND_TAGS` só decide empate no mesmo ano: itera ao
    // contrário pra primária sobrescrever por último.
    let mut dividends: BTreeMap<i32, f64> = BTreeMap::new();
    for tag in DIVIDEND_TAGS.iter().rev() {
        if let Some(rows) = src.concept(cik, tag).await? {
            dividends.extend(by_fiscal_year(&rows)?);
        }
    }

    let years: Vec<i32> =
        net_income.keys().rev().filter(|y| dividends.contains_key(*y)).take(PAYOUT_YEARS_AVERAGED).copied().collect();
    if years.is_empty() {
        return Ok(None);
    }
    let profitable: Vec<i32> = years.into_iter().filter(|y| net_income[y] > 0.0).collect();
    let total_income: f64 = profitable.iter().map(|y| net_income[y]).sum();
    let total_dividends: f64 = profitable.iter().map(|y| dividends[y]).sum();
    if total_income <= 0.0 {
        return Ok(None);
    }
    Ok(Some(total_dividends / total_income * 100.0))
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReitFundamentals {
    pub reference_year: i32,
    pub revenue: f64,
    pub real_estate_property_net: Option<f64>,
    pub real_estate_property_at_cost: Option<f64>,
    pub stockholders_equity: f64,
    pub net_income: Option<f64>,
    pub eps_diluted: f64,
}

/// Indicadores imobiliários do ano fiscal mais recente. FFO/AFFO e taxa de
/// ocupação, que definem um REIT, **não são tags XBRL** em nenhuma taxonomia
/// (são não-GAAP, só em texto no 10-K); o que dá pra automatizar é receita,
/// valor dos imóveis, patrimônio, LPA e lucro. Obrigatórios: receita,
/// patrimônio e LPA. `NetIncomeLoss` é inconsistente entre REITs (o Simon
/// Property, um UPREIT, usa `ProfitLoss`), daí o fallback de tag.
pub async fn reit_with(src: &dyn ConceptSource, cik: i64) -> Result<Option<ReitFundamentals>, AppError> {
    let Some(revenue) = try_tags(
        src,
        cik,
        &["Revenues", "RevenueFromContractWithCustomerExcludingAssessedTax"],
        Pick::Duration,
    )
    .await?
    else {
        return Ok(None);
    };
    let Some(equity) = value(src, cik, &["StockholdersEquity"], Pick::Instant(0)).await? else { return Ok(None) };
    let Some(eps) = value(src, cik, &["EarningsPerShareDiluted"], Pick::Duration).await? else { return Ok(None) };
    let Some(reference_year) = revenue.fy else { return Err(unavailable("revenue row without fiscal year")) };
    Ok(Some(ReitFundamentals {
        reference_year,
        revenue: millions(revenue.val),
        real_estate_property_net: value(src, cik, &["RealEstateInvestmentPropertyNet"], Pick::Instant(0)).await?.map(millions),
        real_estate_property_at_cost: value(src, cik, &["RealEstateInvestmentPropertyAtCost"], Pick::Instant(0)).await?.map(millions),
        stockholders_equity: millions(equity),
        net_income: value(src, cik, &["NetIncomeLoss", "ProfitLoss"], Pick::Duration).await?.map(millions),
        eps_diluted: eps,
    }))
}

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------

pub trait UsFundamentalsProvider: DataProvider {
    fn fundamentals<'a>(&'a self, ticker: &'a str) -> BoxFuture<'a, Result<UsFundamentals, AppError>>;
    fn dcf_fundamentals<'a>(&'a self, ticker: &'a str) -> BoxFuture<'a, Result<DcfFundamentals, AppError>>;
    fn payout<'a>(&'a self, ticker: &'a str) -> BoxFuture<'a, Result<f64, AppError>>;
    /// Vazio quando o REIT não tem os campos obrigatórios (não é erro).
    fn reit_fundamentals<'a>(&'a self, ticker: &'a str) -> BoxFuture<'a, Result<Vec<ReitFundamentals>, AppError>>;
}

pub struct SecEdgar;

impl DataProvider for SecEdgar {
    fn id(&self) -> &'static str {
        SOURCE
    }
}

fn no_data(ticker: &str, what: &str) -> AppError {
    AppError::NoData(format!("SEC EDGAR: no {what} for '{ticker}'"))
}

impl UsFundamentalsProvider for SecEdgar {
    fn fundamentals<'a>(&'a self, ticker: &'a str) -> BoxFuture<'a, Result<UsFundamentals, AppError>> {
        Box::pin(async move {
            let cik = resolve_cik(ticker).await?;
            fundamentals_with(&SecHttp, cik).await?.ok_or_else(|| no_data(ticker, "fundamentals"))
        })
    }
    fn dcf_fundamentals<'a>(&'a self, ticker: &'a str) -> BoxFuture<'a, Result<DcfFundamentals, AppError>> {
        Box::pin(async move {
            let cik = resolve_cik(ticker).await?;
            dcf_with(&SecHttp, cik).await?.ok_or_else(|| no_data(ticker, "DCF fundamentals"))
        })
    }
    fn payout<'a>(&'a self, ticker: &'a str) -> BoxFuture<'a, Result<f64, AppError>> {
        Box::pin(async move {
            let cik = resolve_cik(ticker).await?;
            payout_with(&SecHttp, cik).await?.ok_or_else(|| no_data(ticker, "payout"))
        })
    }
    fn reit_fundamentals<'a>(&'a self, ticker: &'a str) -> BoxFuture<'a, Result<Vec<ReitFundamentals>, AppError>> {
        Box::pin(async move {
            let cik = resolve_cik(ticker).await?;
            Ok(reit_with(&SecHttp, cik).await?.into_iter().collect())
        })
    }
}

fn providers() -> Vec<Box<dyn UsFundamentalsProvider>> {
    vec![Box::new(SecEdgar)]
}

pub async fn fundamentals(ticker: &str) -> Result<Sourced<UsFundamentals>, AppError> {
    let providers = providers();
    let attempts = providers.iter().map(|p| (p.id(), p.fundamentals(ticker))).collect();
    cascade(&format!("US fundamentals for '{ticker}'"), attempts).await
}

pub async fn dcf_fundamentals(ticker: &str) -> Result<Sourced<DcfFundamentals>, AppError> {
    let providers = providers();
    let attempts = providers.iter().map(|p| (p.id(), p.dcf_fundamentals(ticker))).collect();
    cascade(&format!("US DCF fundamentals for '{ticker}'"), attempts).await
}

pub async fn payout(ticker: &str) -> Result<Sourced<f64>, AppError> {
    let providers = providers();
    let attempts = providers.iter().map(|p| (p.id(), p.payout(ticker))).collect();
    cascade(&format!("US payout for '{ticker}'"), attempts).await
}

pub async fn reit_fundamentals(ticker: &str) -> Result<Sourced<Vec<ReitFundamentals>>, AppError> {
    let providers = providers();
    let attempts = providers.iter().map(|p| (p.id(), p.reit_fundamentals(ticker))).collect();
    cascade(&format!("REIT fundamentals for '{ticker}'"), attempts).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Fonte falsa: tag → linhas; tag ausente = a empresa não reporta (404).
    struct Fake(HashMap<&'static str, Vec<Fact>>);

    impl ConceptSource for Fake {
        fn concept<'a>(&'a self, _cik: i64, tag: &'a str) -> BoxFuture<'a, Result<Option<Vec<Fact>>, AppError>> {
            Box::pin(async move { Ok(self.0.get(tag).cloned()) })
        }
    }

    fn duration(val: f64, fy: i32, start: &str, end: &str, filed: &str) -> Fact {
        Fact {
            start: Some(start.into()),
            end: end.into(),
            val,
            fy: Some(fy),
            fp: Some("FY".into()),
            form: Some("10-K".into()),
            filed: filed.into(),
        }
    }

    /// Ano fiscal 2025 completo, entregue em 2026-02-01.
    fn fy2025(val: f64) -> Fact {
        duration(val, 2025, "2025-01-01", "2025-12-31", "2026-02-01")
    }

    fn instant(val: f64, fy: i32, end: &str, filed: &str) -> Fact {
        Fact { start: None, end: end.into(), val, fy: Some(fy), fp: Some("FY".into()), form: Some("10-K".into()), filed: filed.into() }
    }

    fn inst2025(val: f64) -> Fact {
        instant(val, 2025, "2025-12-31", "2026-02-01")
    }

    fn inst2024(val: f64) -> Fact {
        instant(val, 2024, "2024-12-31", "2025-02-01")
    }

    fn fake(entries: Vec<(&'static str, Vec<Fact>)>) -> Fake {
        Fake(entries.into_iter().collect())
    }

    // --- parsing e seleção ---------------------------------------------------

    #[test]
    fn concept_picks_the_first_present_unit() {
        let row = r#"{"end":"2025-12-31","val":1.5,"fy":2025,"fp":"FY","form":"10-K","filed":"2026-02-01","accn":"x"}"#;
        let usd = format!(r#"{{"units":{{"USD/shares":[{row}],"shares":[{row},{row}]}}}}"#);
        assert_eq!(parse_concept(&usd).unwrap().unwrap().len(), 1); // USD/shares vence shares
        assert_eq!(parse_concept(r#"{"units":{}}"#).unwrap(), None);
        assert_eq!(parse_concept(r#"{"units":{"pure":[]}}"#).unwrap(), None);
        assert!(matches!(parse_concept("<html>"), Err(AppError::ProviderUnavailable(_))));
    }

    #[test]
    fn find_cik_is_case_insensitive_and_prefers_the_lowest_index() {
        let body = r#"{"0":{"cik_str":320193,"ticker":"AAPL","title":"Apple"},"1":{"cik_str":19617,"ticker":"JPM","title":"JPM"},
                       "10":{"cik_str":999,"ticker":"AAPL","title":"Dup"}}"#;
        assert_eq!(find_cik(body, "aapl").unwrap(), Some(320193));
        assert_eq!(find_cik(body, "jpm").unwrap(), Some(19617));
        assert_eq!(find_cik(body, "NOPE").unwrap(), None);
    }

    #[test]
    fn only_annual_10k_rows_with_a_350_to_380_day_window_count() {
        let rows = vec![
            duration(1.0, 2025, "2025-10-01", "2025-12-31", "2026-02-01"), // trimestre (nota do 10-K)
            duration(2.0, 2025, "2025-04-01", "2025-12-31", "2026-02-01"), // 9 meses
            duration(3.0, 2025, "2025-01-01", "2025-12-31", "2026-02-01"), // 364 dias: anual
            Fact { form: Some("10-Q".into()), ..duration(4.0, 2025, "2025-01-01", "2025-12-31", "2026-05-01") },
            Fact { fp: Some("Q3".into()), ..duration(5.0, 2025, "2025-01-01", "2025-12-31", "2026-05-01") },
        ];
        assert_eq!(pick(&rows, Pick::Duration).unwrap().unwrap().val, 3.0);
        assert!(is_annual_duration(&duration(0.0, 2025, "2025-01-01", "2026-01-15", "x")).unwrap()); // 379 dias
        assert!(!is_annual_duration(&duration(0.0, 2025, "2025-01-01", "2026-01-20", "x")).unwrap()); // 384 dias
        assert!(is_annual_duration(&duration(0.0, 2025, "2025-01-05", "2025-12-31", "x")).unwrap()); // 360 dias
        assert!(!is_annual_duration(&duration(0.0, 2025, "2025-01-20", "2025-12-31", "x")).unwrap()); // 345 dias
    }

    #[test]
    fn an_amendment_filed_later_wins_for_the_same_period() {
        let rows = vec![
            duration(10.0, 2025, "2025-01-01", "2025-12-31", "2026-02-01"),
            duration(11.0, 2025, "2025-01-01", "2025-12-31", "2026-04-01"), // 10-K/A
            duration(9.0, 2025, "2025-01-01", "2025-12-31", "2026-03-01"),
        ];
        assert_eq!(pick(&rows, Pick::Duration).unwrap().unwrap().val, 11.0);
    }

    #[test]
    fn instants_rank_by_fiscal_year_end() {
        let rows = vec![inst2024(5.0), inst2025(7.0), instant(1.0, 2023, "2023-12-31", "2024-02-01")];
        assert_eq!(pick(&rows, Pick::Instant(0)).unwrap().unwrap().val, 7.0);
        assert_eq!(pick(&rows, Pick::Instant(1)).unwrap().unwrap().val, 5.0);
        assert_eq!(pick(&rows, Pick::Instant(3)).unwrap(), None);
    }

    #[test]
    fn ties_keep_the_first_row_like_python_max() {
        let a = duration(1.0, 2025, "2025-01-01", "2025-12-31", "2026-02-01");
        let b = duration(2.0, 2025, "2025-01-01", "2025-12-31", "2026-02-01");
        assert_eq!(pick(&[a, b], Pick::Duration).unwrap().unwrap().val, 1.0);
    }

    #[tokio::test]
    async fn try_tags_returns_the_most_recent_row_among_all_tags() {
        // A tag antiga tem dado até 2023; a nova cobre 2025: vence a nova, mesmo vindo depois na lista.
        let old = duration(100.0, 2023, "2023-01-01", "2023-12-31", "2024-02-01");
        let src = fake(vec![("OldTag", vec![old]), ("NewTag", vec![fy2025(200.0)])]);
        let got = try_tags(&src, 1, &["OldTag", "NewTag"], Pick::Duration).await.unwrap().unwrap();
        assert_eq!(got.val, 200.0);
        assert_eq!(try_tags(&src, 1, &["Missing"], Pick::Duration).await.unwrap(), None);
    }

    // --- fundamentos -----------------------------------------------------------

    fn fundamentals_source(equity: f64, shares: f64) -> Fake {
        fake(vec![
            ("EarningsPerShareDiluted", vec![fy2025(6.5)]),
            ("StockholdersEquity", vec![inst2025(equity)]),
            ("NetIncomeLoss", vec![fy2025(100_000_000.0)]),
            ("CommonStockSharesOutstanding", vec![inst2025(shares)]),
        ])
    }

    #[tokio::test]
    async fn fundamentals_are_computed_from_the_four_tags() {
        let f = fundamentals_with(&fundamentals_source(500_000_000.0, 10_000_000.0), 1).await.unwrap().unwrap();
        assert_eq!(f, UsFundamentals { lpa: 6.5, vpa: 50.0, roe: 20.0, shares_outstanding: 10_000_000.0 });
    }

    #[tokio::test]
    async fn fundamentals_need_positive_equity_and_shares_and_every_tag() {
        assert_eq!(fundamentals_with(&fundamentals_source(-1.0, 10.0), 1).await.unwrap(), None);
        assert_eq!(fundamentals_with(&fundamentals_source(10.0, 0.0), 1).await.unwrap(), None);
        assert_eq!(fundamentals_with(&fake(vec![]), 1).await.unwrap(), None);
    }

    // --- DCF -------------------------------------------------------------------

    fn dcf_source() -> Vec<(&'static str, Vec<Fact>)> {
        vec![
            ("OperatingIncomeLoss", vec![fy2025(50_000_000.0)]),
            ("Revenues", vec![fy2025(100_000_000.0)]),
            ("LongTermDebtNoncurrent", vec![inst2025(25_000_000.0)]),
            ("LongTermDebtCurrent", vec![inst2025(15_000_000.0)]),
            ("CashAndCashEquivalentsAtCarryingValue", vec![inst2025(25_000_000.0)]),
            ("InventoryNet", vec![inst2025(8_000_000.0), inst2024(7_000_000.0)]),
            ("AccountsReceivableNetCurrent", vec![inst2025(10_000_000.0), inst2024(9_000_000.0)]),
            ("AccountsPayableCurrent", vec![inst2025(6_000_000.0), inst2024(5_000_000.0)]),
            (PRETAX_INCOME_TAG, vec![fy2025(40_000_000.0)]),
            ("IncomeTaxExpenseBenefit", vec![fy2025(10_000_000.0)]),
            ("DepreciationDepletionAndAmortization", vec![fy2025(3_000_000.0)]),
            ("PaymentsToAcquirePropertyPlantAndEquipment", vec![fy2025(4_000_000.0)]),
        ]
    }

    #[tokio::test]
    async fn dcf_computes_all_nine_fields_in_millions() {
        let dcf = dcf_with(&fake(dcf_source()), 1).await.unwrap().unwrap();
        assert_eq!(dcf.reference_year, 2025);
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
    async fn dcf_is_unavailable_without_ebit_which_is_the_bank_case() {
        let mut entries = dcf_source();
        entries.retain(|(tag, _)| *tag != "OperatingIncomeLoss");
        assert_eq!(dcf_with(&fake(entries), 1).await.unwrap(), None);
    }

    #[tokio::test]
    async fn dcf_keeps_going_without_da_capex_or_current_debt_but_not_without_a_required_tag() {
        let mut entries = dcf_source();
        entries.retain(|(tag, _)| !matches!(*tag, "DepreciationDepletionAndAmortization" | "PaymentsToAcquirePropertyPlantAndEquipment" | "LongTermDebtCurrent"));
        let dcf = dcf_with(&fake(entries), 1).await.unwrap().unwrap();
        assert_eq!((dcf.depreciation_amortization, dcf.capex, dcf.total_debt), (None, None, 25.0));

        let mut entries = dcf_source();
        entries.retain(|(tag, _)| *tag != "AccountsPayableCurrent");
        assert_eq!(dcf_with(&fake(entries), 1).await.unwrap(), None);
    }

    #[tokio::test]
    async fn the_tax_rate_is_none_for_losses_and_implausible_ratios() {
        for (pretax, tax) in [(-5.0, 1.0), (0.0, 1.0), (100.0, 263.0), (100.0, -10.0)] {
            let src = fake(vec![(PRETAX_INCOME_TAG, vec![fy2025(pretax)]), ("IncomeTaxExpenseBenefit", vec![fy2025(tax)])]);
            assert_eq!(effective_tax_rate(&src, 1).await.unwrap(), None, "{pretax} {tax}");
        }
        assert_eq!(effective_tax_rate(&fake(vec![]), 1).await.unwrap(), None);
    }

    // --- payout ----------------------------------------------------------------

    fn fy(year: i32, val: f64, filed: &str) -> Fact {
        duration(val, year, &format!("{year}-01-01"), &format!("{year}-12-31"), filed)
    }

    #[tokio::test]
    async fn payout_sums_dividends_and_income_across_years() {
        let src = fake(vec![
            ("NetIncomeLoss", vec![fy(2024, 100.0, "2025-02-01"), fy(2025, 100.0, "2026-02-01")]),
            ("PaymentsOfDividendsCommonStock", vec![fy(2024, 40.0, "2025-02-01"), fy(2025, 20.0, "2026-02-01")]),
        ]);
        assert_eq!(payout_with(&src, 1).await.unwrap(), Some(30.0)); // (20+40)/(100+100)
    }

    #[tokio::test]
    async fn payout_merges_both_dividend_tags_with_the_primary_winning_ties() {
        let src = fake(vec![
            ("NetIncomeLoss", vec![fy(2023, 100.0, "2024-02-01"), fy(2024, 100.0, "2025-02-01"), fy(2025, 100.0, "2026-02-01")]),
            // A primária só vai até 2023; a secundária cobre 2024–2025 e também 2023 (que a primária sobrescreve).
            ("PaymentsOfDividendsCommonStock", vec![fy(2023, 10.0, "2024-02-01")]),
            ("PaymentsOfDividends", vec![fy(2023, 99.0, "2024-02-01"), fy(2024, 20.0, "2025-02-01"), fy(2025, 30.0, "2026-02-01")]),
        ]);
        assert_eq!(payout_with(&src, 1).await.unwrap(), Some((10.0 + 20.0 + 30.0) / 300.0 * 100.0));
    }

    #[tokio::test]
    async fn payout_skips_loss_years_and_needs_data() {
        let src = fake(vec![
            ("NetIncomeLoss", vec![fy(2024, -50.0, "2025-02-01"), fy(2025, 100.0, "2026-02-01")]),
            ("PaymentsOfDividendsCommonStock", vec![fy(2024, 30.0, "2025-02-01"), fy(2025, 25.0, "2026-02-01")]),
        ]);
        assert_eq!(payout_with(&src, 1).await.unwrap(), Some(25.0)); // 2024 (prejuízo) fica de fora
        assert_eq!(payout_with(&fake(vec![]), 1).await.unwrap(), None); // sem NetIncomeLoss
        let no_dividends = fake(vec![("NetIncomeLoss", vec![fy(2025, 100.0, "2026-02-01")])]);
        assert_eq!(payout_with(&no_dividends, 1).await.unwrap(), None);
        let all_losses = fake(vec![
            ("NetIncomeLoss", vec![fy(2025, -1.0, "2026-02-01")]),
            ("PaymentsOfDividendsCommonStock", vec![fy(2025, 1.0, "2026-02-01")]),
        ]);
        assert_eq!(payout_with(&all_losses, 1).await.unwrap(), None);
    }

    #[tokio::test]
    async fn payout_only_looks_at_the_five_most_recent_common_years() {
        let years = 2018..=2025;
        let income: Vec<Fact> = years.clone().map(|y| fy(y, 100.0, &format!("{}-02-01", y + 1))).collect();
        let dividends: Vec<Fact> = years.map(|y| fy(y, if y >= 2021 { 10.0 } else { 1000.0 }, &format!("{}-02-01", y + 1))).collect();
        let src = fake(vec![("NetIncomeLoss", income), ("PaymentsOfDividendsCommonStock", dividends)]);
        assert_eq!(payout_with(&src, 1).await.unwrap(), Some(10.0)); // 2021..2025
    }

    // --- REIT ------------------------------------------------------------------

    fn reit_source() -> Vec<(&'static str, Vec<Fact>)> {
        vec![
            ("Revenues", vec![fy2025(4_700_000_000.0)]),
            ("StockholdersEquity", vec![inst2025(38_000_000_000.0)]),
            ("EarningsPerShareDiluted", vec![fy2025(1.3)]),
            ("RealEstateInvestmentPropertyNet", vec![inst2025(50_000_000_000.0)]),
            ("RealEstateInvestmentPropertyAtCost", vec![inst2025(55_000_000_000.0)]),
            ("NetIncomeLoss", vec![fy2025(1_000_000_000.0)]),
        ]
    }

    #[tokio::test]
    async fn reit_fundamentals_are_computed_in_millions() {
        let r = reit_with(&fake(reit_source()), 1).await.unwrap().unwrap();
        assert_eq!(
            r,
            ReitFundamentals {
                reference_year: 2025,
                revenue: 4700.0,
                real_estate_property_net: Some(50000.0),
                real_estate_property_at_cost: Some(55000.0),
                stockholders_equity: 38000.0,
                net_income: Some(1000.0),
                eps_diluted: 1.3,
            }
        );
    }

    #[tokio::test]
    async fn reit_falls_back_to_profit_loss_and_needs_its_required_tags() {
        let mut entries = reit_source();
        entries.retain(|(tag, _)| !matches!(*tag, "NetIncomeLoss" | "RealEstateInvestmentPropertyNet"));
        entries.push(("ProfitLoss", vec![fy2025(2_000_000_000.0)]));
        let r = reit_with(&fake(entries), 1).await.unwrap().unwrap();
        assert_eq!((r.net_income, r.real_estate_property_net), (Some(2000.0), None));

        let mut entries = reit_source();
        entries.retain(|(tag, _)| *tag != "Revenues");
        assert_eq!(reit_with(&fake(entries), 1).await.unwrap(), None);
    }

    // --- throttle e credencial ---------------------------------------------------

    #[test]
    fn calls_are_spaced_by_reserving_consecutive_slots() {
        let t0 = Instant::now();
        let first = next_slot(None, t0);
        assert_eq!(first, t0);
        // Duas chamadas no mesmo instante: a segunda reserva depois da primeira + intervalo.
        let second = next_slot(Some(first + REQUEST_INTERVAL), t0);
        assert_eq!(second, t0 + REQUEST_INTERVAL);
        // Depois de um tempo parado, não acumula atraso.
        let later = t0 + Duration::from_secs(5);
        assert_eq!(next_slot(Some(second + REQUEST_INTERVAL), later), later);
    }

    #[test]
    fn the_contact_email_is_read_from_the_keyring_or_this_env_var() {
        assert_eq!(SEC_EDGAR_EMAIL.env_var, "SEC_EDGAR_CONTACT_EMAIL");
        assert_eq!(SEC_EDGAR_EMAIL.keyring_user, "sec_edgar_contact_email");
    }

    // Ao vivo (rede): precisa de `SEC_EDGAR_CONTACT_EMAIL` no ambiente.
    // `cargo test --lib -- --ignored --nocapture finance_core::sec_edgar`.
    #[tokio::test]
    #[ignore]
    async fn live_apple_a_bank_and_a_reit() {
        for ticker in ["AAPL", "JPM", "O"] {
            let started = std::time::Instant::now();
            println!("{ticker} fundamentals {:?}", fundamentals(ticker).await.map(|s| s.value));
            println!("{ticker} payout {:?}", payout(ticker).await.map(|s| s.value));
            println!("{ticker} dcf {:?}", dcf_fundamentals(ticker).await.map(|s| s.value));
            println!("{ticker} reit {:?} ({:?})", reit_fundamentals(ticker).await.map(|s| s.value), started.elapsed());
        }
        assert!(matches!(resolve_cik("ZZZNOPE9").await.unwrap_err(), AppError::NoData(_)));
    }
}
