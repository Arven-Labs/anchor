// Rascunho de tese a partir dos dados coletados (frente 3 do ROADMAP, sem IA). Só leitura:
// monta o texto e devolve; quem salva a tese é o formulário, depois de o dono editar.
// Os indicadores vêm do screener (mesma conta de DY e razões), os valuations da tabela `valuation`.

use chrono::{NaiveDate, Utc};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};

use crate::commands::crypto_indicator::crypto_score_as_of;
use crate::commands::screener::{screen_stocks_on, ScreenerFilters};
use crate::domain::thesis_draft::{
    render_crypto_draft, CryptoDraftFearGreed, CryptoDraftInput, CryptoPricePoint, render_draft, render_fii_draft, Draft, DraftIndicators, DraftInput, DraftValuation, FiiDraftInput, FiiDraftMonthly,
    FiiDraftProperty, FundDraftInput, FundDraftTechnicals, FundKind, render_fund_draft,
};
use crate::entity::{
    assets, crypto_fear_greed, fii_cnpj_cache, fii_cvm_monthly, fii_cvm_properties, stock_dividends_avg,
    stock_price_history, stock_quotes, stock_technicals, valuation,
};
use crate::error::AppError;

/// `today` é entrada (não lê o relógio) para o retrato datado ser testável.
pub async fn build_draft_on(db: &DatabaseConnection, ticker: &str, today: NaiveDate) -> Result<Draft, AppError> {
    let ticker = ticker.trim().to_uppercase();
    let screened = screen_stocks_on(db, &ScreenerFilters::default(), today).await?;
    let row = screened.rows.into_iter().find(|r| r.ticker == ticker);
    let valuations = valuation::Entity::find()
        .filter(valuation::Column::Ticker.eq(ticker.as_str()))
        .all(db)
        .await?
        .into_iter()
        .map(|v| DraftValuation {
            model: v.model,
            fair_price: v.fair_price,
            safety_margin: v.safety_margin,
            verdict: v.verdict,
            updated_at: v.updated_at,
        })
        .collect();
    let input = DraftInput {
        name: row.as_ref().and_then(|r| r.name.clone()),
        indicators: row.map(|r| DraftIndicators {
            price: r.price,
            currency: r.currency,
            pe: r.pe,
            pb: r.pb,
            roe: r.roe,
            dividend_yield_12m: r.dividend_yield_12m,
            dividend_yield_5y_avg: r.dividend_yield_5y_avg,
            fetched_at: r.fundamentals_fetched_at,
        }),
        ticker,
        today,
        valuations,
    };
    Ok(render_draft(&input))
}

/// Rascunho de tese de FII. Só lê o que já foi coletado (cotação, CNPJ em cache, CVM): sem rede.
pub async fn build_fii_draft_on(db: &DatabaseConnection, ticker: &str, today: NaiveDate) -> Result<Draft, AppError> {
    let ticker = ticker.trim().to_uppercase();
    let quote = stock_quotes::Entity::find()
        .filter(stock_quotes::Column::Ticker.eq(ticker.as_str()))
        .order_by_desc(stock_quotes::Column::FetchedAt)
        .one(db)
        .await?;
    let cnpj = fii_cnpj_cache::Entity::find()
        .filter(fii_cnpj_cache::Column::Ticker.eq(ticker.as_str()))
        .one(db)
        .await?
        .map(|c| c.cnpj);

    let (monthly, properties) = match cnpj {
        Some(cnpj) => {
            let monthly = fii_cvm_monthly::Entity::find()
                .filter(fii_cvm_monthly::Column::Cnpj.eq(cnpj.as_str()))
                .order_by_desc(fii_cvm_monthly::Column::ReferenceDate)
                .one(db)
                .await?
                .map(|m| FiiDraftMonthly {
                    reference_date: m.reference_date,
                    net_assets: m.patrimonio_liquido,
                    book_value_per_share: m.valor_patrimonial_cota,
                    shareholders: m.numero_cotistas,
                    dividend_yield_month: m.dividend_yield_mes,
                    effective_return_month: m.rentabilidade_efetiva_mes,
                });
            let rows = fii_cvm_properties::Entity::find()
                .filter(fii_cvm_properties::Column::Cnpj.eq(cnpj.as_str()))
                .order_by_desc(fii_cvm_properties::Column::ReferenceDate)
                .all(db)
                .await?;
            let latest_date = rows.first().map(|p| p.reference_date.clone());
            let properties = rows
                .into_iter()
                .filter(|p| Some(&p.reference_date) == latest_date.as_ref())
                .map(|p| FiiDraftProperty {
                    name: p.nome_imovel,
                    area_m2: p.area_m2,
                    vacancy: p.percentual_vacancia,
                    delinquency: p.percentual_inadimplencia,
                    leased: p.percentual_locado,
                })
                .collect();
            (monthly, properties)
        }
        None => (None, Vec::new()),
    };

    let input = FiiDraftInput {
        name: quote.as_ref().and_then(|q| q.name.clone()),
        price: quote.as_ref().map(|q| q.price),
        currency: quote.and_then(|q| q.currency),
        ticker,
        today,
        monthly,
        properties,
    };
    Ok(render_fii_draft(&input))
}

/// Rascunho de tese de ETF ou BDR. Só lê o que já foi coletado (cotação, técnicos, dividendo médio
/// e, no BDR, a classe subjacente do ativo cadastrado): sem rede.
pub async fn build_fund_draft_on(db: &DatabaseConnection, ticker: &str, kind: FundKind, today: NaiveDate) -> Result<Draft, AppError> {
    let ticker = ticker.trim().to_uppercase();
    let quote = stock_quotes::Entity::find()
        .filter(stock_quotes::Column::Ticker.eq(ticker.as_str()))
        .order_by_desc(stock_quotes::Column::FetchedAt)
        .one(db)
        .await?;
    let technicals = stock_technicals::Entity::find()
        .filter(stock_technicals::Column::Ticker.eq(ticker.as_str()))
        .order_by_desc(stock_technicals::Column::FetchedAt)
        .one(db)
        .await?
        .map(|t| FundDraftTechnicals {
            sma_50: t.sma_50,
            sma_100: t.sma_100,
            sma_200: t.sma_200,
            cagr_5y: t.cagr_5y,
            cagr_10y: t.cagr_10y,
            fetched_at: t.fetched_at,
        });
    let avg_dividend_5y = stock_dividends_avg::Entity::find()
        .filter(stock_dividends_avg::Column::Ticker.eq(ticker.as_str()))
        .order_by_desc(stock_dividends_avg::Column::FetchedAt)
        .one(db)
        .await?
        .map(|d| d.avg_dividend_5y);
    let underlying = match kind {
        FundKind::Bdr => assets::Entity::find()
            .filter(assets::Column::Ticker.eq(ticker.as_str()))
            .one(db)
            .await?
            .and_then(|a| a.underlying_asset_class),
        FundKind::Etf => None,
    };

    let input = FundDraftInput {
        kind,
        name: quote.as_ref().and_then(|q| q.name.clone()),
        price: quote.as_ref().map(|q| q.price),
        currency: quote.and_then(|q| q.currency),
        ticker,
        today,
        technicals,
        avg_dividend_5y,
        underlying,
    };
    Ok(render_fund_draft(&input))
}

/// Rascunho de tese de cripto. Só lê o que já foi coletado (última cotação, histórico de
/// fechamentos, último Fear & Greed e o score da moeda): sem rede. O score é visto na data `today`.
pub async fn build_crypto_draft_on(db: &DatabaseConnection, ticker: &str, today: NaiveDate) -> Result<Draft, AppError> {
    let ticker = ticker.trim().to_uppercase();
    let quote = stock_quotes::Entity::find()
        .filter(stock_quotes::Column::Ticker.eq(ticker.as_str()))
        .order_by_desc(stock_quotes::Column::FetchedAt)
        .one(db)
        .await?;
    // Linha com data ilegível é pulada: um ponto ruim não derruba o rascunho.
    let mut history: Vec<CryptoPricePoint> = stock_price_history::Entity::find()
        .filter(stock_price_history::Column::Ticker.eq(ticker.as_str()))
        .all(db)
        .await?
        .into_iter()
        .filter_map(|h| {
            let date = NaiveDate::parse_from_str(&h.price_date, "%Y-%m-%d").ok()?;
            Some(CryptoPricePoint { date, close: h.close_price })
        })
        .collect();
    history.sort_by_key(|p| p.date);
    let fear_greed = crypto_fear_greed::Entity::find()
        .order_by_desc(crypto_fear_greed::Column::ReadingDate)
        .one(db)
        .await?
        .map(|f| CryptoDraftFearGreed { value: f.value, classification: f.classification, reading_date: f.reading_date });
    let score = crypto_score_as_of(db, ticker.clone(), today).await?;

    let input = CryptoDraftInput {
        name: quote.as_ref().and_then(|q| q.name.clone()),
        price: quote.as_ref().map(|q| q.price),
        currency: quote.and_then(|q| q.currency),
        ticker,
        today,
        history,
        fear_greed,
        score: Some(score),
    };
    Ok(render_crypto_draft(&input))
}

#[tauri::command]
pub async fn build_crypto_thesis_draft(db: tauri::State<'_, DatabaseConnection>, ticker: String) -> Result<Draft, AppError> {
    build_crypto_draft_on(db.inner(), ticker.trim(), Utc::now().date_naive()).await
}

#[tauri::command]
pub async fn build_fund_thesis_draft(
    db: tauri::State<'_, DatabaseConnection>,
    ticker: String,
    kind: FundKind,
) -> Result<Draft, AppError> {
    build_fund_draft_on(db.inner(), ticker.trim(), kind, Utc::now().date_naive()).await
}

#[tauri::command]
pub async fn build_fii_thesis_draft(db: tauri::State<'_, DatabaseConnection>, ticker: String) -> Result<Draft, AppError> {
    build_fii_draft_on(db.inner(), ticker.trim(), Utc::now().date_naive()).await
}

#[tauri::command]
pub async fn build_thesis_draft(db: tauri::State<'_, DatabaseConnection>, ticker: String) -> Result<Draft, AppError> {
    build_draft_on(db.inner(), ticker.trim(), Utc::now().date_naive()).await
}

#[cfg(test)]
mod tests {
    use sea_orm::{ActiveModelTrait, Set};

    use super::*;
    use crate::entity::{
        assets, fii_cnpj_cache, fii_cvm_monthly, fii_cvm_properties, stock_dividends_avg, stock_fundamentals,
        stock_quotes, stock_technicals,
    };
    use crate::commands::crypto_indicator::{record_crypto_indicator_in, RecordCryptoIndicatorRequest};
    use crate::entity::{crypto_fear_greed, stock_price_history};
    use crate::test_support::{asset, memory_db};

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 20).unwrap()
    }

    async fn collected(db: &DatabaseConnection, ticker: &str) {
        let at = "2026-11-01T10:00:00+00:00";
        stock_quotes::ActiveModel {
            ticker: Set(ticker.to_string()),
            price: Set(20.0),
            source: Set("test".to_string()),
            fetched_at: Set(at.to_string()),
            name: Set(Some("Banco do Brasil".to_string())),
            exchange: Set(None),
            currency: Set(Some("BRL".to_string())),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("insert quote");
        stock_fundamentals::ActiveModel {
            ticker: Set(ticker.to_string()),
            lpa: Set(2.0),
            vpa: Set(10.0),
            roe: Set(15.0),
            source: Set("test".to_string()),
            fetched_at: Set(at.to_string()),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("insert fundamentals");
    }

    async fn saved(db: &DatabaseConnection, ticker: &str, model: &str, fair: f64, at: &str) {
        valuation::ActiveModel {
            ticker: Set(ticker.to_string()),
            reference_year: Set(2026),
            current_price: Set(20.0),
            model: Set(model.to_string()),
            fair_price: Set(Some(fair)),
            safety_margin: Set(Some((fair - 20.0) / fair)),
            verdict: Set(Some("CHEAP".to_string())),
            updated_at: Set(at.to_string()),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("insert valuation");
    }

    #[tokio::test]
    async fn the_draft_uses_the_collected_numbers_and_the_latest_valuation_of_each_model() {
        let db = memory_db().await;
        collected(&db, "BBAS3").await;
        saved(&db, "BBAS3", "bazin", 24.0, "2026-08-01T00:00:00+00:00").await;
        saved(&db, "BBAS3", "bazin", 25.0, "2026-10-01T00:00:00+00:00").await;
        saved(&db, "OUTRA3", "bazin", 99.0, "2026-10-02T00:00:00+00:00").await;

        let draft = build_draft_on(&db, "bbas3", today()).await.unwrap();
        let text = draft.content_markdown;
        assert_eq!(draft.title, "BBAS3 thesis");
        assert!(text.starts_with("# BBAS3 — Banco do Brasil\n"), "{text}");
        assert!(text.contains("## Snapshot (2026-11-20)"), "{text}");
        assert!(text.contains("- P/E: 10.00"), "{text}");
        assert!(text.contains("- P/B: 2.00"), "{text}");
        assert!(text.contains("- bazin: fair price 25.00, margin 20.0%, CHEAP (updated 2026-10-01)"), "{text}");
        assert!(!text.contains("24.00"), "o valuation antigo do mesmo modelo não entra:\n{text}");
        assert!(!text.contains("99.00"), "valuation de outro ticker não entra:\n{text}");
    }

    const CNPJ: &str = "11.111.111/0001-11";

    async fn fii_quote(db: &DatabaseConnection, ticker: &str, price: f64, fetched_at: &str) {
        stock_quotes::ActiveModel {
            ticker: Set(ticker.to_string()),
            price: Set(price),
            source: Set("test".to_string()),
            fetched_at: Set(fetched_at.to_string()),
            name: Set(Some("CSHG Logística".to_string())),
            exchange: Set(None),
            currency: Set(Some("BRL".to_string())),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("insert quote");
    }

    async fn fii_cnpj(db: &DatabaseConnection, ticker: &str, cnpj: &str) {
        fii_cnpj_cache::ActiveModel {
            ticker: Set(ticker.to_string()),
            cnpj: Set(cnpj.to_string()),
            fund_name: Set("Fundo".to_string()),
            resolved_at: Set("2026-11-01T10:00:00+00:00".to_string()),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("insert cnpj");
    }

    async fn fii_month(db: &DatabaseConnection, cnpj: &str, reference_date: &str, book_value: f64) {
        fii_cvm_monthly::ActiveModel {
            cnpj: Set(cnpj.to_string()),
            reference_date: Set(reference_date.to_string()),
            patrimonio_liquido: Set(3_000_000_000.0),
            valor_patrimonial_cota: Set(book_value),
            numero_cotistas: Set(Some(150_000)),
            dividend_yield_mes: Set(Some(0.008)),
            rentabilidade_efetiva_mes: Set(Some(0.012)),
            source: Set("test".to_string()),
            fetched_at: Set("2026-11-01T10:00:00+00:00".to_string()),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("insert month");
    }

    async fn fii_property(db: &DatabaseConnection, cnpj: &str, reference_date: &str, name: &str, vacancy: f64) {
        fii_cvm_properties::ActiveModel {
            cnpj: Set(cnpj.to_string()),
            reference_date: Set(reference_date.to_string()),
            nome_imovel: Set(name.to_string()),
            area_m2: Set(Some(100.0)),
            percentual_vacancia: Set(Some(vacancy)),
            percentual_inadimplencia: Set(Some(0.0)),
            percentual_locado: Set(Some(1.0 - vacancy)),
            source: Set("test".to_string()),
            fetched_at: Set("2026-11-01T10:00:00+00:00".to_string()),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("insert property");
    }

    #[tokio::test]
    async fn the_fii_draft_uses_the_latest_quote_month_and_property_date() {
        let db = memory_db().await;
        fii_cnpj(&db, "HGLG11", CNPJ).await;
        fii_quote(&db, "HGLG11", 150.0, "2026-10-01T10:00:00+00:00").await;
        fii_quote(&db, "HGLG11", 160.0, "2026-11-01T10:00:00+00:00").await;
        fii_month(&db, CNPJ, "2026-09-30", 100.0).await;
        fii_month(&db, CNPJ, "2026-10-31", 200.0).await;
        fii_property(&db, CNPJ, "2026-06-30", "Antigo", 0.50).await;
        fii_property(&db, CNPJ, "2026-09-30", "Galpão A", 0.10).await;

        let draft = build_fii_draft_on(&db, "hglg11", today()).await.unwrap();
        let text = draft.content_markdown;
        assert_eq!(draft.title, "HGLG11 thesis");
        assert!(text.starts_with("# HGLG11 — CSHG Logística\n"), "{text}");
        assert!(text.contains("- Price: 160.00 BRL"), "a cotação mais nova: {text}");
        assert!(text.contains("CVM reference 2026-10-31."), "o mês mais novo: {text}");
        assert!(text.contains("- P/VP: 0.80"), "160 / 200: {text}");
        assert!(text.contains("- Properties: 1"), "só a data de referência mais nova: {text}");
        assert!(text.contains("- Vacancy (area-weighted): 10.0%"), "{text}");
    }

    #[tokio::test]
    async fn a_fii_without_a_cached_cnpj_gets_the_notice_and_the_quote_only() {
        let db = memory_db().await;
        fii_quote(&db, "HGLG11", 160.0, "2026-11-01T10:00:00+00:00").await;
        let text = build_fii_draft_on(&db, "HGLG11", today()).await.unwrap().content_markdown;
        assert!(text.contains("No CVM data for HGLG11 yet (n/d)."), "{text}");
        assert!(text.contains("- Price: 160.00 BRL"), "{text}");
        assert!(text.contains("## Why own it"), "{text}");
    }

    #[tokio::test]
    async fn another_funds_data_does_not_leak_into_the_draft() {
        let db = memory_db().await;
        fii_cnpj(&db, "HGLG11", CNPJ).await;
        fii_cnpj(&db, "XPLG11", "22.222.222/0001-22").await;
        fii_quote(&db, "HGLG11", 160.0, "2026-11-01T10:00:00+00:00").await;
        fii_month(&db, CNPJ, "2026-10-31", 200.0).await;
        fii_month(&db, "22.222.222/0001-22", "2026-10-31", 999.0).await;
        fii_property(&db, "22.222.222/0001-22", "2026-10-31", "Do outro", 0.77).await;
        let text = build_fii_draft_on(&db, "HGLG11", today()).await.unwrap().content_markdown;
        assert!(text.contains("- Book value per share: 200.00"), "{text}");
        assert!(!text.contains("999.00"), "{text}");
        assert!(text.contains("No property data (n/d)."), "{text}");
    }

    #[tokio::test]
    async fn an_unknown_fii_gets_the_skeleton_and_no_error() {
        let db = memory_db().await;
        let text = build_fii_draft_on(&db, "ZZZZ11", today()).await.unwrap().content_markdown;
        assert!(text.contains("No CVM data for ZZZZ11 yet (n/d)."), "{text}");
        assert!(text.contains("- Price: n/d"), "{text}");
    }

    async fn fund_technicals(db: &DatabaseConnection, ticker: &str, sma_50: f64, fetched_at: &str) {
        stock_technicals::ActiveModel {
            ticker: Set(ticker.to_string()),
            sma_50: Set(Some(sma_50)),
            sma_100: Set(Some(125.0)),
            sma_200: Set(None),
            cagr_5y: Set(Some(12.5)),
            cagr_10y: Set(Some(9.0)),
            source: Set("test".to_string()),
            fetched_at: Set(fetched_at.to_string()),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("insert technicals");
    }

    async fn fund_dividend_avg(db: &DatabaseConnection, ticker: &str, avg: f64, fetched_at: &str) {
        stock_dividends_avg::ActiveModel {
            ticker: Set(ticker.to_string()),
            avg_dividend_5y: Set(avg),
            source: Set("test".to_string()),
            fetched_at: Set(fetched_at.to_string()),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("insert dividend avg");
    }

    #[tokio::test]
    async fn the_fund_draft_uses_the_latest_quote_technicals_and_dividend_average() {
        let db = memory_db().await;
        fii_quote(&db, "BOVA11", 120.0, "2026-10-01T10:00:00+00:00").await;
        fii_quote(&db, "BOVA11", 130.0, "2026-11-01T10:00:00+00:00").await;
        fund_technicals(&db, "BOVA11", 100.0, "2026-10-01T10:00:00+00:00").await;
        fund_technicals(&db, "BOVA11", 128.5, "2026-11-01T10:00:00+00:00").await;
        fund_dividend_avg(&db, "BOVA11", 1.0, "2026-10-01T10:00:00+00:00").await;
        fund_dividend_avg(&db, "BOVA11", 1.2, "2026-11-01T10:00:00+00:00").await;

        let draft = build_fund_draft_on(&db, "bova11", FundKind::Etf, today()).await.unwrap();
        let text = draft.content_markdown;
        assert_eq!(draft.title, "BOVA11 thesis");
        assert!(text.contains("- Class: ETF"), "{text}");
        assert!(text.contains("- Price: 130.00 BRL"), "a cotação mais nova: {text}");
        assert!(text.contains("- SMA 50: 128.50"), "os técnicos mais novos: {text}");
        assert!(text.contains("- SMA 200: n/d"), "{text}");
        assert!(text.contains("- CAGR 5y: 12.5%"), "{text}");
        assert!(text.contains("- Avg dividend/share (5y): 1.20"), "o dividendo médio mais novo: {text}");
    }

    #[tokio::test]
    async fn another_funds_data_does_not_leak_into_the_fund_draft() {
        let db = memory_db().await;
        fii_quote(&db, "BOVA11", 130.0, "2026-11-01T10:00:00+00:00").await;
        fund_technicals(&db, "OUTRO11", 999.0, "2026-11-02T10:00:00+00:00").await;
        fund_dividend_avg(&db, "OUTRO11", 77.0, "2026-11-02T10:00:00+00:00").await;
        let text = build_fund_draft_on(&db, "BOVA11", FundKind::Etf, today()).await.unwrap().content_markdown;
        assert!(text.contains("- SMA 50: n/d"), "{text}");
        assert!(!text.contains("999.00") && !text.contains("77.00"), "{text}");
    }

    #[tokio::test]
    async fn the_bdr_draft_shows_the_underlying_class_of_the_registered_asset() {
        let db = memory_db().await;
        fii_quote(&db, "AAPL34", 60.0, "2026-11-01T10:00:00+00:00").await;
        let registered = asset(&db, "AAPL34", "bdr").await;
        let mut active: assets::ActiveModel = registered.into();
        active.underlying_asset_class = Set(Some("acao_internacional".to_string()));
        active.update(&db).await.expect("set underlying");

        let text = build_fund_draft_on(&db, "AAPL34", FundKind::Bdr, today()).await.unwrap().content_markdown;
        assert!(text.contains("- Class: BDR"), "{text}");
        assert!(text.contains("- Underlying: acao_internacional"), "{text}");
    }

    #[tokio::test]
    async fn a_bdr_without_a_registered_asset_has_no_underlying_line() {
        let db = memory_db().await;
        fii_quote(&db, "AAPL34", 60.0, "2026-11-01T10:00:00+00:00").await;
        let text = build_fund_draft_on(&db, "AAPL34", FundKind::Bdr, today()).await.unwrap().content_markdown;
        assert!(!text.contains("Underlying"), "{text}");
    }

    async fn crypto_quote(db: &DatabaseConnection, ticker: &str, price: f64, fetched_at: &str) {
        stock_quotes::ActiveModel {
            ticker: Set(ticker.to_string()),
            price: Set(price),
            source: Set("coingecko".to_string()),
            fetched_at: Set(fetched_at.to_string()),
            name: Set(Some("Ethereum".to_string())),
            exchange: Set(Some("CoinGecko".to_string())),
            currency: Set(Some("USD".to_string())),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("insert crypto quote");
    }

    async fn close_on(db: &DatabaseConnection, ticker: &str, date: &str, close: f64) {
        stock_price_history::ActiveModel {
            ticker: Set(ticker.to_string()),
            price_date: Set(date.to_string()),
            close_price: Set(close),
            source: Set("coingecko".to_string()),
            fetched_at: Set("2026-11-20T10:00:00+00:00".to_string()),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("insert close");
    }

    async fn fear_greed(db: &DatabaseConnection, value: i32, classification: &str, reading_date: &str, fetched_at: &str) {
        crypto_fear_greed::ActiveModel {
            value: Set(value),
            classification: Set(classification.to_string()),
            reading_date: Set(reading_date.to_string()),
            source: Set("alternative.me".to_string()),
            fetched_at: Set(fetched_at.to_string()),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("insert fear greed");
    }

    fn indicator_on(coin: &str, indicator: &str, reading_date: &str, raw_value: f64) -> RecordCryptoIndicatorRequest {
        RecordCryptoIndicatorRequest {
            coin: coin.to_string(),
            indicator: indicator.to_string(),
            reading_date: reading_date.to_string(),
            raw_value,
            source: "test".to_string(),
        }
    }

    #[tokio::test]
    async fn the_crypto_draft_uses_the_latest_quote_the_ordered_history_fear_greed_and_score() {
        let db = memory_db().await;
        crypto_quote(&db, "ETH", 3000.0, "2026-11-10T10:00:00+00:00").await;
        crypto_quote(&db, "ETH", 3300.0, "2026-11-20T10:00:00+00:00").await;
        // Fora de ordem de propósito: o rascunho tem que ordenar por data.
        close_on(&db, "ETH", "2026-11-20", 120.0).await;
        close_on(&db, "ETH", "2026-11-13", 100.0).await;
        close_on(&db, "ETH", "2026-11-17", 110.0).await;
        // O mais novo por data da leitura, não por quando foi gravado.
        fear_greed(&db, 40, "Fear", "2026-11-18", "2026-11-21T09:00:00+00:00").await;
        fear_greed(&db, 72, "Greed", "2026-11-19", "2026-11-19T09:00:00+00:00").await;
        record_crypto_indicator_in(&db, indicator_on("ETH", "funding_rate_7d", "2026-11-20", 3.0)).await.unwrap();

        let draft = build_crypto_draft_on(&db, "eth", today()).await.unwrap();
        let text = draft.content_markdown;
        assert_eq!(draft.title, "ETH thesis");
        assert!(text.starts_with("# ETH — Ethereum\n"), "{text}");
        assert!(text.contains("- Price: 3300.00 USD"), "a cotação mais nova: {text}");
        assert!(text.contains("- Price history up to: 2026-11-20"), "{text}");
        assert!(text.contains("- Change 7d: +20.0%"), "{text}");
        assert!(text.contains("- Fear & Greed: 72 (Greed), read on 2026-11-19"), "{text}");
        assert!(text.contains("green (1 logged)"), "o score tem a leitura de hoje: {text}");
    }

    #[tokio::test]
    async fn another_coins_data_does_not_leak_into_the_crypto_draft() {
        let db = memory_db().await;
        crypto_quote(&db, "ETH", 3300.0, "2026-11-20T10:00:00+00:00").await;
        crypto_quote(&db, "BTC", 99999.0, "2026-11-20T10:00:00+00:00").await;
        close_on(&db, "BTC", "2026-11-20", 99999.0).await;
        record_crypto_indicator_in(&db, indicator_on("BTC", "funding_rate_7d", "2026-11-20", 3.0)).await.unwrap();

        let text = build_crypto_draft_on(&db, "ETH", today()).await.unwrap().content_markdown;
        assert!(text.contains("- Price: 3300.00 USD"), "{text}");
        assert!(text.contains("- Price history up to: n/d"), "{text}");
        assert!(text.contains("- Crypto score: n/d"), "{text}");
        assert!(!text.contains("99999"), "{text}");
    }

    #[tokio::test]
    async fn the_crypto_score_is_seen_on_the_snapshot_date_not_on_the_clock() {
        let db = memory_db().await;
        crypto_quote(&db, "ETH", 3300.0, "2026-11-20T10:00:00+00:00").await;
        // Funding vale 7 dias: lida em 2026-11-12, venceu em 2026-11-20 (8 dias).
        record_crypto_indicator_in(&db, indicator_on("ETH", "funding_rate_7d", "2026-11-12", 3.0)).await.unwrap();
        let text = build_crypto_draft_on(&db, "ETH", today()).await.unwrap().content_markdown;
        assert!(text.contains("- Crypto score: n/d"), "{text}");
        assert!(text.contains("0 logged, 1 expired"), "{text}");
    }

    #[tokio::test]
    async fn a_history_row_with_an_unreadable_date_is_skipped_not_fatal() {
        let db = memory_db().await;
        crypto_quote(&db, "ETH", 3300.0, "2026-11-20T10:00:00+00:00").await;
        close_on(&db, "ETH", "not-a-date", 1.0).await;
        close_on(&db, "ETH", "2026-11-20", 120.0).await;
        let text = build_crypto_draft_on(&db, "ETH", today()).await.unwrap().content_markdown;
        assert!(text.contains("- Price history up to: 2026-11-20"), "{text}");
    }

    #[tokio::test]
    async fn an_unknown_crypto_gets_the_skeleton_and_no_error() {
        let db = memory_db().await;
        let text = build_crypto_draft_on(&db, "ZZZ", today()).await.unwrap().content_markdown;
        assert!(text.contains("No collected data for ZZZ yet (n/d)."), "{text}");
        assert!(text.contains("## Why own it"), "{text}");
    }

    #[tokio::test]
    async fn an_unknown_fund_gets_the_skeleton_and_no_error() {
        let db = memory_db().await;
        let text = build_fund_draft_on(&db, "ZZZZ11", FundKind::Etf, today()).await.unwrap().content_markdown;
        assert!(text.contains("No collected data for ZZZZ11 yet (n/d)."), "{text}");
        assert!(text.contains("## Why own it"), "{text}");
    }

    #[tokio::test]
    async fn an_unknown_ticker_gets_the_skeleton_and_no_error() {
        let db = memory_db().await;
        let draft = build_draft_on(&db, "ZZZZ3", today()).await.unwrap();
        let text = draft.content_markdown;
        assert!(text.contains("No collected data for ZZZZ3 yet (n/d)."), "{text}");
        assert!(text.contains("No saved valuation (n/d)."), "{text}");
        assert!(text.contains("## Why own it"), "{text}");
    }
}
