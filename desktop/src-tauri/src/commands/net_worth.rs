// Fase 12 — Patrimônio Líquido/Alavancagem cruzando Portfolio + Finanças
// Gerais. Sem migration nova, tudo derivado on-the-fly (mesmo padrão de
// `BankAccountView.balance`/`PositionView.market_value`/
// `LiabilityView.saldo_devedor_atual`).

use std::collections::HashSet;

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::Serialize;

use crate::commands::bank_account::{compute_total_cash, mirrored_cash_deduction};
use crate::commands::currency::{base_currency, positions_in_base, rates_for, DEFAULT_BASE_CURRENCY};
use crate::commands::liability::list_liabilities_view;
use crate::commands::loan::list_loans_view;
use crate::commands::option_position::list_option_positions_view;
use crate::domain::net_worth::{compute_leverage, sum_options_in_net_worth, sum_receivable_in_net_worth};
use crate::entity::portfolio;
use crate::error::AppError;

/// Classe dos papéis de renda fixa que a importação da Movimentação cria como posição.
const FIXED_INCOME_CLASS: &str = "renda_fixa";

#[derive(Serialize)]
pub struct NetWorthSummary {
    /// Fase 19 — moeda de todos os totais abaixo.
    pub base_currency: String,
    /// Posições, contas, dívidas, empréstimos e opções deixados de fora por
    /// falta de cotação do par pra moeda base.
    pub items_missing_fx: usize,
    pub total_market_value: f64,
    pub total_cash: f64,
    /// Quanto do saldo das contas marcadas "já na carteira" saiu do caixa por já estar nas posições de renda fixa.
    pub cash_in_portfolio: f64,
    pub total_debt: f64,
    /// Saldo a receber dos empréstimos concedidos marcados pra entrar no
    /// patrimônio (Fase 29). Os que ficaram de fora não aparecem aqui.
    pub total_loans_receivable: f64,
    /// Posições em opções abertas com a flag ligada (Fase 18): comprada soma,
    /// lançada subtrai. Fora de `total_debt`, então não muda a alavancagem.
    pub total_options_value: f64,
    /// Quantas dessas posições valeram o prêmio, por falta de preço guardado.
    pub options_at_cost_count: usize,
    pub net_worth: f64,
    pub leverage_ratio: Option<f64>,
}

#[tauri::command]
pub async fn get_net_worth_summary(
    db: tauri::State<'_, DatabaseConnection>,
    workspace_id: i32,
) -> Result<NetWorthSummary, AppError> {
    compute_net_worth(db.inner(), workspace_id).await
}

pub async fn compute_net_worth(
    db: &DatabaseConnection,
    workspace_id: i32,
) -> Result<NetWorthSummary, AppError> {
    let portfolios = portfolio::Entity::find()
        .filter(portfolio::Column::WorkspaceId.eq(workspace_id))
        .all(db)
        .await?;

    // Fase 19 — tudo entra na moeda base do workspace. Item sem cotação do par
    // fica fora da soma e é contado em `items_missing_fx`.
    let base_currency = base_currency(db, workspace_id).await?;
    let mut items_missing_fx = 0;

    let mut total_market_value = 0.0;
    // Renda fixa importada da Movimentação (a custo): é o que as contas marcadas como "já na carteira"
    // têm dentro, então esse valor sai do caixa delas pra não contar duas vezes.
    let mut portfolio_fixed_income_value = 0.0;
    for p in &portfolios {
        let (positions, _) = positions_in_base(db, p.id).await?;
        for pos in &positions {
            if pos.fx_missing {
                if pos.quantity > 0.0 {
                    items_missing_fx += 1;
                }
            } else if let Some(value) = pos.market_value {
                total_market_value += value;
                if pos.asset_class == FIXED_INCOME_CLASS {
                    portfolio_fixed_income_value += value;
                }
            }
        }
    }

    let cash = compute_total_cash(db, workspace_id).await?;
    let cash_in_portfolio = mirrored_cash_deduction(&cash.mirrored_balances_in_base, portfolio_fixed_income_value);
    let total_cash = cash.total_in_base - cash_in_portfolio;
    items_missing_fx += cash.accounts_missing_fx;

    let liabilities = list_liabilities_view(db, workspace_id).await?;
    let mut total_debt = 0.0;
    for l in &liabilities {
        match l.saldo_devedor_in_base {
            Some(value) => total_debt += value,
            None => items_missing_fx += 1,
        }
    }

    let loans = list_loans_view(db, workspace_id).await?;
    items_missing_fx += loans
        .iter()
        .filter(|l| l.include_in_net_worth && l.saldo_a_receber_in_base.is_none())
        .count();
    let total_loans_receivable = sum_receivable_in_net_worth(
        &loans
            .iter()
            .filter_map(|l| l.saldo_a_receber_in_base.map(|v| (l.include_in_net_worth, v)))
            .collect::<Vec<_>>(),
    );

    // Opções são da B3, em BRL.
    let option_rates = rates_for(db, workspace_id, &HashSet::from([DEFAULT_BASE_CURRENCY.to_string()])).await?;
    let mut option_values = Vec::new();
    let mut options_at_cost_count = 0;
    for p in &portfolios {
        for position in list_option_positions_view(db, p.id).await? {
            let Some(value) = position.net_worth_value else {
                continue; // fechada
            };
            let Some(value) = option_rates.convert(value, DEFAULT_BASE_CURRENCY) else {
                if position.include_in_net_worth {
                    items_missing_fx += 1;
                }
                continue;
            };
            option_values.push((position.include_in_net_worth, value));
            if position.include_in_net_worth && position.value_at_cost {
                options_at_cost_count += 1;
            }
        }
    }
    let total_options_value = sum_options_in_net_worth(&option_values);

    let net_worth =
        total_market_value + total_cash + total_loans_receivable + total_options_value - total_debt;
    let leverage_ratio = compute_leverage(total_debt, net_worth);

    Ok(NetWorthSummary {
        base_currency,
        items_missing_fx,
        total_market_value,
        total_cash,
        cash_in_portfolio,
        total_debt,
        total_loans_receivable,
        total_options_value,
        options_at_cost_count,
        net_worth,
        leverage_ratio,
    })
}
