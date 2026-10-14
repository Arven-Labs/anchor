use std::collections::{HashMap, HashSet};

use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};

use crate::domain::dividend_suggestion::{quantity_held_by, within_tolerance, LedgerRow};
use crate::domain::proventos::{PAYMENT_TYPE_DIVIDENDO, PAYMENT_TYPE_JSCP};
use crate::entity::{
    assets, portfolio, stock_dividend_payments, suggested_dividends, transactions,
};
use crate::error::AppError;

const STATUS_PENDING: &str = "pending";
const STATUS_MATCHED: &str = "matched";
const STATUS_CONFIRMED: &str = "confirmed";
const STATUS_DISCARDED: &str = "discarded";
const STATUS_DIVERGENT: &str = "divergent";

// Provento esperado cadastrado à mão pelo dono do projeto (Sessão 77.2) —
// costuma ter data futura; quando a fonte automática trouxer o pagamento de
// verdade, o `generate_` concilia em vez de duplicar.
const SOURCE_MANUAL: &str = "manual";

// Tolerância relativa (±10%) pra considerar que o valor/cota que a fonte
// trouxe "bateu" com o que o dono do projeto previu (decisão Sessão 77.2).
const MATCH_TOLERANCE_PCT: f64 = 10.0;

// Fase 13.4 — Dividendo vs. JSCP, ver `domain::proventos` pro cálculo de IR.
fn validate_payment_type(payment_type: &Option<String>) -> Result<(), AppError> {
    match payment_type {
        Some(value) if value != PAYMENT_TYPE_DIVIDENDO && value != PAYMENT_TYPE_JSCP => {
            Err(AppError::InvalidGuard(format!(
                "payment_type '{value}' inválido (esperado '{PAYMENT_TYPE_DIVIDENDO}' ou '{PAYMENT_TYPE_JSCP}')"
            )))
        }
        _ => Ok(()),
    }
}

// Fase 13.6 — parte "passado", só histórico Yahoo (`stock_dividend_payments`,
// fonte `"yahoo_finance"`). Desenho: pra cada pagamento de provento
// registrado de um ticker que o portfolio tem/lanceu, reconstruir a
// quantidade na data de pagamento (compras − vendas até a data, mesma regra
// de `get_portfolio_positions`) e gerar uma "sugestão" pendente — nunca
// grava direto no ledger. O dono do projeto confirma (vira `provento` de
// verdade em `transactions`, com Data Com manual, decisão da Sessão 76),
// edita a quantidade ou descarta, numa tela de revisão dedicada.
//
// Conciliação (Sessão 77.2, generalizada na automação CVM da parte
// "futuro"): se já existe uma sugestão pendente de outra fonte na mesma
// data (`source = "manual"`, cadastrada à mão pelo dono do projeto, ou
// `source = "cvm"`, gerada pela automação CVM removida na Fase 20), o
// `generate_` não cria sugestão nova — só concilia: valor ±10% → marca a
// outra como `matched` (ainda exige o clique de Confirmar); divergiu →
// marca a outra como `divergent` e cria uma sugestão automática paralela
// pra comparar (habilitado pelo índice único que já inclui `source`).

// Geração idempotente de sugestões pendentes para um portfolio:
//   - pula pagamento que já virou lançamento (`provento` na mesma data);
//   - pula pagamento futuro (a fonte só tem passado, mas o guard é barato);
//   - pula ativo com quantidade nula/negativa na data de pagamento;
//   - pra cada data, no máximo uma sugestão por `source` (o índice único
//     garante): a mesma sugestão automática não regera.
#[derive(Deserialize)]
pub struct GenerateDividendSuggestionsRequest {
    pub portfolio_id: i32,
}

#[derive(Serialize)]
pub struct GenerateResult {
    pub generated: usize,
}

#[tauri::command]
pub async fn generate_dividend_suggestions(
    db: tauri::State<'_, DatabaseConnection>,
    request: GenerateDividendSuggestionsRequest,
) -> Result<GenerateResult, AppError> {
    portfolio::Entity::find_by_id(request.portfolio_id)
        .one(db.inner())
        .await?
        .ok_or_else(|| AppError::NotFound(format!("portfolio {}", request.portfolio_id)))?;

    let txs = transactions::Entity::find()
        .filter(transactions::Column::PortfolioId.eq(request.portfolio_id))
        .all(db.inner())
        .await?;

    // Separa os lançamentos por ativo pra reconstrução de quantidade e coleta
    // os ativos envolvidos pra casar ticker → pagamentos.
    let mut txs_by_asset: HashMap<i32, Vec<LedgerRow>> = HashMap::new();
    let mut asset_ids: HashSet<i32> = HashSet::new();
    for tx in &txs {
        if let Some(asset_id) = tx.asset_id {
            asset_ids.insert(asset_id);
            txs_by_asset
                .entry(asset_id)
                .or_insert_with(Vec::new)
                .push(LedgerRow {
                    transaction_type: tx.transaction_type.clone(),
                    quantity: tx.quantity.unwrap_or(0.0),
                    transaction_date: tx.transaction_date.clone(),
                });
        }
    }

    if asset_ids.is_empty() {
        return Ok(GenerateResult { generated: 0 });
    }

    let assets_map: HashMap<i32, assets::Model> = assets::Entity::find()
        .filter(assets::Column::Id.is_in(asset_ids.iter().copied().collect::<Vec<i32>>()))
        .all(db.inner())
        .await?
        .into_iter()
        .map(|a| (a.id, a))
        .collect();

    // Lançamentos `provento` já existentes: (portfolio, asset, data) já
    // registrados manualmente não devem reaparecer como sugestão.
    let logged: HashSet<(i32, String)> = txs
        .iter()
        .filter(|tx| tx.transaction_type == "provento")
        .filter_map(|tx| tx.asset_id.map(|a| (a, tx.transaction_date.clone())))
        .collect();

    // Sugestões existentes agrupadas por (asset, payment_date) — preciso ver
    // por data as duas fontes possíveis (manual e automática) em vez de só
    // "existe ou não", que era o guard da versão 1 (Sessão 77) e quebraria a
    // conciliação.
    let mut existing_by_date: HashMap<(i32, String), Vec<suggested_dividends::Model>> =
        HashMap::new();
    for s in suggested_dividends::Entity::find()
        .filter(suggested_dividends::Column::PortfolioId.eq(request.portfolio_id))
        .all(db.inner())
        .await?
    {
        existing_by_date
            .entry((s.asset_id, s.payment_date.clone()))
            .or_default()
            .push(s);
    }

    let today = chrono::Local::now().date_naive().format("%Y-%m-%d").to_string();

    let payments = stock_dividend_payments::Entity::find().all(db.inner()).await?;
    let payments_by_ticker: HashMap<String, Vec<stock_dividend_payments::Model>> = {
        let mut map: HashMap<String, Vec<stock_dividend_payments::Model>> = HashMap::new();
        for payment in payments {
            map.entry(payment.ticker.clone()).or_default().push(payment);
        }
        map
    };

    let mut generated = 0usize;
    for (asset_id, asset) in &assets_map {
        let Some(asset_txs) = txs_by_asset.get(asset_id) else {
            continue;
        };
        let Some(asset_payments) = payments_by_ticker.get(&asset.ticker) else {
            continue;
        };

        for payment in asset_payments {
            if payment.payment_date > today {
                continue;
            }
            if logged.contains(&(*asset_id, payment.payment_date.clone())) {
                continue;
            }

            let quantity = quantity_held_by(asset_txs, &payment.payment_date);
            if quantity <= 0.0 {
                continue;
            }

            let key = (*asset_id, payment.payment_date.clone());
            let date_rows = existing_by_date.get(&key);

            // Já existe sugestão da mesma fonte pra essa data — idempotente,
            // não cria de novo (independente do status, que o dono já decidiu).
            let same_source_exists = date_rows.is_some_and(|rows| {
                rows.iter().any(|s| s.source == payment.source)
            });
            if same_source_exists {
                continue;
            }

            // Conciliação: uma sugestão pendente de outra fonte (manual ou
            // "cvm", Fase 13.6) na mesma data — generalizado de "só manual"
            // (Sessão 77.2) pra "qualquer fonte que não seja esta mesma"
            // quando a fonte CVM passou a gerar sugestões futuras também
            // sujeitas a conciliar contra o pagamento real do Yahoo.
            if let Some(manual) = date_rows.and_then(|rows| {
                rows.iter()
                    .find(|s| s.source != payment.source && s.status == STATUS_PENDING)
            }) {
                let now = chrono::Utc::now().to_rfc3339();
                if within_tolerance(manual.amount, payment.amount, MATCH_TOLERANCE_PCT) {
                    // Bateu: marca como conciliado, ainda exige confirmar. O
                    // valor passa a ser o real da fonte, quantidade continua a
                    // que o dono do projeto lançou (total recalculado). Não
                    // cria sugestão nova — é a conciliação que o dono pediu.
                    suggested_dividends::ActiveModel {
                        id: Set(manual.id),
                        amount: Set(payment.amount),
                        total: Set(manual.quantity * payment.amount),
                        status: Set(STATUS_MATCHED.to_string()),
                        ..Default::default()
                    }
                    .update(db.inner())
                    .await?;
                } else {
                    // Divergiu: marca o esperado como `divergent` e cria uma
                    // sugestão automática paralela pra comparar os dois valores.
                    suggested_dividends::ActiveModel {
                        id: Set(manual.id),
                        status: Set(STATUS_DIVERGENT.to_string()),
                        ..Default::default()
                    }
                    .update(db.inner())
                    .await?;

                    suggested_dividends::ActiveModel {
                        portfolio_id: Set(request.portfolio_id),
                        asset_id: Set(*asset_id),
                        payment_date: Set(payment.payment_date.clone()),
                        amount: Set(payment.amount),
                        quantity: Set(quantity),
                        total: Set(quantity * payment.amount),
                        status: Set(STATUS_PENDING.to_string()),
                        com_date: Set(None),
                        source: Set(payment.source.clone()),
                        payment_type: Set(PAYMENT_TYPE_DIVIDENDO.to_string()),
                        created_at: Set(now),
                        ..Default::default()
                    }
                    .insert(db.inner())
                    .await?;
                    generated += 1;
                }
                continue;
            }

            // Só chega aqui se não havia sugestão de outra fonte pendente:
            // já havia sugestão de outra fonte decidida/paralela, ou nada.
            let already_decided = date_rows.is_some_and(|rows| {
                rows.iter().any(|s| {
                    s.status == STATUS_CONFIRMED
                        || s.status == STATUS_DISCARDED
                        || s.source != payment.source
                })
            });
            if already_decided {
                continue;
            }

            let now = chrono::Utc::now().to_rfc3339();
            suggested_dividends::ActiveModel {
                portfolio_id: Set(request.portfolio_id),
                asset_id: Set(*asset_id),
                payment_date: Set(payment.payment_date.clone()),
                amount: Set(payment.amount),
                quantity: Set(quantity),
                total: Set(quantity * payment.amount),
                status: Set(STATUS_PENDING.to_string()),
                com_date: Set(None),
                source: Set(payment.source.clone()),
                payment_type: Set(PAYMENT_TYPE_DIVIDENDO.to_string()),
                created_at: Set(now),
                ..Default::default()
            }
            .insert(db.inner())
            .await?;
            generated += 1;
        }
    }

    Ok(GenerateResult { generated })
}

// Cadastra manualmente um provento esperado (`source = "manual"`) — costuma
// ser data futura, mas também aceita passado; na hora que a fonte automática
// trouxer o pagamento, o `generate_` concilia em vez de duplicar (Sessão
// 77.2). Não toca no ledger: é só uma expectativa, status `pending`.
#[derive(Deserialize)]
pub struct CreateExpectedDividendRequest {
    pub portfolio_id: i32,
    pub asset_id: i32,
    pub payment_date: String,
    pub amount: f64,
    pub quantity: f64,
    pub com_date: Option<String>,
    pub payment_type: Option<String>,
}

#[tauri::command]
pub async fn create_expected_dividend(
    db: tauri::State<'_, DatabaseConnection>,
    request: CreateExpectedDividendRequest,
) -> Result<DividendSuggestionView, AppError> {
    if request.amount <= 0.0 || request.quantity <= 0.0 {
        return Err(AppError::InvalidGuard(
            "amount e quantity precisam ser maiores que zero".to_string(),
        ));
    }
    validate_payment_type(&request.payment_type)?;

    portfolio::Entity::find_by_id(request.portfolio_id)
        .one(db.inner())
        .await?
        .ok_or_else(|| AppError::NotFound(format!("portfolio {}", request.portfolio_id)))?;

    let asset = assets::Entity::find_by_id(request.asset_id)
        .one(db.inner())
        .await?
        .ok_or_else(|| AppError::NotFound(format!("asset {}", request.asset_id)))?;

    // Somente um esperado manual por (asset, payment_date) — o índice único
    // agora inclui `source`, então o guard explícito mantém a mensagem clara
    // em vez do erro cru de constraint.
    let duplicate = suggested_dividends::Entity::find()
        .filter(suggested_dividends::Column::PortfolioId.eq(request.portfolio_id))
        .filter(suggested_dividends::Column::AssetId.eq(request.asset_id))
        .filter(suggested_dividends::Column::PaymentDate.eq(&request.payment_date))
        .filter(suggested_dividends::Column::Source.eq(SOURCE_MANUAL))
        .one(db.inner())
        .await?;
    if duplicate.is_some() {
        return Err(AppError::InvalidGuard(
            "já existe um provento esperado manual pra essa data".to_string(),
        ));
    }

    let total = request.quantity * request.amount;
    let payment_type = request.payment_type.unwrap_or_else(|| PAYMENT_TYPE_DIVIDENDO.to_string());
    let model = suggested_dividends::ActiveModel {
        portfolio_id: Set(request.portfolio_id),
        asset_id: Set(request.asset_id),
        payment_date: Set(request.payment_date),
        amount: Set(request.amount),
        quantity: Set(request.quantity),
        total: Set(total),
        status: Set(STATUS_PENDING.to_string()),
        com_date: Set(request.com_date),
        source: Set(SOURCE_MANUAL.to_string()),
        payment_type: Set(payment_type),
        created_at: Set(chrono::Utc::now().to_rfc3339()),
        ..Default::default()
    }
    .insert(db.inner())
    .await?;

    Ok(DividendSuggestionView {
        id: model.id,
        asset_id: model.asset_id,
        ticker: asset.ticker,
        name: asset.name,
        asset_class: asset.asset_class,
        currency: asset.currency,
        payment_date: model.payment_date,
        amount: model.amount,
        quantity: model.quantity,
        total: model.total,
        status: model.status,
        com_date: model.com_date,
        source: model.source,
        payment_type: model.payment_type,
        created_at: model.created_at,
    })
}

#[derive(Serialize)]
pub struct DividendSuggestionView {
    pub id: i32,
    pub asset_id: i32,
    pub ticker: String,
    pub name: String,
    pub asset_class: String,
    pub currency: String,
    pub payment_date: String,
    pub amount: f64,
    pub quantity: f64,
    pub total: f64,
    pub status: String,
    pub com_date: Option<String>,
    pub source: String,
    pub payment_type: String,
    pub created_at: String,
}

#[tauri::command]
pub async fn list_dividend_suggestions(
    db: tauri::State<'_, DatabaseConnection>,
    portfolio_id: i32,
) -> Result<Vec<DividendSuggestionView>, AppError> {
    let suggestions = suggested_dividends::Entity::find()
        .filter(suggested_dividends::Column::PortfolioId.eq(portfolio_id))
        .order_by_asc(suggested_dividends::Column::PaymentDate)
        .all(db.inner())
        .await?;

    if suggestions.is_empty() {
        return Ok(vec![]);
    }

    let asset_ids: Vec<i32> = suggestions.iter().map(|s| s.asset_id).collect();
    let assets_map: HashMap<i32, assets::Model> = assets::Entity::find()
        .filter(assets::Column::Id.is_in(asset_ids))
        .all(db.inner())
        .await?
        .into_iter()
        .map(|a| (a.id, a))
        .collect();

    let views = suggestions
        .into_iter()
        .map(|s| {
            let asset = assets_map.get(&s.asset_id);
            DividendSuggestionView {
                id: s.id,
                asset_id: s.asset_id,
                ticker: asset.map(|a| a.ticker.clone()).unwrap_or_default(),
                name: asset.map(|a| a.name.clone()).unwrap_or_default(),
                asset_class: asset.map(|a| a.asset_class.clone()).unwrap_or_default(),
                currency: asset.map(|a| a.currency.clone()).unwrap_or_default(),
                payment_date: s.payment_date,
                amount: s.amount,
                quantity: s.quantity,
                total: s.total,
                status: s.status,
                com_date: s.com_date,
                source: s.source,
                payment_type: s.payment_type,
                created_at: s.created_at,
            }
        })
        .collect();

    Ok(views)
}

// Confirma uma sugestão: grava o `provento` de verdade em `transactions`
// (quantidade editável, Data Com manual — decisão da Sessão 76) e marca a
// sugestão como confirmada. As duas escritas são atômicas (mesmo padrão de
// `create_liability`), pra sugestão nunca ficar confirmada sem lançamento.
// Aceita também `matched` (esperado manual que a fonte confirmou) e
// `divergent` (esperado manual que divergiu — o dono pode confirmar qualquer
// uma das duas linhas, a manual com o valor dele ou a automática com o real).
#[derive(Deserialize)]
pub struct ConfirmDividendSuggestionRequest {
    pub suggestion_id: i32,
    pub quantity: f64,
    pub com_date: Option<String>,
    pub payment_type: Option<String>,
}

#[tauri::command]
pub async fn confirm_dividend_suggestion(
    db: tauri::State<'_, DatabaseConnection>,
    request: ConfirmDividendSuggestionRequest,
) -> Result<DividendSuggestionView, AppError> {
    if request.quantity <= 0.0 {
        return Err(AppError::InvalidGuard(
            "quantidade confirmada precisa ser maior que zero".to_string(),
        ));
    }
    validate_payment_type(&request.payment_type)?;

    let suggestion = suggested_dividends::Entity::find_by_id(request.suggestion_id)
        .one(db.inner())
        .await?
        .ok_or_else(|| AppError::NotFound(format!("suggestion {}", request.suggestion_id)))?;

    if suggestion.status != STATUS_PENDING && suggestion.status != STATUS_MATCHED
        && suggestion.status != STATUS_DIVERGENT
    {
        return Err(AppError::InvalidGuard(format!(
            "sugestão {} não está em estado confirmável (status '{}')",
            suggestion.id, suggestion.status
        )));
    }

    let txn = db.inner().begin().await?;

    let total = request.quantity * suggestion.amount;
    let payment_type = request.payment_type.clone().unwrap_or_else(|| suggestion.payment_type.clone());
    let notes = Some(format!(
        "Auto dividend via suggestion {}/{}",
        suggestion.payment_date, suggestion.source
    ));

    let _created = transactions::ActiveModel {
        portfolio_id: Set(suggestion.portfolio_id),
        asset_id: Set(Some(suggestion.asset_id)),
        custodia_id: Set(None),
        transfer_to_custodia_id: Set(None),
        transaction_type: Set("provento".to_string()),
        payment_type: Set(Some(payment_type.clone())),
        quantity: Set(Some(request.quantity)),
        unit_price: Set(Some(suggestion.amount)),
        total_value: Set(total),
        fee: Set(None),
        transaction_date: Set(suggestion.payment_date.clone()),
        notes: Set(notes),
        fi_emissor: Set(None),
        fi_indexador: Set(None),
        fi_taxa_percentual: Set(None),
        fi_data_vencimento: Set(None),
        fi_liquidez: Set(None),
        created_at: Set(chrono::Utc::now().to_rfc3339()),
        ..Default::default()
    }
    .insert(&txn)
    .await?;

    let updated = suggested_dividends::ActiveModel {
        id: Set(suggestion.id),
        status: Set(STATUS_CONFIRMED.to_string()),
        quantity: Set(request.quantity),
        total: Set(total),
        com_date: Set(request.com_date),
        payment_type: Set(payment_type),
        ..Default::default()
    }
    .update(&txn)
    .await?;

    let asset = assets::Entity::find_by_id(updated.asset_id)
        .one(&txn)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("asset {}", updated.asset_id)))?;

    txn.commit().await?;

    Ok(DividendSuggestionView {
        id: updated.id,
        asset_id: updated.asset_id,
        ticker: asset.ticker,
        name: asset.name,
        asset_class: asset.asset_class,
        currency: asset.currency,
        payment_date: updated.payment_date,
        amount: updated.amount,
        quantity: updated.quantity,
        total: updated.total,
        status: updated.status,
        com_date: updated.com_date,
        source: updated.source,
        payment_type: updated.payment_type,
        created_at: updated.created_at,
    })
}

// Descarta uma sugestão — não escreve nada no ledger, só muda o status pra
// `discarded` (com o guard da geração, descartada não regera). Aceita os
// mesmos estados confirmáveis (`pending`/`matched`/`divergent`).
#[derive(Deserialize)]
pub struct DiscardDividendSuggestionRequest {
    pub suggestion_id: i32,
}

#[tauri::command]
pub async fn discard_dividend_suggestion(
    db: tauri::State<'_, DatabaseConnection>,
    request: DiscardDividendSuggestionRequest,
) -> Result<(), AppError> {
    let suggestion = suggested_dividends::Entity::find_by_id(request.suggestion_id)
        .one(db.inner())
        .await?
        .ok_or_else(|| AppError::NotFound(format!("suggestion {}", request.suggestion_id)))?;

    if suggestion.status != STATUS_PENDING && suggestion.status != STATUS_MATCHED
        && suggestion.status != STATUS_DIVERGENT
    {
        return Err(AppError::InvalidGuard(format!(
            "sugestão {} não está descartável (status '{}')",
            suggestion.id, suggestion.status
        )));
    }

    suggested_dividends::ActiveModel {
        id: Set(suggestion.id),
        status: Set(STATUS_DISCARDED.to_string()),
        ..Default::default()
    }
    .update(db.inner())
    .await?;

    Ok(())
}