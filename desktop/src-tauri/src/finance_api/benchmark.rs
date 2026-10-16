// Fase 14.4 (fatia final, Sessão 92) — porta `data-collector/main.py::
// collect_benchmark_returns` (Fase 13.5) pra Rust. CDI/IPCA/IFIX/SMLL/IDIV/
// IVVB11 já eram portáveis desde a Fase 1.7 do easybusiness (endpoints já
// existentes em `finance_api::client`, sem chamador até agora); só o IBOV
// (`^BVSP`, Yahoo sem sufixo) esperava a Fase 1.11 — reaproveita
// `finance_api::us_stock::collect_price_history` sem tratamento especial,
// mesma decisão do Python (`collect_us_price_history(["^BVSP"])`).
//
// `commands::profitability::get_profitability_comparison` lê estas linhas
// exatamente pelos `index_code`/`ticker` usados abaixo (`"cdi"`/`"ipca"`/
// `"selic"` em `macro_index_monthly`; `"^BVSP"`/`"IVVB11"`/`"IFIX"`/`"SMLL"`/
// `"IDIV"` em `stock_price_history`) — não mudar sem atualizar os dois lados.
use chrono::{NaiveDate, Utc};
use sea_orm::sea_query::OnConflict;
use sea_orm::{DatabaseConnection, EntityTrait, Set};

use crate::domain::rates;
use crate::entity::{macro_index_monthly, stock_price_history};
use crate::error::AppError;
use crate::finance_api::{client, insert_ignoring_conflicts, stocks, us_stock, FinanceApiHandle};

pub struct BenchmarkSummary {
    pub cdi_count: usize,
    pub ipca_count: usize,
    pub selic_count: usize,
    pub ibov_count: usize,
    pub ivvb11_count: usize,
    pub ifix_count: usize,
    pub smll_count: usize,
    pub idiv_count: usize,
}

/// CDI/IPCA usam upsert de verdade (`ON CONFLICT ... DO UPDATE`), não
/// `DO NOTHING` — a fonte pode revisar um valor recém-publicado do mês
/// corrente, e rodar de novo deve refletir o número mais atual (mesma
/// distinção que o Python já fazia: `INSERT OR REPLACE`, diferente do resto
/// do coletor que lida com histórico fechado).
async fn collect_macro_series(
    db: &DatabaseConnection,
    handle: &FinanceApiHandle,
    series_code: &str,
) -> Result<usize, AppError> {
    let response = client::fetch_benchmark_series(handle, series_code).await?;
    let rows: Vec<(String, f64)> = response
        .data
        .iter()
        .map(|point| (point.reference_month.format("%Y-%m").to_string(), point.value_pct))
        .collect();
    upsert_monthly(db, series_code, "bcb_sgs", rows).await
}

/// Fase 17 — Selic como benchmark: a fonte é a Meta Selic diária (% a.a.),
/// convertida aqui em retorno mensal (% no mês, mesma unidade do CDI) por
/// `domain::rates::selic_monthly_returns`. Mês corrente parcial é reescrito
/// a cada coleta (upsert).
async fn collect_selic_series(db: &DatabaseConnection) -> Result<usize, AppError> {
    let response = client::fetch_selic(db, "meta").await?;
    let points: Vec<(NaiveDate, f64)> =
        response.data.iter().map(|p| (p.reference_date, p.value_pct)).collect();
    let rows = rates::selic_monthly_returns(&points, Utc::now().date_naive());
    upsert_monthly(db, "selic", "bcb_sgs_meta_selic", rows).await
}

async fn upsert_monthly(
    db: &DatabaseConnection,
    index_code: &str,
    source: &str,
    rows: Vec<(String, f64)>,
) -> Result<usize, AppError> {
    let now = Utc::now().to_rfc3339();
    let models: Vec<_> = rows
        .into_iter()
        .map(|(year_month, value_pct)| macro_index_monthly::ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            index_code: Set(index_code.to_string()),
            year_month: Set(year_month),
            value_pct: Set(value_pct),
            source: Set(source.to_string()),
            fetched_at: Set(now.clone()),
        })
        .collect();

    let count = models.len();
    if !models.is_empty() {
        macro_index_monthly::Entity::insert_many(models)
            .on_conflict(
                OnConflict::columns([
                    macro_index_monthly::Column::IndexCode,
                    macro_index_monthly::Column::YearMonth,
                ])
                .update_columns([
                    macro_index_monthly::Column::ValuePct,
                    macro_index_monthly::Column::Source,
                    macro_index_monthly::Column::FetchedAt,
                ])
                .to_owned(),
            )
            .exec(db)
            .await?;
    }

    Ok(count)
}

/// IFIX/SMLL/IDIV via `/v1/b3-indexes/{index_code}/history` — grava com o
/// `index_code` **maiúsculo** originalmente passado (não o que a API ecoa
/// de volta, que é minúsculo), mesma decisão do Python
/// (`collect_b3_index_history` grava o `index_code` recebido, não o campo
/// da resposta) — `commands::profitability` espera `"IFIX"`/`"SMLL"`/
/// `"IDIV"` em `stock_price_history.ticker`.
async fn collect_b3_index_history(
    db: &DatabaseConnection,
    handle: &FinanceApiHandle,
    index_code: &str,
) -> Result<usize, AppError> {
    let now = Utc::now().to_rfc3339();
    let response = client::fetch_b3_index_history(handle, index_code).await?;

    let total = response.data.len();
    let models: Vec<_> = response
        .data
        .into_iter()
        .map(|point| stock_price_history::ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            ticker: Set(index_code.to_string()),
            price_date: Set(point.price_date.to_string()),
            close_price: Set(point.close_price),
            source: Set("b3_index_stats".to_string()),
            fetched_at: Set(now.clone()),
        })
        .collect();

    insert_ignoring_conflicts::<stock_price_history::Entity>(
        db,
        models,
        OnConflict::columns([
            stock_price_history::Column::Ticker,
            stock_price_history::Column::PriceDate,
        ])
        .do_nothing()
        .to_owned(),
    )
    .await?;

    Ok(total)
}

pub async fn collect_benchmarks(
    db: &DatabaseConnection,
    handle: &FinanceApiHandle,
) -> Result<BenchmarkSummary, AppError> {
    let cdi_count = collect_macro_series(db, handle, "cdi").await?;
    let ipca_count = collect_macro_series(db, handle, "ipca").await?;
    // Selic não derruba o resto: a série diária é a mais frágil da coleta
    // (~10 mil pontos em 4 janelas do BCB, que oscila — ver Fase 17).
    // Falhou → 0 meses agora, tenta de novo na próxima coleta.
    let selic_count = match collect_selic_series(db).await {
        Ok(count) => count,
        Err(err) => {
            eprintln!("collect_benchmarks: Selic skipped — {err}");
            0
        }
    };

    // IBOV (^BVSP, sem sufixo — é índice, não papel B3): mesmo mecanismo de
    // "ticker sem sufixo" que ação americana/REIT/ETF-US usam.
    let ibov_count = us_stock::collect_price_history(db, handle, &["^BVSP".to_string()]).await?;
    // IVVB11 (ETF B3 comum, sufixo .SA padrão): já portável, mesma função
    // que Ação BR/FII/ETF-BR usa.
    let ivvb11_count =
        stocks::collect_price_history(db, handle, &["IVVB11".to_string()]).await?;

    let ifix_count = collect_b3_index_history(db, handle, "IFIX").await?;
    let smll_count = collect_b3_index_history(db, handle, "SMLL").await?;
    let idiv_count = collect_b3_index_history(db, handle, "IDIV").await?;

    Ok(BenchmarkSummary {
        cdi_count,
        ipca_count,
        selic_count,
        ibov_count,
        ivvb11_count,
        ifix_count,
        smll_count,
        idiv_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handle() -> FinanceApiHandle {
        FinanceApiHandle::for_test(
            "http://localhost:8000".to_string(),
            "local-dev-key-change-me".to_string(),
        )
    }

    async fn dev_db() -> DatabaseConnection {
        sea_orm::Database::connect("sqlite:///data/anchor.db?mode=rwc")
            .await
            .expect("failed to connect to the real dev database")
    }

    #[tokio::test]
    #[ignore]
    async fn live_collect_benchmarks_writes_all_8_series() {
        let db = dev_db().await;
        let handle = handle();

        let summary = collect_benchmarks(&db, &handle).await.unwrap();
        assert!(summary.cdi_count > 0);
        assert!(summary.ipca_count > 0);
        // Meta Selic desde 1999 → bem mais de 300 meses.
        assert!(summary.selic_count > 300);
        assert!(summary.ibov_count > 0);
        assert!(summary.ivvb11_count > 0);
        assert!(summary.ifix_count > 0);
        assert!(summary.smll_count > 0);
        assert!(summary.idiv_count > 0);
    }

    #[tokio::test]
    #[ignore]
    async fn live_collect_macro_series_upserts_not_duplicates() {
        use sea_orm::{ColumnTrait, QueryFilter};

        let db = dev_db().await;
        let handle = handle();

        collect_macro_series(&db, &handle, "cdi").await.unwrap();
        let before = macro_index_monthly::Entity::find()
            .filter(macro_index_monthly::Column::IndexCode.eq("cdi"))
            .all(&db)
            .await
            .unwrap()
            .len();
        collect_macro_series(&db, &handle, "cdi").await.unwrap();
        let after = macro_index_monthly::Entity::find()
            .filter(macro_index_monthly::Column::IndexCode.eq("cdi"))
            .all(&db)
            .await
            .unwrap()
            .len();
        assert_eq!(before, after, "re-running CDI must not duplicate rows (upsert, not insert)");
    }
}
