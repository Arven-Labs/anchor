// BCB PTAX (Olinda, OData) — a cotação oficial que a Lei 14.754/2023 manda usar na conversão de moeda estrangeira
// (art. 15: fechamento, para venda, na data do fato gerador; o imposto pago lá fora usa o fechamento para compra).
// Pública, sem chave. Datas da consulta em `'MM-DD-YYYY'`.
//
// - Dólar: `CotacaoDolarPeriodo` devolve uma linha por dia útil (o fechamento): `cotacaoCompra`, `cotacaoVenda`,
//   `dataHoraCotacao` (`"2025-01-02 13:09:42.489"`).
// - Outras moedas (`PTAX_CURRENCIES`): `CotacaoMoedaPeriodo` devolve todos os boletins do dia (`Abertura`,
//   `Intermediário`, `Fechamento`) com `tipoBoletim`; vale o `Fechamento`. Os `cotacao*` já vêm em reais por unidade.
// - Fim de semana e feriado não têm linha; quem consome decide o que fazer (`domain::ptax`).
//
// O BCB oscila (ver `bcb_sgs`): timeout maior e até 3 tentativas, e corpo HTML com status 200 também é tentado de novo.

use std::collections::BTreeMap;
use std::time::Duration;

use chrono::{NaiveDate, Utc};
use serde::Deserialize;

use crate::error::AppError;
use crate::finance_core::http;

pub const SOURCE: &str = "bcb_ptax";
const BASE_URL: &str = "https://olinda.bcb.gov.br/olinda/servico/PTAX/versao/v1/odata";
const TIMEOUT: Duration = Duration::from_secs(45);
const RETRY_ATTEMPTS: u32 = 3;
const RETRY_BACKOFF: Duration = Duration::from_secs(2);
/// Um ano por consulta: ~250 linhas do dólar ou ~1.000 boletins de outra moeda, bem abaixo do `$top`.
const WINDOW_DAYS: i64 = 365;
const TOP: u32 = 10_000;
const CLOSING_BULLETIN: &str = "Fechamento";

/// Moedas que a PTAX cota (`/Moedas` da Olinda).
pub const PTAX_CURRENCIES: [&str; 10] = ["AUD", "CAD", "CHF", "DKK", "EUR", "GBP", "JPY", "NOK", "SEK", "USD"];

pub fn is_supported(currency: &str) -> bool {
    PTAX_CURRENCIES.contains(&currency)
}

/// Fechamento PTAX de um dia, em reais por unidade da moeda.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PtaxQuote {
    pub buy: f64,
    pub sell: f64,
}

#[derive(Deserialize)]
struct Envelope {
    value: Vec<Item>,
}

#[derive(Deserialize)]
struct Item {
    #[serde(rename = "cotacaoCompra")]
    buy: f64,
    #[serde(rename = "cotacaoVenda")]
    sell: f64,
    #[serde(rename = "dataHoraCotacao")]
    at: String,
    #[serde(rename = "tipoBoletim", default)]
    bulletin: Option<String>,
}

fn parse_items(body: &str) -> Result<Vec<Item>, AppError> {
    serde_json::from_str::<Envelope>(body)
        .map(|e| e.value)
        .map_err(|e| AppError::ProviderUnavailable(format!("BCB PTAX: {e}")))
}

fn day_of(item: &Item) -> Result<NaiveDate, AppError> {
    item.at
        .get(0..10)
        .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        .ok_or_else(|| AppError::ProviderUnavailable(format!("BCB PTAX: invalid date '{}'", item.at)))
}

fn valid(item: &Item) -> bool {
    item.buy.is_finite() && item.sell.is_finite() && item.buy > 0.0 && item.sell > 0.0
}

/// Dólar: uma linha por dia (se houver mais de uma, vale a mais tardia).
pub fn parse_dollar(body: &str) -> Result<BTreeMap<NaiveDate, PtaxQuote>, AppError> {
    let mut latest: BTreeMap<NaiveDate, (String, PtaxQuote)> = BTreeMap::new();
    for item in parse_items(body)?.into_iter().filter(valid) {
        let day = day_of(&item)?;
        let quote = PtaxQuote { buy: item.buy, sell: item.sell };
        match latest.get(&day) {
            Some((at, _)) if *at >= item.at => {}
            _ => {
                latest.insert(day, (item.at, quote));
            }
        }
    }
    Ok(latest.into_iter().map(|(day, (_, quote))| (day, quote)).collect())
}

/// Outras moedas: só o boletim de `Fechamento` de cada dia (os demais são parciais).
pub fn parse_currency(body: &str) -> Result<BTreeMap<NaiveDate, PtaxQuote>, AppError> {
    let mut quotes = BTreeMap::new();
    for item in parse_items(body)?.into_iter().filter(valid) {
        if item.bulletin.as_deref() != Some(CLOSING_BULLETIN) {
            continue;
        }
        quotes.insert(day_of(&item)?, PtaxQuote { buy: item.buy, sell: item.sell });
    }
    Ok(quotes)
}

/// Janelas `[início, fim]` de um ano de `from` até `to`, contíguas e sem sobreposição.
pub fn windows(from: NaiveDate, to: NaiveDate) -> Vec<(NaiveDate, NaiveDate)> {
    let mut out = Vec::new();
    let mut start = from;
    while start <= to {
        let end = (start + chrono::Duration::days(WINDOW_DAYS - 1)).min(to);
        out.push((start, end));
        start = end.succ_opt().expect("has successor");
    }
    out
}

fn url(currency: &str, start: NaiveDate, end: NaiveDate) -> String {
    let (from, to) = (start.format("%m-%d-%Y"), end.format("%m-%d-%Y"));
    if currency == "USD" {
        format!(
            "{BASE_URL}/CotacaoDolarPeriodo(dataInicial=@dataInicial,dataFinalCotacao=@dataFinalCotacao)\
             ?@dataInicial='{from}'&@dataFinalCotacao='{to}'&$top={TOP}&$format=json"
        )
    } else {
        format!(
            "{BASE_URL}/CotacaoMoedaPeriodo(moeda=@moeda,dataInicial=@dataInicial,dataFinalCotacao=@dataFinalCotacao)\
             ?@moeda='{currency}'&@dataInicial='{from}'&@dataFinalCotacao='{to}'&$top={TOP}&$format=json"
        )
    }
}

async fn fetch_window(
    currency: &str,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<BTreeMap<NaiveDate, PtaxQuote>, AppError> {
    let url = url(currency, start, end);
    let parse = if currency == "USD" { parse_dollar } else { parse_currency };
    let mut last = None;
    for attempt in 0..RETRY_ATTEMPTS {
        if attempt > 0 {
            tokio::time::sleep(RETRY_BACKOFF).await;
        }
        // Corpo HTML com status 200 cai no parse e também é tentado de novo.
        match http::get_text("BCB PTAX", &url, TIMEOUT).await.and_then(|b| parse(&b)) {
            Ok(quotes) => return Ok(quotes),
            // 404 não melhora com nova tentativa.
            Err(err @ AppError::NoData(_)) => return Err(err),
            Err(err) => last = Some(err),
        }
    }
    Err(last.expect("at least one attempt"))
}

/// Fechamentos PTAX de `currency` entre `from` e `to` (inclusive). Moeda que a PTAX não cota é `NoData` antes de
/// qualquer rede.
pub async fn fetch_range(
    currency: &str,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<BTreeMap<NaiveDate, PtaxQuote>, AppError> {
    if !is_supported(currency) {
        return Err(AppError::NoData(format!("BCB PTAX does not quote {currency}")));
    }
    let to = to.min(Utc::now().date_naive() + chrono::Duration::days(1));
    let mut all = BTreeMap::new();
    for (start, end) in windows(from, to) {
        all.extend(fetch_window(currency, start, end).await?);
    }
    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    // Respostas reais da Olinda (amostradas em 2026-10-21).
    const DOLLAR: &str = r#"{"@odata.context":"https://was-p.bcnet.bcb.gov.br/olinda/servico/PTAX/versao/v1/odata$metadata#_CotacaoDolarPeriodo","value":[
        {"cotacaoCompra":6.19170,"cotacaoVenda":6.19230,"dataHoraCotacao":"2024-12-30 13:06:26.618"},
        {"cotacaoCompra":6.19170,"cotacaoVenda":6.19230,"dataHoraCotacao":"2024-12-31 10:04:09.264"},
        {"cotacaoCompra":6.20800,"cotacaoVenda":6.20860,"dataHoraCotacao":"2025-01-02 13:09:42.489"},
        {"cotacaoCompra":6.15570,"cotacaoVenda":6.15630,"dataHoraCotacao":"2025-01-03 13:07:30.503"}]}"#;

    const EURO: &str = r#"{"@odata.context":"x","value":[
        {"paridadeCompra":1.03180,"paridadeVenda":1.03200,"cotacaoCompra":6.42320,"cotacaoVenda":6.42500,"dataHoraCotacao":"2025-01-02 10:10:25.056","tipoBoletim":"Abertura"},
        {"paridadeCompra":1.02740,"paridadeVenda":1.02750,"cotacaoCompra":6.35980,"cotacaoVenda":6.36100,"dataHoraCotacao":"2025-01-02 13:09:42.485","tipoBoletim":"Intermediário"},
        {"paridadeCompra":1.02740,"paridadeVenda":1.02750,"cotacaoCompra":6.37810,"cotacaoVenda":6.37930,"dataHoraCotacao":"2025-01-02 13:09:42.489","tipoBoletim":"Fechamento"},
        {"paridadeCompra":1.02960,"paridadeVenda":1.02970,"cotacaoCompra":6.34420,"cotacaoVenda":6.34540,"dataHoraCotacao":"2025-01-03 10:11:26.859","tipoBoletim":"Abertura"}]}"#;

    #[test]
    fn the_dollar_has_one_closing_quote_per_business_day() {
        let quotes = parse_dollar(DOLLAR).unwrap();
        assert_eq!(quotes.len(), 4);
        assert_eq!(quotes[&d(2025, 1, 2)], PtaxQuote { buy: 6.208, sell: 6.2086 });
        assert!(!quotes.contains_key(&d(2025, 1, 1)), "feriado não tem PTAX");
    }

    #[test]
    fn other_currencies_keep_only_the_closing_bulletin() {
        let quotes = parse_currency(EURO).unwrap();
        // 03/01 só tem a abertura (resposta cortada): não vira cotação do dia
        assert_eq!(quotes.len(), 1);
        assert_eq!(quotes[&d(2025, 1, 2)], PtaxQuote { buy: 6.3781, sell: 6.3793 });
    }

    #[test]
    fn a_later_row_of_the_same_day_wins_for_the_dollar() {
        let body = r#"{"value":[
            {"cotacaoCompra":5.0,"cotacaoVenda":5.1,"dataHoraCotacao":"2025-01-02 10:00:00.000"},
            {"cotacaoCompra":5.2,"cotacaoVenda":5.3,"dataHoraCotacao":"2025-01-02 13:00:00.000"}]}"#;
        assert_eq!(parse_dollar(body).unwrap()[&d(2025, 1, 2)], PtaxQuote { buy: 5.2, sell: 5.3 });
    }

    #[test]
    fn html_with_status_200_is_a_provider_error_not_an_empty_series() {
        let err = parse_dollar("<html><body>Serviço indisponível</body></html>").unwrap_err();
        assert!(matches!(err, AppError::ProviderUnavailable(_)));
        assert!(matches!(parse_currency("").unwrap_err(), AppError::ProviderUnavailable(_)));
    }

    #[test]
    fn non_positive_quotes_are_dropped() {
        let body = r#"{"value":[{"cotacaoCompra":0.0,"cotacaoVenda":0.0,"dataHoraCotacao":"2025-01-02 13:00:00.000"}]}"#;
        assert!(parse_dollar(body).unwrap().is_empty());
    }

    #[test]
    fn windows_are_contiguous_one_year_ranges() {
        let w = windows(d(2024, 1, 1), d(2026, 6, 30));
        assert_eq!(w[0], (d(2024, 1, 1), d(2024, 12, 30)));
        assert_eq!(w[1].0, d(2024, 12, 31));
        assert_eq!(w.last().unwrap().1, d(2026, 6, 30));
        for pair in w.windows(2) {
            assert_eq!(pair[0].1.succ_opt().unwrap(), pair[1].0, "sem buraco nem sobreposição");
        }
        assert!(windows(d(2026, 7, 1), d(2026, 6, 30)).is_empty());
    }

    #[test]
    fn urls_use_the_month_first_dates_and_the_right_endpoint() {
        let dollar = url("USD", d(2025, 1, 2), d(2025, 1, 8));
        assert!(dollar.contains("CotacaoDolarPeriodo"));
        assert!(dollar.contains("@dataInicial='01-02-2025'") && dollar.contains("@dataFinalCotacao='01-08-2025'"));
        let euro = url("EUR", d(2025, 1, 2), d(2025, 1, 8));
        assert!(euro.contains("CotacaoMoedaPeriodo") && euro.contains("@moeda='EUR'"));
    }

    #[tokio::test]
    async fn an_unsupported_currency_fails_before_any_network_call() {
        let err = fetch_range("BRL", d(2025, 1, 1), d(2025, 1, 31)).await.unwrap_err();
        assert!(matches!(err, AppError::NoData(_)));
        assert!(!is_supported("CNY") && is_supported("EUR"));
    }
}
