// Fase 31 — fontes públicas de indicadores cripto coletadas direto pelo
// Anchor (sem passar pelo Lume): derivativos de ETH na Binance Futures e o
// staking yield real (APY do stETH na DefiLlama menos a inflação bruta do
// ETH no CoinMetrics Community). Todas sem chave. Os cálculos são funções
// puras (testadas com JSON copiado das respostas reais); os `fetch_*` são
// finos.
//
// Binance restringe por região: se uma chamada falhar, o erro diz qual
// fonte caiu, e o coletor segue com os demais indicadores.

use std::future::Future;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Deserialize;

use crate::error::AppError;
use crate::finance_core::http;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const BINANCE_FAPI: &str = "https://fapi.binance.com";
const BYBIT_API: &str = "https://api.bybit.com";
const DERIBIT_API: &str = "https://www.deribit.com/api/v2";
const DEFILLAMA_STABLECOINS: &str = "https://stablecoins.llama.fi";
const DEFILLAMA_YIELDS: &str = "https://yields.llama.fi";
const COINMETRICS_COMMUNITY: &str = "https://community-api.coinmetrics.io/v4";
const COINGECKO_API: &str = "https://api.coingecko.com/api/v3";
/// SOPR e NUPL do BTC. Plano grátis: 10 chamadas por hora e 15 por dia. O plano documenta
/// dado com 7 dias de atraso, mas medido ao vivo em 2026-10-31 a defasagem era de 1 dia (o
/// `d` da resposta é a data real do dado; o atraso varia, então confie no `d`).
const BITCOIN_DATA_API: &str = "https://bitcoin-data.com/v1";
/// Pool stETH (Lido, Ethereum) na DefiLlama — o id é estável.
const STETH_POOL_ID: &str = "747c1d2a-c668-4682-b9f9-296708a3dd90";

/// Funding da Binance sai a cada 8h: 3 por dia, 7 dias.
const FUNDING_PRINTS_PER_DAY: f64 = 3.0;
const FUNDING_WINDOW: usize = 21;
/// A Binance só guarda ~30 dias de histórico de OI e de long/short.
const HISTORY_DAYS: usize = 30;
const LONG_SHORT_WINDOW: usize = 7;

// ---------------------------------------------------------------------------
// Cálculos puros
// ---------------------------------------------------------------------------

fn mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    Some(values.iter().sum::<f64>() / values.len() as f64)
}

/// Média das taxas de funding por período de 8h, anualizada em percentual
/// (ex.: 0,0001 por 8h = 10,95% ao ano).
pub fn funding_annualized_pct(rates: &[f64]) -> Option<f64> {
    mean(rates).map(|avg| avg * FUNDING_PRINTS_PER_DAY * 365.0 * 100.0)
}

/// Variação percentual entre o ponto mais antigo e o mais recente.
pub fn pct_change(oldest: f64, newest: f64) -> Option<f64> {
    if oldest.abs() < f64::EPSILON {
        return None;
    }
    Some((newest / oldest - 1.0) * 100.0)
}

/// Yield real do staking: APY nominal menos a inflação bruta anual do ETH
/// (emissão diária × 365 ÷ oferta circulante), tudo em percentual.
pub fn real_staking_yield(apy_pct: f64, daily_issuance: f64, supply: f64) -> Option<f64> {
    if supply <= 0.0 {
        return None;
    }
    Some(apy_pct - daily_issuance * 365.0 / supply * 100.0)
}

// ---------------------------------------------------------------------------
// Parsers (JSON → números)
// ---------------------------------------------------------------------------

fn source_err(source: &str, detail: impl std::fmt::Display) -> AppError {
    AppError::CryptoSource(format!("{source}: {detail}"))
}

fn parse_f64(source: &str, field: &str, value: &str) -> Result<f64, AppError> {
    value
        .parse::<f64>()
        .map_err(|_| source_err(source, format!("invalid number in '{field}': {value}")))
}

#[derive(Deserialize)]
struct FundingRow {
    #[serde(rename = "fundingRate")]
    funding_rate: String,
}

#[derive(Deserialize)]
struct OpenInterestRow {
    #[serde(rename = "sumOpenInterest")]
    sum_open_interest: String,
}

#[derive(Deserialize)]
struct LongShortRow {
    #[serde(rename = "longShortRatio")]
    long_short_ratio: String,
}

pub fn parse_funding_rates(body: &str) -> Result<Vec<f64>, AppError> {
    let rows: Vec<FundingRow> = serde_json::from_str(body)?;
    rows.iter().map(|r| parse_f64("Binance funding", "fundingRate", &r.funding_rate)).collect()
}

/// Open interest em unidades do ativo (ETH), do mais antigo ao mais recente.
pub fn parse_open_interest(body: &str) -> Result<Vec<f64>, AppError> {
    let rows: Vec<OpenInterestRow> = serde_json::from_str(body)?;
    rows.iter()
        .map(|r| parse_f64("Binance open interest", "sumOpenInterest", &r.sum_open_interest))
        .collect()
}

pub fn parse_long_short(body: &str) -> Result<Vec<f64>, AppError> {
    let rows: Vec<LongShortRow> = serde_json::from_str(body)?;
    rows.iter()
        .map(|r| parse_f64("Binance long/short", "longShortRatio", &r.long_short_ratio))
        .collect()
}

/// Quantos pontos da série entram na mediana. O Lido reportou 15,07% num único dia
/// (2026-10-30) contra ~2,25% no normal: o último ponto sozinho gravaria o pico como se
/// fosse o yield. A mediana de 7 aguenta até 3 pontos ruins.
const STETH_APY_WINDOW: usize = 7;
/// Idade máxima do último ponto. O normal é menos de 1 dia.
const STETH_APY_MAX_AGE_DAYS: i64 = 7;

/// Mediana do `apy` (em %) dos 7 últimos pontos da série do pool stETH. Série com menos de 7
/// pontos, `apy` ausente, nulo, texto, negativo ou não finito em qualquer um dos 7, ou último
/// ponto velho demais é erro. Mesmo critério do celular (`defillama_yields_service.dart`).
pub fn parse_steth_apy(body: &str, today: chrono::NaiveDate) -> Result<f64, AppError> {
    const SOURCE: &str = "DefiLlama yields";
    let json: serde_json::Value = serde_json::from_str(body).map_err(|e| source_err(SOURCE, e))?;
    let data = json
        .get("data")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| source_err(SOURCE, "unexpected response"))?;
    if data.len() < STETH_APY_WINDOW {
        return Err(source_err(SOURCE, "not enough history"));
    }
    let window = &data[data.len() - STETH_APY_WINDOW..];

    let mut apys = Vec::with_capacity(STETH_APY_WINDOW);
    for point in window {
        let apy = point
            .get("apy")
            .and_then(serde_json::Value::as_f64)
            .filter(|v| v.is_finite() && *v >= 0.0)
            .ok_or_else(|| source_err(SOURCE, "unexpected response"))?;
        apys.push(apy);
    }

    // `timestamp` vem como `2026-10-31T12:01:50.401Z`; só a data importa.
    let last_day = window
        .last()
        .and_then(|p| p.get("timestamp"))
        .and_then(serde_json::Value::as_str)
        .and_then(|t| t.get(..10))
        .and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        .ok_or_else(|| source_err(SOURCE, "last point has no valid 'timestamp'"))?;
    let age = (today - last_day).num_days();
    if age > STETH_APY_MAX_AGE_DAYS {
        return Err(source_err(SOURCE, format!("stale data, latest point is {last_day} ({age} days old)")));
    }

    apys.sort_by(f64::total_cmp);
    Ok(apys[STETH_APY_WINDOW / 2])
}

#[derive(Deserialize)]
struct CoinMetricsResponse {
    data: Vec<CoinMetricsRow>,
}

#[derive(Deserialize)]
struct CoinMetricsRow {
    #[serde(rename = "IssTotNtv")]
    issuance: String,
    #[serde(rename = "SplyCur")]
    supply: String,
}

/// `(emissão diária em ETH, oferta circulante)` do dia mais recente.
pub fn parse_issuance_and_supply(body: &str) -> Result<(f64, f64), AppError> {
    let response: CoinMetricsResponse = serde_json::from_str(body)?;
    let row = response
        .data
        .last()
        .ok_or_else(|| source_err("CoinMetrics", "empty IssTotNtv/SplyCur series"))?;
    Ok((
        parse_f64("CoinMetrics", "IssTotNtv", &row.issuance)?,
        parse_f64("CoinMetrics", "SplyCur", &row.supply)?,
    ))
}

/// Razão entre o open interest de puts e o de calls. `None` sem calls.
pub fn put_call_ratio(open_interest: &[(bool, f64)]) -> Option<f64> {
    let puts: f64 = open_interest.iter().filter(|(is_put, _)| *is_put).map(|(_, oi)| oi).sum();
    let calls: f64 = open_interest.iter().filter(|(is_put, _)| !*is_put).map(|(_, oi)| oi).sum();
    if calls <= 0.0 {
        return None;
    }
    Some(puts / calls)
}

// ---------------------------------------------------------------------------
// Parsers — Bybit (fallback da Binance)
// ---------------------------------------------------------------------------
//
// ATENÇÃO: a Bybit devolve as séries do mais NOVO pro mais antigo (a Binance
// é o contrário). Todos os parsers abaixo devolvem em ordem cronológica.

#[derive(Deserialize)]
struct BybitEnvelope<T> {
    #[serde(rename = "retCode")]
    ret_code: i64,
    #[serde(rename = "retMsg")]
    ret_msg: String,
    result: Option<BybitList<T>>,
}

#[derive(Deserialize)]
struct BybitList<T> {
    /// Em resposta de erro o `result` vem `{}`, sem `list`.
    #[serde(default = "Vec::new")]
    list: Vec<T>,
}

/// Linhas da Bybit já em ordem cronológica (do mais antigo ao mais novo).
fn bybit_rows<T: DeserializeOwned>(source: &str, body: &str) -> Result<Vec<T>, AppError> {
    let envelope: BybitEnvelope<T> =
        serde_json::from_str(body).map_err(|e| source_err(source, e))?;
    if envelope.ret_code != 0 {
        return Err(source_err(source, envelope.ret_msg));
    }
    let mut rows = envelope.result.map(|r| r.list).unwrap_or_default();
    rows.reverse();
    Ok(rows)
}

#[derive(Deserialize)]
struct BybitFundingRow {
    #[serde(rename = "fundingRate")]
    funding_rate: String,
}

#[derive(Deserialize)]
struct BybitOpenInterestRow {
    #[serde(rename = "openInterest")]
    open_interest: String,
}

#[derive(Deserialize)]
struct BybitRatioRow {
    #[serde(rename = "buyRatio")]
    buy_ratio: String,
    #[serde(rename = "sellRatio")]
    sell_ratio: String,
}

pub fn parse_bybit_funding(body: &str) -> Result<Vec<f64>, AppError> {
    bybit_rows::<BybitFundingRow>("Bybit funding", body)?
        .iter()
        .map(|r| parse_f64("Bybit funding", "fundingRate", &r.funding_rate))
        .collect()
}

pub fn parse_bybit_open_interest(body: &str) -> Result<Vec<f64>, AppError> {
    bybit_rows::<BybitOpenInterestRow>("Bybit open interest", body)?
        .iter()
        .map(|r| parse_f64("Bybit open interest", "openInterest", &r.open_interest))
        .collect()
}

/// Razão long/short = proporção de contas compradas ÷ vendidas.
pub fn parse_bybit_long_short(body: &str) -> Result<Vec<f64>, AppError> {
    bybit_rows::<BybitRatioRow>("Bybit long/short", body)?
        .iter()
        .map(|r| {
            let buy = parse_f64("Bybit long/short", "buyRatio", &r.buy_ratio)?;
            let sell = parse_f64("Bybit long/short", "sellRatio", &r.sell_ratio)?;
            if sell <= 0.0 {
                return Err(source_err("Bybit long/short", "sellRatio is zero"));
            }
            Ok(buy / sell)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Parsers — Deribit (opções) e DefiLlama (stablecoins)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct DeribitEnvelope<T> {
    result: T,
}

#[derive(Deserialize)]
struct DvolResult {
    /// Linhas `[timestamp, open, high, low, close]`, do mais antigo ao mais novo.
    data: Vec<Vec<f64>>,
}

/// Fechamento mais recente do DVOL.
pub fn parse_dvol_latest(body: &str) -> Result<f64, AppError> {
    let envelope: DeribitEnvelope<DvolResult> =
        serde_json::from_str(body).map_err(|e| source_err("Deribit DVOL", e))?;
    envelope
        .result
        .data
        .last()
        .and_then(|row| row.get(4).copied())
        .ok_or_else(|| source_err("Deribit DVOL", "empty volatility index series"))
}

#[derive(Deserialize)]
struct OptionSummary {
    instrument_name: String,
    open_interest: f64,
}

/// `(é put, open interest)` por instrumento; o que não termina em `-P`/`-C`
/// é ignorado.
pub fn parse_option_open_interest(body: &str) -> Result<Vec<(bool, f64)>, AppError> {
    let envelope: DeribitEnvelope<Vec<OptionSummary>> =
        serde_json::from_str(body).map_err(|e| source_err("Deribit options", e))?;
    Ok(envelope
        .result
        .into_iter()
        .filter_map(|o| {
            if o.instrument_name.ends_with("-P") {
                Some((true, o.open_interest))
            } else if o.instrument_name.ends_with("-C") {
                Some((false, o.open_interest))
            } else {
                None
            }
        })
        .collect())
}

#[derive(Deserialize)]
struct StablecoinPoint {
    /// Segundos Unix, como texto.
    date: String,
    #[serde(rename = "totalCirculatingUSD")]
    total: StablecoinTotal,
}

#[derive(Deserialize)]
struct StablecoinTotal {
    #[serde(rename = "peggedUSD")]
    pegged_usd: f64,
}

const STABLECOIN_WINDOW_SECS: i64 = 30 * 86_400;

/// Variação % do total circulante de stablecoins (USD) nos últimos 30 dias.
/// A base é o último ponto **até** 30 dias antes do mais recente (a série
/// tem falhas, então não dá pra contar 30 posições).
pub fn parse_stablecoin_change_30d(body: &str) -> Result<f64, AppError> {
    let points: Vec<StablecoinPoint> =
        serde_json::from_str(body).map_err(|e| source_err("DefiLlama stablecoins", e))?;
    let dated: Vec<(i64, f64)> = points
        .iter()
        .filter_map(|p| p.date.parse::<i64>().ok().map(|d| (d, p.total.pegged_usd)))
        .collect();
    let (last_date, last_total) = *dated
        .last()
        .ok_or_else(|| source_err("DefiLlama stablecoins", "empty series"))?;
    let baseline = dated
        .iter()
        .rev()
        .find(|(date, _)| *date <= last_date - STABLECOIN_WINDOW_SECS)
        .ok_or_else(|| source_err("DefiLlama stablecoins", "less than 30 days of history"))?
        .1;
    pct_change(baseline, last_total)
        .ok_or_else(|| source_err("DefiLlama stablecoins", "zero supply at the start"))
}

#[derive(Deserialize)]
struct GlobalMarket {
    data: GlobalMarketData,
}

#[derive(Deserialize)]
struct GlobalMarketData {
    market_cap_percentage: std::collections::HashMap<String, f64>,
}

/// Dominância do BTC (% do cap. total do mercado), do `/global` da CoinGecko.
/// Só o valor de hoje: a fonte não tem série grátis, então não há tendência.
pub fn parse_btc_dominance(body: &str) -> Result<f64, AppError> {
    let global: GlobalMarket = serde_json::from_str(body).map_err(|e| source_err("CoinGecko global", e))?;
    let btc = global
        .data
        .market_cap_percentage
        .get("btc")
        .copied()
        .ok_or_else(|| source_err("CoinGecko global", "missing market_cap_percentage.btc"))?;
    if !btc.is_finite() || !(0.0..=100.0).contains(&btc) {
        return Err(source_err("CoinGecko global", format!("implausible dominance {btc}")));
    }
    Ok(btc)
}

/// Leitura de uma fonte que informa a data do dado (que não é a de hoje).
#[derive(Clone, Debug, PartialEq)]
pub struct DatedReading {
    pub value: f64,
    /// `YYYY-MM-DD`, a data do dado segundo a fonte.
    pub date: String,
}

/// Faixa em que o valor é plausível; fora dela a fonte provavelmente mudou de formato.
fn bitcoin_data_range(field: &str) -> Option<(f64, f64)> {
    match field {
        "sopr" => Some((0.0, 10.0)),
        "nupl" => Some((-1.0, 1.0)),
        _ => None,
    }
}

/// Resposta de `/v1/{field}/last` do bitcoin-data.com, ex.:
/// `{"d":"2026-10-19","unixTs":1792368000,"sopr":1.0057,"delayed":true,...}`.
/// `field` é `sopr` ou `nupl`.
pub fn parse_bitcoin_data(body: &str, field: &str) -> Result<DatedReading, AppError> {
    let source = "bitcoin-data.com";
    let (min, max) = bitcoin_data_range(field)
        .ok_or_else(|| source_err(source, format!("unsupported field '{field}'")))?;
    let json: serde_json::Value = serde_json::from_str(body).map_err(|e| source_err(source, e))?;
    let value = json
        .get(field)
        .and_then(serde_json::Value::as_f64)
        .ok_or_else(|| source_err(source, format!("missing numeric '{field}'")))?;
    // SOPR é razão (> 0) e NUPL fica em [-1, 1]; os limites são exclusivos no SOPR.
    if !value.is_finite() || value < min || value > max || (field == "sopr" && value == min) {
        return Err(source_err(source, format!("implausible {field} {value}")));
    }
    let date = json
        .get("d")
        .and_then(serde_json::Value::as_str)
        .filter(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").is_ok())
        .ok_or_else(|| source_err(source, "missing or invalid 'd' date"))?;
    Ok(DatedReading { value, date: date.to_string() })
}

// ---------------------------------------------------------------------------
// Fetch
// ---------------------------------------------------------------------------

/// Delega ao cliente compartilhado; a mensagem já vem prefixada com a fonte,
/// e o erro segue como `CryptoSource` (o código `CRYPTO_SOURCE_ERROR` é do contrato da tela).
async fn get_text(source: &str, url: &str) -> Result<String, AppError> {
    http::get_text(source, url, REQUEST_TIMEOUT).await.map_err(|err| match err {
        AppError::NoData(detail) | AppError::ProviderUnavailable(detail) => AppError::CryptoSource(detail),
        other => AppError::CryptoSource(format!("{source}: {other}")),
    })
}

/// Valor e a fonte que de fato respondeu (gravada em `crypto_indicators.source`).
pub type Sourced = (f64, &'static str);

/// Tenta a Binance e, se falhar (rede, HTTP ou bloqueio de região), a Bybit.
/// Só dá erro se as duas falharem, e a mensagem traz os dois motivos.
async fn binance_then_bybit(
    metric: &str,
    binance: impl Future<Output = Result<f64, AppError>>,
    bybit: impl Future<Output = Result<f64, AppError>>,
) -> Result<Sourced, AppError> {
    match binance.await {
        Ok(value) => Ok((value, "binance")),
        Err(primary) => match bybit.await {
            Ok(value) => Ok((value, "bybit")),
            Err(fallback) => Err(AppError::CryptoSource(format!(
                "{metric}: Binance failed ({primary}); Bybit failed ({fallback})"
            ))),
        },
    }
}

fn funding_value(rates: &[f64], source: &str) -> Result<f64, AppError> {
    funding_annualized_pct(rates).ok_or_else(|| source_err(source, "no funding data"))
}

fn open_interest_value(series: &[f64], source: &str) -> Result<f64, AppError> {
    if series.len() < 2 {
        return Err(source_err(source, "not enough history"));
    }
    pct_change(series[0], series[series.len() - 1])
        .ok_or_else(|| source_err(source, "zero open interest at the start"))
}

fn long_short_value(ratios: &[f64], source: &str) -> Result<f64, AppError> {
    mean(ratios).ok_or_else(|| source_err(source, "no data"))
}

/// Símbolo do perpétuo em USDT (`ETHUSDT`, `BTCUSDT`), igual na Binance e na Bybit.
fn perp_symbol(coin: &str) -> String {
    format!("{}USDT", coin.to_ascii_uppercase())
}

async fn binance_funding(coin: &str) -> Result<f64, AppError> {
    let symbol = perp_symbol(coin);
    let url = format!("{BINANCE_FAPI}/fapi/v1/fundingRate?symbol={symbol}&limit={FUNDING_WINDOW}");
    funding_value(&parse_funding_rates(&get_text("Binance funding", &url).await?)?, "Binance funding")
}

async fn bybit_funding(coin: &str) -> Result<f64, AppError> {
    let symbol = perp_symbol(coin);
    let url = format!(
        "{BYBIT_API}/v5/market/funding/history?category=linear&symbol={symbol}&limit={FUNDING_WINDOW}"
    );
    funding_value(&parse_bybit_funding(&get_text("Bybit funding", &url).await?)?, "Bybit funding")
}

async fn binance_open_interest(coin: &str) -> Result<f64, AppError> {
    let symbol = perp_symbol(coin);
    let url = format!(
        "{BINANCE_FAPI}/futures/data/openInterestHist?symbol={symbol}&period=1d&limit={HISTORY_DAYS}"
    );
    open_interest_value(
        &parse_open_interest(&get_text("Binance open interest", &url).await?)?,
        "Binance open interest",
    )
}

async fn bybit_open_interest(coin: &str) -> Result<f64, AppError> {
    let symbol = perp_symbol(coin);
    let url = format!(
        "{BYBIT_API}/v5/market/open-interest?category=linear&symbol={symbol}&intervalTime=1d&limit={HISTORY_DAYS}"
    );
    open_interest_value(
        &parse_bybit_open_interest(&get_text("Bybit open interest", &url).await?)?,
        "Bybit open interest",
    )
}

async fn binance_long_short(coin: &str) -> Result<f64, AppError> {
    let symbol = perp_symbol(coin);
    let url = format!(
        "{BINANCE_FAPI}/futures/data/globalLongShortAccountRatio?symbol={symbol}&period=1d&limit={LONG_SHORT_WINDOW}"
    );
    long_short_value(&parse_long_short(&get_text("Binance long/short", &url).await?)?, "Binance long/short")
}

async fn bybit_long_short(coin: &str) -> Result<f64, AppError> {
    let symbol = perp_symbol(coin);
    let url = format!(
        "{BYBIT_API}/v5/market/account-ratio?category=linear&symbol={symbol}&period=1d&limit={LONG_SHORT_WINDOW}"
    );
    long_short_value(&parse_bybit_long_short(&get_text("Bybit long/short", &url).await?)?, "Bybit long/short")
}

/// Funding médio de 7 dias do perpétuo `{coin}USDT`, anualizado em %.
pub async fn fetch_funding_rate_7d(coin: &str) -> Result<Sourced, AppError> {
    binance_then_bybit("funding", binance_funding(coin), bybit_funding(coin)).await
}

/// Variação do open interest, em unidades da moeda, nos últimos ~30 dias, em %.
pub async fn fetch_open_interest_change_30d(coin: &str) -> Result<Sourced, AppError> {
    binance_then_bybit("open interest", binance_open_interest(coin), bybit_open_interest(coin)).await
}

/// Média de 7 dias da razão long/short de contas do perpétuo `{coin}USDT`.
pub async fn fetch_long_short_ratio(coin: &str) -> Result<Sourced, AppError> {
    binance_then_bybit("long/short", binance_long_short(coin), bybit_long_short(coin)).await
}

/// Último fechamento do DVOL (volatilidade implícita) da moeda na Deribit (só ETH e BTC têm).
pub async fn fetch_dvol(coin: &str) -> Result<f64, AppError> {
    let currency = coin.to_ascii_uppercase();
    let end = chrono::Utc::now().timestamp_millis();
    let start = end - 3 * 86_400_000;
    let url = format!(
        "{DERIBIT_API}/public/get_volatility_index_data?currency={currency}&start_timestamp={start}&end_timestamp={end}&resolution=86400"
    );
    parse_dvol_latest(&get_text("Deribit DVOL", &url).await?)
}

/// Put/call por open interest de todas as opções da moeda na Deribit.
pub async fn fetch_put_call_ratio(coin: &str) -> Result<f64, AppError> {
    let currency = coin.to_ascii_uppercase();
    let url = format!("{DERIBIT_API}/public/get_book_summary_by_currency?currency={currency}&kind=option");
    let open_interest = parse_option_open_interest(&get_text("Deribit options", &url).await?)?;
    put_call_ratio(&open_interest)
        .ok_or_else(|| source_err("Deribit options", "no call open interest"))
}

/// Variação % em 30 dias da oferta total de stablecoins (DefiLlama).
pub async fn fetch_stablecoin_supply_change_30d() -> Result<f64, AppError> {
    let url = format!("{DEFILLAMA_STABLECOINS}/stablecoincharts/all");
    parse_stablecoin_change_30d(&get_text("DefiLlama stablecoins", &url).await?)
}

/// Dominância do BTC hoje, em % do cap. total (CoinGecko `/global`).
pub async fn fetch_btc_dominance() -> Result<f64, AppError> {
    parse_btc_dominance(&get_text("CoinGecko global", &format!("{COINGECKO_API}/global")).await?)
}

/// Último SOPR (`sopr`) ou NUPL (`nupl`) do BTC no bitcoin-data.com, com a data
/// do dado (1 a 7 dias atrás no plano grátis). Cada chamada gasta a cota da fonte:
/// o coletor só chega aqui depois de checar que não há leitura de hoje.
pub async fn fetch_bitcoin_data(field: &str) -> Result<DatedReading, AppError> {
    let url = format!("{BITCOIN_DATA_API}/{field}/last");
    parse_bitcoin_data(&get_text("bitcoin-data.com", &url).await?, field)
}

/// Staking yield real do ETH: mediana do APY do stETH (7 últimos pontos) menos a inflação bruta.
pub async fn fetch_staking_yield_real() -> Result<f64, AppError> {
    let apy_url = format!("{DEFILLAMA_YIELDS}/chart/{STETH_POOL_ID}");
    let apy = parse_steth_apy(&get_text("DefiLlama yields", &apy_url).await?, chrono::Utc::now().date_naive())?;

    let cm_url = format!(
        "{COINMETRICS_COMMUNITY}/timeseries/asset-metrics?assets=eth&metrics=IssTotNtv,SplyCur&frequency=1d&limit_per_asset=1&paging_from=end"
    );
    let cm_body = get_text("CoinMetrics", &cm_url).await?;
    crate::finance_core::eth_indicators::ensure_fresh(&cm_body, chrono::Utc::now().date_naive())?;
    let (issuance, supply) = parse_issuance_and_supply(&cm_body)?;
    real_staking_yield(apy, issuance, supply)
        .ok_or_else(|| source_err("CoinMetrics", "circulating supply is zero"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Trechos copiados das respostas reais de 2026-10-16.
    const FUNDING: &str = r#"[{"symbol":"ETHUSDT","fundingTime":1792080000000,"fundingRate":"0.00005355","markPrice":"2439.89","rateType":"Regular"},{"symbol":"ETHUSDT","fundingTime":1792108800001,"fundingRate":"0.00007466","markPrice":"2441.05","rateType":"Regular"},{"symbol":"ETHUSDT","fundingTime":1792137600001,"fundingRate":"0.00004648","markPrice":"2454.27","rateType":"Regular"}]"#;
    const OPEN_INTEREST: &str = r#"[{"symbol":"ETHUSDT","sumOpenInterest":"2270359.16000000","sumOpenInterestValue":"5668837083.01","CMCCirculatingSupply":"122127761.09","timestamp":1791936000000},{"symbol":"ETHUSDT","sumOpenInterest":"2258306.77400000","sumOpenInterestValue":"5513177087.62","CMCCirculatingSupply":"122133648.45","timestamp":1792108800000}]"#;
    const LONG_SHORT: &str = r#"[{"symbol":"ETHUSDT","longAccount":"0.7267","longShortRatio":"2.6590","shortAccount":"0.2733","timestamp":1791936000000},{"symbol":"ETHUSDT","longAccount":"0.7252","longShortRatio":"2.6390","shortAccount":"0.2748","timestamp":1792108800000}]"#;
    // Últimos 8 pontos reais do pool stETH (DefiLlama, lido em 2026-10-31): o Lido reportou
    // 15,067% em 2026-10-30 e 2026-10-31, contra ~2,25% no normal.
    const YIELD_CHART: &str = r#"{"status":"success","data":[{"timestamp":"2026-10-24T23:01:32.839Z","apy":2.781},{"timestamp":"2026-10-25T23:01:13.950Z","apy":2.255},{"timestamp":"2026-10-26T22:01:34.873Z","apy":2.24},{"timestamp":"2026-10-27T23:01:29.472Z","apy":2.27155},{"timestamp":"2026-10-28T23:01:11.562Z","apy":2.25205},{"timestamp":"2026-10-29T23:01:43.312Z","apy":2.24386},{"timestamp":"2026-10-30T23:01:39.843Z","apy":15.06732},{"timestamp":"2026-10-31T12:01:50.401Z","apy":15.06732}]}"#;
    const ISSUANCE: &str = r#"{"data":[{"asset":"eth","time":"2026-10-14T00:00:00.000000000Z","IssTotNtv":"2979.961923327","SplyCur":"122130969.817571701831258692"},{"asset":"eth","time":"2026-10-15T00:00:00.000000000Z","IssTotNtv":"2979.616171783","SplyCur":"122133892.772935516304718648"}]}"#;

    // Trecho da resposta real de `/global` de 2026-10-26 (só as chaves usadas).
    const GLOBAL: &str = r#"{"data":{"active_cryptocurrencies":22501,"market_cap_percentage":{"btc":59.98151875544407,"eth":10.606521696958916,"usdt":6.4},"updated_at":1792973614}}"#;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    /// Série do pool no formato da DefiLlama: pares (data `YYYY-MM-DD`, `apy` em JSON cru,
    /// ex.: `2.5`, `null`, `"x"`).
    fn yield_chart(points: &[(&str, &str)]) -> String {
        let rows: Vec<String> = points
            .iter()
            .map(|(day, apy)| format!(r#"{{"timestamp":"{day}T12:00:00.000Z","apy":{apy}}}"#))
            .collect();
        format!(r#"{{"status":"success","data":[{}]}}"#, rows.join(","))
    }

    /// Série de 7 dias seguidos (2026-10-25 a 2026-10-31), um `apy` cru por dia, do mais antigo
    /// ao mais novo.
    fn week(apys: [&str; 7]) -> String {
        let days = ["25", "26", "27", "28", "29", "30", "31"];
        let owned: Vec<String> = days.iter().map(|d| format!("2026-10-{d}")).collect();
        let points: Vec<(&str, &str)> = owned.iter().map(String::as_str).zip(apys).collect();
        yield_chart(&points)
    }

    fn day(y: i32, m: u32, d: u32) -> chrono::NaiveDate {
        chrono::NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn btc_dominance_is_read_from_the_global_market_cap_percentage() {
        assert!(close(parse_btc_dominance(GLOBAL).unwrap(), 59.98151875544407));
    }

    #[test]
    fn btc_dominance_rejects_missing_or_implausible_values() {
        assert!(parse_btc_dominance(r#"{"data":{"market_cap_percentage":{"eth":10.6}}}"#).is_err());
        assert!(parse_btc_dominance(r#"{"data":{"market_cap_percentage":{"btc":140.0}}}"#).is_err());
        assert!(parse_btc_dominance(r#"{"status":{"error_code":429}}"#).is_err());
        assert!(parse_btc_dominance("not json").is_err());
    }

    // Respostas reais de `/v1/{sopr,nupl}/last` de 2026-10-26 (dado de 7 dias antes).
    const SOPR_LAST: &str = r#"{"d":"2026-10-19","unixTs":1792368000,"sopr":1.0057,"delayed":true,"message":"Real-time data (last 7 days) requires an active subscription. https://bitcoin-data.com/bguser/pricing"}"#;
    const NUPL_LAST: &str = r#"{"d":"2026-10-19","unixTs":1792368000,"nupl":0.347,"delayed":true,"message":"Real-time data (last 7 days) requires an active subscription. https://bitcoin-data.com/bguser/pricing"}"#;

    #[test]
    fn bitcoin_data_reads_the_value_and_the_date_of_the_data() {
        let sopr = parse_bitcoin_data(SOPR_LAST, "sopr").unwrap();
        assert!(close(sopr.value, 1.0057));
        assert_eq!(sopr.date, "2026-10-19");
        let nupl = parse_bitcoin_data(NUPL_LAST, "nupl").unwrap();
        assert!(close(nupl.value, 0.347));
        assert_eq!(nupl.date, "2026-10-19");
    }

    #[test]
    fn bitcoin_data_rejects_the_wrong_field_a_bad_date_and_implausible_values() {
        // O corpo de um indicador não vale para o outro.
        assert!(parse_bitcoin_data(SOPR_LAST, "nupl").is_err());
        assert!(parse_bitcoin_data(NUPL_LAST, "sopr").is_err());
        // Campo que a fonte não atende aqui.
        assert!(parse_bitcoin_data(SOPR_LAST, "mvrv").is_err());
        // Data ausente ou fora do formato.
        assert!(parse_bitcoin_data(r#"{"sopr":1.0}"#, "sopr").is_err());
        assert!(parse_bitcoin_data(r#"{"d":"19/10/2026","sopr":1.0}"#, "sopr").is_err());
        // Fora da faixa: SOPR é razão positiva, NUPL fica em [-1, 1].
        assert!(parse_bitcoin_data(r#"{"d":"2026-10-19","sopr":0}"#, "sopr").is_err());
        assert!(parse_bitcoin_data(r#"{"d":"2026-10-19","sopr":-1.2}"#, "sopr").is_err());
        assert!(parse_bitcoin_data(r#"{"d":"2026-10-19","sopr":55.0}"#, "sopr").is_err());
        assert!(parse_bitcoin_data(r#"{"d":"2026-10-19","nupl":1.4}"#, "nupl").is_err());
        assert!(parse_bitcoin_data(r#"{"d":"2026-10-19","nupl":-0.2}"#, "nupl").is_ok());
        // Valor como texto (formato diferente) e corpo que não é JSON (ex.: página de limite).
        assert!(parse_bitcoin_data(r#"{"d":"2026-10-19","sopr":"1.0"}"#, "sopr").is_err());
        assert!(parse_bitcoin_data("Too Many Requests", "sopr").is_err());
    }

    #[test]
    fn funding_is_annualized_from_the_8h_average() {
        // 0,0001 por 8h × 3 × 365 × 100 = 10,95% ao ano.
        assert!(close(funding_annualized_pct(&[0.0001, 0.0001]).unwrap(), 10.95));
        assert!(funding_annualized_pct(&[]).is_none());
    }

    #[test]
    fn negative_funding_stays_negative() {
        assert!(funding_annualized_pct(&[-0.0001, -0.0003]).unwrap() < 0.0);
    }

    #[test]
    fn pct_change_guards_zero_start() {
        assert!(close(pct_change(200.0, 250.0).unwrap(), 25.0));
        assert!(close(pct_change(200.0, 150.0).unwrap(), -25.0));
        assert!(pct_change(0.0, 10.0).is_none());
    }

    #[test]
    fn real_yield_subtracts_gross_inflation() {
        // 2,255% de APY − (2.980 ETH/dia × 365 ÷ 122,13M × 100) = 2,255 − 0,8905
        // ≈ 1,3645.
        let real = real_staking_yield(2.255, 2979.616171783, 122133892.7729).unwrap();
        assert!(real > 1.364 && real < 1.365);
        assert!(real_staking_yield(2.0, 100.0, 0.0).is_none());
    }

    #[test]
    fn parses_real_binance_funding_payload() {
        let rates = parse_funding_rates(FUNDING).unwrap();
        assert_eq!(rates.len(), 3);
        assert!(close(rates[0], 0.00005355));
    }

    #[test]
    fn parses_open_interest_in_coin_units() {
        let oi = parse_open_interest(OPEN_INTEREST).unwrap();
        assert!(close(oi[0], 2270359.16));
        // Variação de −0,53% entre os dois pontos.
        assert!(pct_change(oi[0], oi[1]).unwrap() < 0.0);
    }

    #[test]
    fn parses_long_short_ratio() {
        let ratios = parse_long_short(LONG_SHORT).unwrap();
        assert!(close(mean(&ratios).unwrap(), 2.649));
    }

    #[test]
    fn steth_apy_is_the_median_of_the_last_7_so_a_lido_spike_does_not_win() {
        // Últimos 7 da série real, ordenados: 2,24 2,24386 2,25205 2,255 2,27155 15,067 15,067
        // → mediana 2,255. O último ponto sozinho (o critério antigo) daria 15,067.
        let apy = parse_steth_apy(YIELD_CHART, day(2026, 11, 1)).unwrap();
        assert!(close(apy, 2.255), "{apy}");
    }

    #[test]
    fn steth_apy_window_is_the_last_7_points_not_the_first_7_nor_the_whole_series() {
        // Últimos 7 = 10 → 10. Com os 7 primeiros daria 2; com os 8 (mediana 6), 6.
        let body = yield_chart(&[
            ("2026-10-24", "0"),
            ("2026-10-25", "2"),
            ("2026-10-26", "2"),
            ("2026-10-27", "2"),
            ("2026-10-28", "10"),
            ("2026-10-29", "10"),
            ("2026-10-30", "10"),
            ("2026-10-31", "10"),
        ]);
        assert!(close(parse_steth_apy(&body, day(2026, 11, 1)).unwrap(), 10.0));
    }

    #[test]
    fn steth_apy_ignores_points_before_the_window_even_if_invalid() {
        let mut points = vec![("2022-01-01", "null"), ("2022-01-02", "-1")];
        let owned: Vec<String> = ["25", "26", "27", "28", "29", "30", "31"]
            .iter()
            .map(|d| format!("2026-10-{d}"))
            .collect();
        points.extend(owned.iter().map(|d| (d.as_str(), "3")));
        let body = yield_chart(&points);
        assert!(close(parse_steth_apy(&body, day(2026, 11, 1)).unwrap(), 3.0));
    }

    #[test]
    fn steth_apy_survives_up_to_3_bad_points_in_the_window() {
        let body = week(["2", "2", "2", "2", "15", "15", "15"]);
        assert!(close(parse_steth_apy(&body, day(2026, 11, 1)).unwrap(), 2.0));
    }

    #[test]
    fn steth_apy_rejects_a_series_with_fewer_than_7_points() {
        let body = yield_chart(&[("2026-10-30", "2"), ("2026-10-31", "2")]);
        let err = parse_steth_apy(&body, day(2026, 11, 1)).unwrap_err();
        assert!(err.to_string().contains("DefiLlama yields"), "{err}");
        assert!(parse_steth_apy(r#"{"data":[]}"#, day(2026, 11, 1)).is_err());
    }

    #[test]
    fn steth_apy_rejects_null_text_negative_or_non_finite_apy_in_any_of_the_7() {
        for bad in ["null", r#""x""#, "-0.1", r#""NaN""#] {
            for position in 0..7 {
                let mut apys = ["2"; 7];
                apys[position] = bad;
                let body = week(apys);
                assert!(
                    parse_steth_apy(&body, day(2026, 11, 1)).is_err(),
                    "{bad} na posição {position} devia dar erro"
                );
            }
        }
        // `apy` ausente (sem a chave) também.
        let missing = r#"{"data":[{"timestamp":"2026-10-25T12:00:00Z"},{"timestamp":"2026-10-26T12:00:00Z","apy":2},{"timestamp":"2026-10-27T12:00:00Z","apy":2},{"timestamp":"2026-10-28T12:00:00Z","apy":2},{"timestamp":"2026-10-29T12:00:00Z","apy":2},{"timestamp":"2026-10-30T12:00:00Z","apy":2},{"timestamp":"2026-10-31T12:00:00Z","apy":2}]}"#;
        assert!(parse_steth_apy(missing, day(2026, 11, 1)).is_err());
    }

    #[test]
    fn steth_apy_accepts_zero() {
        let body = week(["0"; 7]);
        assert!(close(parse_steth_apy(&body, day(2026, 11, 1)).unwrap(), 0.0));
    }

    #[test]
    fn steth_apy_rejects_a_stale_last_point() {
        let body = week(["2"; 7]); // último ponto: 2026-10-31
        assert!(parse_steth_apy(&body, day(2026, 11, 7)).is_ok()); // 7 dias: no limite
        let stale = parse_steth_apy(&body, day(2026, 11, 8)).unwrap_err(); // 8 dias
        assert!(stale.to_string().contains("stale data"), "{stale}");
    }

    #[test]
    fn steth_apy_rejects_a_missing_or_invalid_timestamp() {
        for last in [r#"{"apy":2}"#, r#"{"timestamp":"t","apy":2}"#, r#"{"timestamp":"2026-13-45T00:00:00Z","apy":2}"#, r#"{"timestamp":20261031,"apy":2}"#] {
            let first = (25..31)
                .map(|d| format!(r#"{{"timestamp":"2026-10-{d}T12:00:00Z","apy":2}}"#))
                .collect::<Vec<_>>()
                .join(",");
            let body = format!(r#"{{"data":[{first},{last}]}}"#);
            assert!(parse_steth_apy(&body, day(2026, 11, 1)).is_err(), "{last}");
        }
    }

    #[test]
    fn steth_apy_rejects_a_body_that_is_not_the_expected_shape() {
        for body in ["", "not json", "[]", r#"{"data":"x"}"#, r#"{"status":"success"}"#] {
            assert!(parse_steth_apy(body, day(2026, 11, 1)).is_err(), "{body}");
        }
    }

    #[test]
    fn parses_latest_issuance_and_supply() {
        let (issuance, supply) = parse_issuance_and_supply(ISSUANCE).unwrap();
        assert!(close(issuance, 2979.616171783));
        assert!(supply > 122_000_000.0);
        assert!(parse_issuance_and_supply(r#"{"data":[]}"#).is_err());
    }

    #[test]
    fn invalid_number_names_the_source() {
        let err = parse_funding_rates(r#"[{"fundingRate":"abc"}]"#).unwrap_err();
        assert!(err.to_string().contains("Binance funding"));
    }

    // Bybit, respostas reais de 2026-10-16: **mais novo primeiro**.
    const BYBIT_FUNDING: &str = r#"{"retCode":0,"retMsg":"OK","result":{"category":"linear","list":[{"symbol":"ETHUSDT","fundingRate":"0.00005006","fundingRateTimestamp":"1792137600000"},{"symbol":"ETHUSDT","fundingRate":"0.00009644","fundingRateTimestamp":"1792108800000"}]},"retExtInfo":{},"time":1792157675806}"#;
    const BYBIT_OPEN_INTEREST: &str = r#"{"retCode":0,"retMsg":"OK","result":{"symbol":"ETHUSDT","category":"linear","list":[{"openInterest":"781262.72000000","singleOpenInterest":"390631.36","timestamp":"1792108800000"},{"openInterest":"774923.49000000","singleOpenInterest":"387461.75","timestamp":"1792022400000"}],"nextPageCursor":"x"},"retExtInfo":{},"time":1792157676206}"#;
    const BYBIT_RATIO: &str = r#"{"retCode":0,"retMsg":"OK","result":{"list":[{"symbol":"ETHUSDT","buyRatio":"0.724","sellRatio":"0.276","timestamp":"1792108800000"},{"symbol":"ETHUSDT","buyRatio":"0.7228","sellRatio":"0.2772","timestamp":"1792022400000"}],"nextPageCursor":"x"},"retExtInfo":{},"time":1792157676601}"#;
    // Deribit: forma real, `[timestamp, open, high, low, close]`.
    const DVOL: &str = r#"{"jsonrpc":"2.0","result":{"data":[[1791936000000,50.1,52.0,49.0,51.5],[1792022400000,51.5,53.2,50.8,52.4],[1792108800000,52.4,53.0,50.0,51.08]],"continuation":null},"testnet":false}"#;
    const OPTIONS: &str = r#"{"jsonrpc":"2.0","result":[{"instrument_name":"ETH-25JUN27-2900-P","open_interest":508.0,"mark_price":0.26},{"instrument_name":"ETH-25JUN27-3500-C","open_interest":800.0,"mark_price":0.1},{"instrument_name":"ETH-30OCT26-2500-P","open_interest":200.0,"mark_price":0.05},{"instrument_name":"ETH-30OCT26-2500-C","open_interest":300.0,"mark_price":0.2},{"instrument_name":"ETH-PERPETUAL","open_interest":999.0,"mark_price":2440.0}]}"#;
    // DefiLlama stablecoins: forma real, datas em segundos Unix como texto.
    // Último ponto = 1_000_000 + 30 dias; base = 300; agora = 306 (+2%).
    const STABLECOINS: &str = r#"[{"date":"1000000","totalCirculating":{"peggedUSD":300.0},"totalCirculatingUSD":{"peggedUSD":300.0}},{"date":"1864000","totalCirculating":{"peggedUSD":310.0},"totalCirculatingUSD":{"peggedUSD":310.0}},{"date":"3592000","totalCirculating":{"peggedUSD":306.0},"totalCirculatingUSD":{"peggedUSD":306.0}}]"#;

    #[test]
    fn bybit_series_come_back_in_chronological_order() {
        // A Bybit manda o mais novo primeiro; o parser inverte.
        let funding = parse_bybit_funding(BYBIT_FUNDING).unwrap();
        assert!(close(funding[0], 0.00009644));
        assert!(close(funding[1], 0.00005006));

        let oi = parse_bybit_open_interest(BYBIT_OPEN_INTEREST).unwrap();
        assert!(close(oi[0], 774923.49));
        // Subiu de 774.923 pra 781.262 (+0,82%), não caiu.
        assert!(pct_change(oi[0], oi[1]).unwrap() > 0.8);
        assert!(open_interest_value(&oi, "t").unwrap() > 0.0);
    }

    #[test]
    fn bybit_long_short_is_buy_over_sell() {
        let ratios = parse_bybit_long_short(BYBIT_RATIO).unwrap();
        assert!(close(ratios[1], 0.724 / 0.276));
        // Bate com a razão da Binance no mesmo dia (2,64).
        assert!((mean(&ratios).unwrap() - 2.64).abs() < 0.05);
        assert!(parse_bybit_long_short(
            r#"{"retCode":0,"retMsg":"OK","result":{"list":[{"buyRatio":"0.5","sellRatio":"0"}]}}"#
        )
        .is_err());
    }

    #[test]
    fn bybit_error_envelope_becomes_a_named_source_error() {
        let err = parse_bybit_funding(r#"{"retCode":10001,"retMsg":"params error","result":{}}"#)
            .unwrap_err();
        assert!(err.to_string().contains("Bybit funding"));
        assert!(err.to_string().contains("params error"));
    }

    #[test]
    fn dvol_takes_the_latest_close() {
        assert!(close(parse_dvol_latest(DVOL).unwrap(), 51.08));
        assert!(parse_dvol_latest(r#"{"result":{"data":[]}}"#).is_err());
    }

    #[test]
    fn put_call_sums_open_interest_by_option_type_and_skips_other_instruments() {
        let oi = parse_option_open_interest(OPTIONS).unwrap();
        assert_eq!(oi.len(), 4); // o perpétuo fica de fora
        // Puts 508 + 200 = 708; calls 800 + 300 = 1.100.
        assert!(close(put_call_ratio(&oi).unwrap(), 708.0 / 1100.0));
        assert!(put_call_ratio(&[(true, 10.0)]).is_none());
    }

    #[test]
    fn stablecoin_change_uses_the_point_30_days_before_not_the_nearest() {
        // O ponto de 310 (a ~20 dias do fim) não pode servir de base.
        assert!(close(parse_stablecoin_change_30d(STABLECOINS).unwrap(), 2.0));
    }

    #[test]
    fn stablecoin_change_needs_30_days_of_history() {
        let short = r#"[{"date":"1000000","totalCirculatingUSD":{"peggedUSD":300.0}},{"date":"1100000","totalCirculatingUSD":{"peggedUSD":301.0}}]"#;
        assert!(parse_stablecoin_change_30d(short).is_err());
        assert!(parse_stablecoin_change_30d("[]").is_err());
    }

    #[tokio::test]
    async fn fallback_uses_bybit_only_when_binance_fails() {
        let ok = |v: f64| std::future::ready(Ok::<f64, AppError>(v));
        let err = |m: &str| std::future::ready(Err::<f64, AppError>(AppError::CryptoSource(m.to_string())));

        assert_eq!(binance_then_bybit("m", ok(1.0), ok(2.0)).await.unwrap(), (1.0, "binance"));
        assert_eq!(
            binance_then_bybit("m", err("geo-blocked"), ok(2.0)).await.unwrap(),
            (2.0, "bybit")
        );
        let both = binance_then_bybit("funding", err("geo-blocked"), err("timeout"))
            .await
            .unwrap_err()
            .to_string();
        assert!(both.contains("funding") && both.contains("geo-blocked") && both.contains("timeout"));
    }

    // Ao vivo: `cargo test --lib -- --ignored crypto_sources`.
    #[tokio::test]
    #[ignore]
    async fn live_sources_return_plausible_values() {
        for coin in ["ETH", "BTC"] {
            let (funding, source) = fetch_funding_rate_7d(coin).await.unwrap();
            assert!(funding.abs() < 200.0 && !source.is_empty(), "{coin}");
            let (oi, _) = fetch_open_interest_change_30d(coin).await.unwrap();
            assert!(oi.abs() < 500.0, "{coin}");
            let (ls, _) = fetch_long_short_ratio(coin).await.unwrap();
            assert!(ls > 0.0 && ls < 20.0, "{coin}");
            let dvol = fetch_dvol(coin).await.unwrap();
            assert!(dvol > 10.0 && dvol < 300.0, "{coin}");
            let put_call = fetch_put_call_ratio(coin).await.unwrap();
            assert!(put_call > 0.05 && put_call < 5.0, "{coin}");
        }
        let staking = fetch_staking_yield_real().await.unwrap();
        assert!(staking > -5.0 && staking < 15.0);
        let stable = fetch_stablecoin_supply_change_30d().await.unwrap();
        assert!(stable.abs() < 50.0);
    }

    #[test]
    fn perp_symbol_follows_the_coin() {
        assert_eq!(perp_symbol("ETH"), "ETHUSDT");
        assert_eq!(perp_symbol("btc"), "BTCUSDT");
    }

    // Ao vivo: a Bybit sozinha entrega valores plausíveis (o fallback).
    #[tokio::test]
    #[ignore]
    async fn live_bybit_fallback_returns_plausible_values() {
        for coin in ["ETH", "BTC"] {
            assert!(bybit_funding(coin).await.unwrap().abs() < 200.0, "{coin}");
            assert!(bybit_open_interest(coin).await.unwrap().abs() < 500.0, "{coin}");
            let ls = bybit_long_short(coin).await.unwrap();
            assert!(ls > 0.0 && ls < 20.0, "{coin}");
        }
    }
}
