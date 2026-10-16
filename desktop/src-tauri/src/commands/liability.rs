use chrono::{Datelike, NaiveDate};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};

use crate::commands::profitability::compute_profitability;
use crate::domain::liability::{
    annualize_monthly_rate, compute_installments, compute_outstanding_balance, AmortizationSystem,
};
use crate::domain::twr::trailing_twelve_month_pct;
use crate::entity::{general_transaction, liability, liability_installment, portfolio};
use crate::error::AppError;

const PARCELA_DIVIDA: &str = "parcela_divida";

fn days_in_month(year: i32, month: u32) -> u32 {
    let next = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    }
    .expect("valid date");
    let start = NaiveDate::from_ymd_opt(year, month, 1).expect("valid date");
    (next - start).num_days() as u32
}

/// Soma `months` a `date`, preservando o dia quando possível (clampado ao
/// último dia do mês de destino — ex. 31/01 + 1 mês = 28/02 ou 29/02).
/// Mesmo espírito de `days_in_month`/`month_end_date` já usados em
/// `commands/profitability.rs`, sem depender deles pra manter os dois
/// módulos independentes.
fn add_months(date: NaiveDate, months: i32) -> NaiveDate {
    let total_months = date.month0() as i32 + months;
    let year = date.year() + total_months.div_euclid(12);
    let month = (total_months.rem_euclid(12)) as u32 + 1;
    let day = date.day().min(days_in_month(year, month));
    NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
}

#[derive(Deserialize)]
pub struct CreateLiabilityRequest {
    pub workspace_id: i32,
    pub bank_account_id: i32,
    pub linked_asset_id: Option<i32>,
    pub nome: String,
    pub titular: String,
    pub principal: f64,
    /// Percentual mensal como digitado (ex. `1.5` pra 1,5% a.m.) — convertido
    /// pra decimal só na hora de chamar `compute_installments` (mesmo
    /// espírito de `fi_taxa_percentual`, que também guarda o número cru).
    pub taxa_percentual: f64,
    pub indexador: String,
    pub prazo_meses: i32,
    pub sistema_amortizacao: String,
    pub data_inicio: String,
}

#[derive(Serialize)]
pub struct LiabilityView {
    pub id: i32,
    pub workspace_id: i32,
    pub bank_account_id: i32,
    pub linked_asset_id: Option<i32>,
    pub nome: String,
    pub titular: String,
    pub principal: f64,
    pub taxa_percentual: f64,
    pub indexador: String,
    pub prazo_meses: i32,
    pub sistema_amortizacao: String,
    pub data_inicio: String,
    pub created_at: String,
    pub saldo_devedor_atual: f64,
    // Fase 12 — retorno alavancado. Os 3 campos abaixo já vêm em pontos
    // percentuais (ex. `42.58`), igual `MonthlyReturn::r_cumulative_pct` —
    // diverge da convenção em fração de `unrealized_pl_pct` (Fase 13.1),
    // mas segue a família TWR da qual este cálculo deriva. Aparecem juntos
    // (todos `Some` ou todos `None`) — sem `linked_asset_id`, "custo da
    // dívida" sozinho não tem com o que ser comparado, então não aparece.
    pub annualized_debt_cost_pct: Option<f64>,
    pub linked_asset_return_12m_pct: Option<f64>,
    pub spread_12m_pct: Option<f64>,
}

#[tauri::command]
pub async fn create_liability(
    db: tauri::State<'_, DatabaseConnection>,
    request: CreateLiabilityRequest,
) -> Result<LiabilityView, AppError> {
    let sistema = AmortizationSystem::parse(&request.sistema_amortizacao)?;
    let data_inicio = NaiveDate::parse_from_str(&request.data_inicio, "%Y-%m-%d")
        .map_err(|e| AppError::InvalidInput(format!("data_inicio inválida: {e}")))?;

    let installments = compute_installments(
        request.principal,
        request.taxa_percentual / 100.0,
        request.prazo_meses,
        sistema,
    )?;

    let txn = db.inner().begin().await?;

    let created_at = chrono::Utc::now().to_rfc3339();
    let liability_model = liability::ActiveModel {
        workspace_id: Set(request.workspace_id),
        bank_account_id: Set(request.bank_account_id),
        linked_asset_id: Set(request.linked_asset_id),
        nome: Set(request.nome.clone()),
        titular: Set(request.titular.clone()),
        principal: Set(request.principal),
        taxa_percentual: Set(request.taxa_percentual),
        indexador: Set(request.indexador.clone()),
        prazo_meses: Set(request.prazo_meses),
        sistema_amortizacao: Set(request.sistema_amortizacao.clone()),
        data_inicio: Set(request.data_inicio.clone()),
        created_at: Set(created_at.clone()),
        ..Default::default()
    }
    .insert(&txn)
    .await?;

    for installment in &installments {
        let due_date = add_months(data_inicio, installment.numero_parcela);
        let tx = general_transaction::ActiveModel {
            workspace_id: Set(request.workspace_id),
            bank_account_id: Set(request.bank_account_id),
            transaction_type: Set(PARCELA_DIVIDA.to_string()),
            category_id: Set(None),
            valor: Set(installment.valor_parcela),
            transaction_date: Set(due_date.format("%Y-%m-%d").to_string()),
            notes: Set(Some(format!(
                "{} — parcela {}/{}",
                request.nome, installment.numero_parcela, request.prazo_meses
            ))),
            created_at: Set(chrono::Utc::now().to_rfc3339()),
            ..Default::default()
        }
        .insert(&txn)
        .await?;

        liability_installment::ActiveModel {
            liability_id: Set(liability_model.id),
            general_transaction_id: Set(tx.id),
            numero_parcela: Set(installment.numero_parcela),
            valor_parcela: Set(installment.valor_parcela),
            valor_juros: Set(installment.valor_juros),
            valor_amortizacao: Set(installment.valor_amortizacao),
            saldo_devedor_apos: Set(installment.saldo_devedor_apos),
            data_vencimento: Set(due_date.format("%Y-%m-%d").to_string()),
            created_at: Set(chrono::Utc::now().to_rfc3339()),
            ..Default::default()
        }
        .insert(&txn)
        .await?;
    }

    txn.commit().await?;

    // `linked_asset_return_12m_pct`/`spread_12m_pct` ficam `None` na resposta
    // imediata da criação (exigiriam escanear preço histórico aqui, sem
    // necessidade — o frontend invalida `["liabilities", workspaceId]` logo
    // em seguida, então esse `None` transitório nunca aparece na tela).
    let annualized_debt_cost_pct = liability_model
        .linked_asset_id
        .map(|_| annualize_monthly_rate(liability_model.taxa_percentual / 100.0) * 100.0);

    Ok(LiabilityView {
        id: liability_model.id,
        workspace_id: liability_model.workspace_id,
        bank_account_id: liability_model.bank_account_id,
        linked_asset_id: liability_model.linked_asset_id,
        nome: liability_model.nome,
        titular: liability_model.titular,
        principal: liability_model.principal,
        taxa_percentual: liability_model.taxa_percentual,
        indexador: liability_model.indexador,
        prazo_meses: liability_model.prazo_meses,
        sistema_amortizacao: liability_model.sistema_amortizacao,
        data_inicio: liability_model.data_inicio,
        created_at: liability_model.created_at,
        saldo_devedor_atual: request.principal,
        annualized_debt_cost_pct,
        linked_asset_return_12m_pct: None,
        spread_12m_pct: None,
    })
}

#[tauri::command]
pub async fn list_liabilities(
    db: tauri::State<'_, DatabaseConnection>,
    workspace_id: i32,
) -> Result<Vec<LiabilityView>, AppError> {
    list_liabilities_view(db.inner(), workspace_id).await
}

// Extraída pra ser reaproveitada por `commands::net_worth::get_net_worth_summary`
// (Fase 12), mesmo precedente de `commands::transaction::compute_positions`
// (Fase 13.1) — evita duplicar a soma de saldo devedor.
pub async fn list_liabilities_view(
    db: &DatabaseConnection,
    workspace_id: i32,
) -> Result<Vec<LiabilityView>, AppError> {
    let liabilities = liability::Entity::find()
        .filter(liability::Column::WorkspaceId.eq(workspace_id))
        .all(db)
        .await?;

    if liabilities.is_empty() {
        return Ok(vec![]);
    }

    let liability_ids: Vec<i32> = liabilities.iter().map(|l| l.id).collect();
    let installments = liability_installment::Entity::find()
        .filter(liability_installment::Column::LiabilityId.is_in(liability_ids))
        .all(db)
        .await?;

    let hoje = chrono::Utc::now().format("%Y-%m-%d").to_string();

    // Resolvido uma vez fora do loop (não por dívida): `Liability.workspace_id`
    // só carrega contexto de workspace, mas a posição/retorno de um ativo só
    // existe dentro de `transactions.portfolio_id` — um workspace pode ter
    // mais de um portfolio, então o retorno do ativo vinculado precisa somar
    // as transações dele em todos os portfolios do workspace da dívida.
    let portfolio_ids: Vec<i32> = portfolio::Entity::find()
        .filter(portfolio::Column::WorkspaceId.eq(workspace_id))
        .all(db)
        .await?
        .into_iter()
        .map(|p| p.id)
        .collect();

    let mut views = Vec::with_capacity(liabilities.len());
    for l in liabilities {
        let pairs: Vec<(&str, f64)> = installments
            .iter()
            .filter(|i| i.liability_id == l.id)
            .map(|i| (i.data_vencimento.as_str(), i.valor_amortizacao))
            .collect();
        let saldo_devedor_atual = compute_outstanding_balance(l.principal, &pairs, &hoje);

        let (annualized_debt_cost_pct, linked_asset_return_12m_pct, spread_12m_pct) =
            match l.linked_asset_id {
                Some(asset_id) if !portfolio_ids.is_empty() => {
                    let annualized = annualize_monthly_rate(l.taxa_percentual / 100.0) * 100.0;
                    // Um gap de preço nesse ativo específico não pode derrubar
                    // a lista inteira de dívidas — engolido via `.ok()`.
                    let asset_return = compute_profitability(db, &portfolio_ids, Some(asset_id))
                        .await
                        .ok()
                        .and_then(|monthly| {
                            let cumulative: Vec<f64> =
                                monthly.iter().map(|m| m.r_cumulative_pct).collect();
                            trailing_twelve_month_pct(&cumulative)
                        });
                    (Some(annualized), asset_return, asset_return.map(|r| r - annualized))
                }
                _ => (None, None, None),
            };

        views.push(LiabilityView {
            id: l.id,
            workspace_id: l.workspace_id,
            bank_account_id: l.bank_account_id,
            linked_asset_id: l.linked_asset_id,
            nome: l.nome,
            titular: l.titular,
            principal: l.principal,
            taxa_percentual: l.taxa_percentual,
            indexador: l.indexador,
            prazo_meses: l.prazo_meses,
            sistema_amortizacao: l.sistema_amortizacao,
            data_inicio: l.data_inicio,
            created_at: l.created_at,
            saldo_devedor_atual,
            annualized_debt_cost_pct,
            linked_asset_return_12m_pct,
            spread_12m_pct,
        });
    }

    Ok(views)
}

#[tauri::command]
pub async fn delete_liability(
    db: tauri::State<'_, DatabaseConnection>,
    liability_id: i32,
) -> Result<(), AppError> {
    let txn = db.inner().begin().await?;

    let installments = liability_installment::Entity::find()
        .filter(liability_installment::Column::LiabilityId.eq(liability_id))
        .all(&txn)
        .await?;

    // Desfaz o fluxo de caixa projetado — sem isso, as parcelas geradas
    // ficariam "fantasma" em `general_transactions` (contando no saldo da
    // conta) mesmo com a dívida excluída. A FK aponta da parcela pro
    // lançamento (excluir a dívida não apaga o lançamento), então isso é
    // feito explicitamente. (O sqlx liga as FKs do SQLite por padrão, o que
    // vale lembrar pra qualquer DELETE em massa: o cascade age na hora.)
    let general_transaction_ids: Vec<i32> =
        installments.iter().map(|i| i.general_transaction_id).collect();
    if !general_transaction_ids.is_empty() {
        general_transaction::Entity::delete_many()
            .filter(general_transaction::Column::Id.is_in(general_transaction_ids))
            .exec(&txn)
            .await?;
    }

    liability_installment::Entity::delete_many()
        .filter(liability_installment::Column::LiabilityId.eq(liability_id))
        .exec(&txn)
        .await?;

    liability::Entity::delete_by_id(liability_id)
        .exec(&txn)
        .await?;

    txn.commit().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_months_preserves_day_within_month() {
        let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
        assert_eq!(add_months(date, 1), NaiveDate::from_ymd_opt(2026, 2, 15).unwrap());
        assert_eq!(add_months(date, 12), NaiveDate::from_ymd_opt(2027, 1, 15).unwrap());
    }

    #[test]
    fn add_months_clamps_to_end_of_shorter_month() {
        let date = NaiveDate::from_ymd_opt(2026, 1, 31).unwrap();
        assert_eq!(add_months(date, 1), NaiveDate::from_ymd_opt(2026, 2, 28).unwrap());
    }

    #[test]
    fn add_months_rolls_over_year_boundary() {
        let date = NaiveDate::from_ymd_opt(2026, 11, 30).unwrap();
        assert_eq!(add_months(date, 3), NaiveDate::from_ymd_opt(2027, 2, 28).unwrap());
    }
}
