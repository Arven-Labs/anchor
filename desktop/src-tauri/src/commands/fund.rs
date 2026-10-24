// Fase 24 — fundos de investimento (classe `fundo`) e a estimativa do come-cotas. O usuário diz o tipo de cada fundo
// (longo prazo, curto prazo ou ações); o valor da cota vem do informe diário da CVM (com o CNPJ do fundo em `assets.cnpj`
// e, se o CNPJ só tem subclasses, a escolhida em `fund_details.cvm_subclass`) ou é lançado à mão por data, e o lançamento
// manual vence a CVM. A estimativa (`domain::tax::come_cotas`) usa a posição do Workspace e o último valor de cota. O
// desconto real do come-cotas é a transação `come_cotas`.

use std::collections::HashMap;

use chrono::{Datelike, Duration, NaiveDate, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};

use crate::commands::tax::{load_rule_set, workspace_portfolio_ids};
use crate::commands::transaction::{cost_events, AMORTIZATION, COME_COTAS};
use crate::domain::cost_basis;
use crate::domain::tax::come_cotas::{self, ComeCotasEstimate, FundHolding, FundTerm, QuotaValue};
use crate::domain::tax::fund_compensation::{self, CompFund, ComeCotasMark, CreditNote, LossCredit};
use crate::domain::tax::fund_redemption::{self, EstoqueEstimate, FundEvent, FundEventKind, RedemptionEstimate};
use crate::domain::tax::rules;
use crate::entity::{assets, fund_detail, fund_quota_value, transactions};
use crate::error::AppError;
use crate::finance_api::fund_quota_store::{self, CvmStatus, FundTarget, Session, SESSION, SOURCE_CVM};
use crate::finance_core::cvm_fii::normalize_cnpj;
use crate::finance_core::cvm_inf_diario::{CvmDailyZips, ZipFetcher};

const FUND_CLASS: &str = "fundo";
const BUY: &str = "compra";
const SELL: &str = "venda";

/// Um desconto de come-cotas já registrado (transação `come_cotas`).
#[derive(Serialize)]
pub struct DeductionView {
    pub tx_id: i32,
    pub date: String,
    pub quotas: f64,
    pub quota_value: f64,
    /// O imposto em reais (cotas × valor da cota).
    pub tax: f64,
    /// O rendimento, em reais, que o crédito de prejuízo cobriu nesse desconto; `None` = não informado.
    pub loss_credit_used: Option<f64>,
}

/// Um fundo com a posição do Workspace e os valores de cota lançados (do mais antigo ao mais novo).
#[derive(Serialize)]
pub struct FundView {
    pub asset_id: i32,
    pub ticker: String,
    pub name: String,
    /// `longo_prazo`, `curto_prazo` ou `acoes`; `None` até o usuário escolher.
    pub term: Option<String>,
    pub quantity: f64,
    pub average_cost: Option<f64>,
    /// CNPJ do fundo (`assets.cnpj`), a chave do informe diário da CVM.
    pub cnpj: Option<String>,
    /// Subclasse do informe da CVM; `None` = a linha da classe.
    pub cvm_subclass: Option<String>,
    pub cvm_status: CvmStatus,
    /// Administradora ou intermediário informado; agrupa os fundos que compensam prejuízo entre si.
    pub administrator: Option<String>,
    /// `aberto` (padrão), `fechado` ou `exclusivo`: o IR do art. 17 é o mesmo, mas só os dois últimos têm estoque de 2023.
    pub structure: String,
    /// Como o estoque de 2023 foi tributado: `geral` (15%), `opcao_8` (8%) ou nada.
    pub estoque_option: Option<String>,
    pub estoque: EstoqueStatus,
    /// Data da cota mais recente lida da CVM.
    pub last_cvm_date: Option<String>,
    pub quotas: Vec<fund_quota_value::Model>,
    /// Descontos de come-cotas registrados, do mais antigo ao mais novo.
    pub deductions: Vec<DeductionView>,
    /// IR e IOF estimados de cada resgate (venda), do mais antigo ao mais novo; vazio até o fundo ter tipo.
    pub redemptions: Vec<RedemptionEstimate>,
    /// Descontos de come-cotas registrados que o grupo de prejuízo já tinha saldo para abater: a administradora pode ter
    /// usado o crédito ali, e o saldo mostrado pode estar acima do real. Só informação.
    pub credit_notes: Vec<CreditNote>,
}

#[derive(Serialize)]
pub struct FundsOverview {
    pub funds: Vec<FundView>,
    /// Estimativa do próximo come-cotas, um item por fundo (com o motivo quando não há imposto a estimar).
    pub estimates: Vec<ComeCotasEstimate>,
    /// Prejuízo a compensar que sobrou em cada grupo (mesma administradora e mesmo tipo), depois dos resgates.
    pub loss_credits: Vec<LossCredit>,
}

#[derive(Deserialize)]
pub struct UpsertFundQuotaRequest {
    pub asset_id: i32,
    /// `YYYY-MM-DD`.
    pub quota_date: String,
    /// Valor de uma cota, em BRL.
    pub value: f64,
}

/// Os fatos do fundo que importam para o IR no resgate: aplicações (custo = valor + taxa), come-cotas, resgates e
/// amortizações, mais o estoque de 2023 quando o fundo o tem (`estoque`: cota de 31/12/2023 e alíquota). Data inválida
/// fica de fora (não derruba a tela).
fn fund_events(txs: &[&transactions::Model], estoque: Option<(f64, f64)>) -> Vec<FundEvent> {
    let mut events: Vec<FundEvent> = txs
        .iter()
        .filter_map(|t| {
            let date = NaiveDate::parse_from_str(&t.transaction_date, "%Y-%m-%d").ok()?;
            let quantity = t.quantity.unwrap_or(0.0);
            let kind = match t.transaction_type.as_str() {
                BUY => FundEventKind::Buy { quantity, cost: t.total_value + t.fee.unwrap_or(0.0) },
                COME_COTAS => FundEventKind::ComeCotas {
                    quantity,
                    quota_value: t.unit_price.unwrap_or(0.0),
                    loss_credit_used: t.loss_credit_used,
                },
                SELL => FundEventKind::Redeem { tx_id: t.id, quantity, quota_value: t.unit_price.unwrap_or(0.0) },
                AMORTIZATION => FundEventKind::Amortize {
                    tx_id: t.id,
                    amount: t.total_value,
                    quota_value: t.unit_price.unwrap_or(0.0),
                },
                _ => return None,
            };
            Some(FundEvent { date, id: t.id, kind })
        })
        .collect();
    if let Some((quota_value, rate)) = estoque {
        events.push(FundEvent { date: estoque_date(), id: 0, kind: FundEventKind::Estoque { quota_value, rate } });
    }
    events
}

/// A data de referência do estoque (Lei 14.754, arts. 27 e 28).
fn estoque_date() -> NaiveDate {
    NaiveDate::from_ymd_opt(2023, 12, 31).expect("data válida")
}

/// O fundo tem estoque de 2023 a estimar (fechado ou exclusivo, de longo ou curto prazo, com a opção informada) e então
/// precisa da cota de dez/2023, que fica fora da janela de 2 anos da busca.
fn wants_estoque_quota(detail: &fund_detail::Model) -> bool {
    detail.structure != "aberto"
        && detail.estoque_option.is_some()
        && matches!(FundTerm::parse(&detail.term), Some(FundTerm::LongTerm | FundTerm::ShortTerm))
}

/// A cota usada como valor patrimonial de 31/12/2023: a última lançada até a data, no máximo 10 dias antes (o último dia
/// útil de 2023 foi 29/12).
const ESTOQUE_QUOTA_WINDOW_DAYS: i64 = 10;

fn estoque_quota(quotas: &[fund_quota_value::Model]) -> Option<(NaiveDate, f64)> {
    let reference = estoque_date();
    quotas
        .iter()
        .filter_map(|q| NaiveDate::parse_from_str(&q.quota_date, "%Y-%m-%d").ok().map(|d| (d, q.value)))
        .filter(|(d, _)| *d <= reference && (reference - *d).num_days() <= ESTOQUE_QUOTA_WINDOW_DAYS)
        .max_by_key(|(d, _)| *d)
}

pub const STRUCTURES: [&str; 3] = ["aberto", "fechado", "exclusivo"];
pub const ESTOQUE_OPTIONS: [&str; 2] = ["geral", "opcao_8"];

/// Como está o estoque de 2023 de um fundo.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EstoqueStatus {
    /// Só fundo fechado ou exclusivo de longo ou curto prazo tem estoque.
    NotApplicable,
    /// O usuário não informou como o estoque foi tributado (ou o contesta): nada é estimado.
    NotTaxed,
    /// Falta a cota de 31/12/2023 (ou de até 10 dias antes).
    NeedsQuota,
    Estimated { estimate: EstoqueEstimate },
}

async fn fund_asset(db: &DatabaseConnection, asset_id: i32) -> Result<assets::Model, AppError> {
    let asset = assets::Entity::find_by_id(asset_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("asset {asset_id}")))?;
    if asset.asset_class != FUND_CLASS {
        return Err(AppError::InvalidInput(format!("{} is not a fund", asset.ticker)));
    }
    Ok(asset)
}

/// Os 14 dígitos do CNPJ do fundo; `None` sem CNPJ ou com um que não tem 14 dígitos.
fn cnpj_digits(asset: &assets::Model) -> Option<String> {
    asset.cnpj.as_deref().map(normalize_cnpj).filter(|digits| digits.len() == 14)
}

/// `00017024000153` -> `00.017.024/0001-53`.
fn format_cnpj(digits: &str) -> String {
    format!("{}.{}.{}/{}-{}", &digits[0..2], &digits[2..5], &digits[5..8], &digits[8..12], &digits[12..14])
}

/// Os fundos com CNPJ do Workspace, com a data desde a qual a cota interessa: a primeira compra (no máximo 2 anos atrás,
/// para a primeira busca não baixar o histórico todo) ou, sem compra, os últimos 90 dias. Fundo com estoque de 2023 pede
/// também o mês de dez/2023 (a cota de 31/12/2023), um zip só em vez de todo o histórico.
async fn fund_targets(
    db: &DatabaseConnection,
    workspace_id: i32,
    today: NaiveDate,
) -> Result<Vec<FundTarget>, AppError> {
    let fund_assets = assets::Entity::find().filter(assets::Column::AssetClass.eq(FUND_CLASS)).all(db).await?;
    let with_cnpj: Vec<(&assets::Model, String)> =
        fund_assets.iter().filter_map(|a| cnpj_digits(a).map(|digits| (a, digits))).collect();
    if with_cnpj.is_empty() {
        return Ok(Vec::new());
    }
    let ids: Vec<i32> = with_cnpj.iter().map(|(a, _)| a.id).collect();
    let subclasses: HashMap<i32, (Option<String>, bool)> = fund_detail::Entity::find()
        .filter(fund_detail::Column::AssetId.is_in(ids.clone()))
        .all(db)
        .await?
        .into_iter()
        .map(|d| {
            let wants_estoque = wants_estoque_quota(&d);
            (d.asset_id, (d.cvm_subclass, wants_estoque))
        })
        .collect();
    let portfolio_ids = workspace_portfolio_ids(db, workspace_id).await?;
    let buys = if portfolio_ids.is_empty() {
        Vec::new()
    } else {
        transactions::Entity::find()
            .filter(transactions::Column::PortfolioId.is_in(portfolio_ids))
            .filter(transactions::Column::AssetId.is_in(ids))
            .filter(transactions::Column::TransactionType.eq(BUY))
            .all(db)
            .await?
    };
    let oldest_allowed = today - Duration::days(730);
    Ok(with_cnpj
        .into_iter()
        .map(|(asset, cnpj)| {
            let first_buy = buys
                .iter()
                .filter(|t| t.asset_id == Some(asset.id))
                .filter_map(|t| NaiveDate::parse_from_str(&t.transaction_date, "%Y-%m-%d").ok())
                .min();
            let (subclass, wants_estoque) = subclasses.get(&asset.id).cloned().unwrap_or((None, false));
            // O estoque só existe para quem já tinha cotas em 31/12/2023.
            let held_in_2023 = first_buy.is_none_or(|d| d <= estoque_date());
            let extra_months = if wants_estoque && held_in_2023 {
                vec![(estoque_date().year(), estoque_date().month())]
            } else {
                Vec::new()
            };
            FundTarget {
                asset_id: asset.id,
                cnpj,
                subclass,
                since: first_buy.map_or(today - Duration::days(90), |d| d.max(oldest_allowed)),
                extra_months,
            }
        })
        .collect())
}

/// Completa as cotas da CVM dos fundos do Workspace. Nunca falha quem chama: sem rede permitida não faz nada, e uma falha
/// vai para o log (a tela segue com as cotas gravadas).
async fn refresh_fund_quotas(db: &DatabaseConnection, workspace_id: i32, today: NaiveDate, allow_network: bool) {
    if !allow_network {
        return;
    }
    if let Err(err) = refresh_fund_quotas_with(db, &SESSION, &CvmDailyZips, workspace_id, today, false).await {
        eprintln!("CVM informe diário: using the stored quotas after a refresh failure: {err}");
    }
}

pub async fn refresh_fund_quotas_with(
    db: &DatabaseConnection,
    session: &Session,
    fetcher: &dyn ZipFetcher,
    workspace_id: i32,
    today: NaiveDate,
    force: bool,
) -> Result<(), AppError> {
    let targets = fund_targets(db, workspace_id, today).await?;
    fund_quota_store::refresh(db, session, fetcher, &targets, today, force).await?;
    Ok(())
}

#[tauri::command]
pub async fn get_funds_overview(
    db: tauri::State<'_, DatabaseConnection>,
    workspace_id: i32,
) -> Result<FundsOverview, AppError> {
    let today = Utc::now().date_naive();
    refresh_fund_quotas(db.inner(), workspace_id, today, std::env::var_os(crate::ingestion::DISABLE_ENV).is_none()).await;
    funds_overview_in(db.inner(), workspace_id, today).await
}

/// O botão "Atualizar da CVM": ignora o intervalo entre buscas e devolve o motivo quando a CVM falha.
#[tauri::command]
pub async fn refresh_fund_quotas_now(
    db: tauri::State<'_, DatabaseConnection>,
    workspace_id: i32,
) -> Result<FundsOverview, AppError> {
    let today = Utc::now().date_naive();
    refresh_fund_quotas_with(db.inner(), &SESSION, &CvmDailyZips, workspace_id, today, true).await?;
    funds_overview_in(db.inner(), workspace_id, today).await
}

pub async fn funds_overview_in(
    db: &DatabaseConnection,
    workspace_id: i32,
    today: NaiveDate,
) -> Result<FundsOverview, AppError> {
    funds_overview_with(db, &SESSION, workspace_id, today).await
}

pub async fn funds_overview_with(
    db: &DatabaseConnection,
    session: &Session,
    workspace_id: i32,
    today: NaiveDate,
) -> Result<FundsOverview, AppError> {
    let fund_assets = assets::Entity::find()
        .filter(assets::Column::AssetClass.eq(FUND_CLASS))
        .order_by_asc(assets::Column::Ticker)
        .all(db)
        .await?;
    if fund_assets.is_empty() {
        return Ok(FundsOverview { funds: Vec::new(), estimates: Vec::new(), loss_credits: Vec::new() });
    }
    let ids: Vec<i32> = fund_assets.iter().map(|a| a.id).collect();

    let details: HashMap<i32, fund_detail::Model> = fund_detail::Entity::find()
        .filter(fund_detail::Column::AssetId.is_in(ids.clone()))
        .all(db)
        .await?
        .into_iter()
        .map(|d| (d.asset_id, d))
        .collect();
    let quota_rows = fund_quota_value::Entity::find()
        .filter(fund_quota_value::Column::AssetId.is_in(ids.clone()))
        .order_by_asc(fund_quota_value::Column::QuotaDate)
        .all(db)
        .await?;

    let portfolio_ids = workspace_portfolio_ids(db, workspace_id).await?;
    let txs = if portfolio_ids.is_empty() {
        Vec::new()
    } else {
        transactions::Entity::find()
            .filter(transactions::Column::PortfolioId.is_in(portfolio_ids))
            .filter(transactions::Column::AssetId.is_in(ids))
            .all(db)
            .await?
    };

    let rules = load_rule_set(db).await?;
    let mut funds = Vec::with_capacity(fund_assets.len());
    let mut holdings = Vec::with_capacity(fund_assets.len());
    for asset in &fund_assets {
        let asset_txs: Vec<&transactions::Model> = txs.iter().filter(|t| t.asset_id == Some(asset.id)).collect();
        let state = cost_basis::replay(&cost_events(&asset_txs)?, &cost_basis::native_rate)
            .expect("a taxa nativa sempre existe");
        let first_buy = asset_txs
            .iter()
            .filter(|t| t.transaction_type == BUY)
            .filter_map(|t| NaiveDate::parse_from_str(&t.transaction_date, "%Y-%m-%d").ok())
            .min();
        let quotas: Vec<fund_quota_value::Model> =
            quota_rows.iter().filter(|q| q.asset_id == asset.id).cloned().collect();
        let term = details.get(&asset.id).map(|d| d.term.clone());
        let last_cvm_date = quotas.iter().filter(|q| q.source == SOURCE_CVM).map(|q| q.quota_date.clone()).max();
        let cvm_status = match cnpj_digits(asset) {
            None => CvmStatus::NoCnpj,
            Some(_) => session
                .outcome(asset.id)
                .unwrap_or(if last_cvm_date.is_some() { CvmStatus::Ok } else { CvmStatus::Pending }),
        };

        let mut deductions: Vec<DeductionView> = asset_txs
            .iter()
            .filter(|t| t.transaction_type == COME_COTAS)
            .map(|t| {
                let quotas = t.quantity.unwrap_or(0.0);
                DeductionView {
                    tx_id: t.id,
                    date: t.transaction_date.clone(),
                    quotas,
                    quota_value: t.unit_price.unwrap_or(0.0),
                    tax: t.total_value,
                    loss_credit_used: t.loss_credit_used,
                }
            })
            .collect();
        deductions.sort_by(|a, b| (&a.date, a.tx_id).cmp(&(&b.date, b.tx_id)));

        let structure = details.get(&asset.id).map(|d| d.structure.clone()).unwrap_or_else(|| "aberto".to_string());
        let estoque_option = details.get(&asset.id).and_then(|d| d.estoque_option.clone());
        let fund_term = term.as_deref().and_then(FundTerm::parse);

        // Estoque de rendimento até 31/12/2023 (Lei 14.754, arts. 27 e 28): só fundo fechado ou exclusivo de longo ou curto
        // prazo; precisa da opção informada e da cota daquela data.
        let has_estoque = structure != "aberto" && matches!(fund_term, Some(FundTerm::LongTerm | FundTerm::ShortTerm));
        let reference_quota = estoque_quota(&quotas);
        let estoque_input = match (has_estoque, estoque_option.as_deref(), reference_quota) {
            (true, Some(option), Some((_, quota_value))) => {
                let key = if option == "opcao_8" { rules::FUND_ESTOQUE_RATE_OPTION } else { rules::FUND_ESTOQUE_RATE_GENERAL };
                Some((quota_value, rules.get(key, estoque_date())?))
            }
            _ => None,
        };

        // IR e IOF estimados de cada resgate e amortização: precisa do tipo do fundo.
        let (redemptions, estoque_estimate, taxed_basis_per_quota) = match fund_term {
            Some(fund_term) => {
                let estimate = fund_redemption::estimate_fund(&fund_events(&asset_txs, estoque_input), fund_term, &rules)?;
                (estimate.redemptions, estimate.estoque, estimate.taxed_basis_per_quota)
            }
            None => (Vec::new(), None, None),
        };
        let estoque = match (has_estoque, estoque_option.is_some(), estoque_estimate) {
            (false, ..) => EstoqueStatus::NotApplicable,
            (true, false, _) => EstoqueStatus::NotTaxed,
            (true, true, Some(estimate)) => EstoqueStatus::Estimated { estimate },
            (true, true, None) => EstoqueStatus::NeedsQuota,
        };

        holdings.push(FundHolding {
            asset_id: asset.id,
            ticker: asset.ticker.clone(),
            name: asset.name.clone(),
            term: term.as_deref().and_then(FundTerm::parse),
            quantity: state.quantity,
            average_cost: state.average_price().unwrap_or(0.0),
            taxed_basis_per_quota,
            first_buy,
            quotas: quotas
                .iter()
                .filter_map(|q| {
                    NaiveDate::parse_from_str(&q.quota_date, "%Y-%m-%d")
                        .ok()
                        .map(|date| QuotaValue { date, value: q.value })
                })
                .collect(),
            deductions: deductions
                .iter()
                .filter_map(|d| {
                    NaiveDate::parse_from_str(&d.date, "%Y-%m-%d")
                        .ok()
                        .map(|date| QuotaValue { date, value: d.quota_value })
                })
                .collect(),
        });
        funds.push(FundView {
            asset_id: asset.id,
            ticker: asset.ticker.clone(),
            name: asset.name.clone(),
            term,
            quantity: state.quantity,
            average_cost: state.average_price(),
            cnpj: asset.cnpj.clone(),
            cvm_subclass: details.get(&asset.id).and_then(|d| d.cvm_subclass.clone()),
            cvm_status,
            administrator: details.get(&asset.id).and_then(|d| d.administrator.clone()),
            structure,
            estoque_option,
            estoque,
            last_cvm_date,
            quotas,
            deductions,
            redemptions,
            credit_notes: Vec::new(),
        });
    }

    // Compensação de prejuízo entre fundos do mesmo grupo: ajusta o IR dos resgates e sobra um saldo por grupo, que ainda
    // abate o ganho da estimativa do próximo come-cotas.
    let (loss_credits, credit_notes) = {
        let mut comp: Vec<CompFund<'_>> = funds
            .iter_mut()
            .filter_map(|f| {
                let term = f.term.as_deref().and_then(FundTerm::parse)?;
                Some(CompFund {
                    asset_id: f.asset_id,
                    ticker: &f.ticker,
                    term,
                    administrator: f.administrator.as_deref(),
                    redemptions: &mut f.redemptions,
                    come_cotas: f
                        .deductions
                        .iter()
                        .filter_map(|d| {
                            let date = NaiveDate::parse_from_str(&d.date, "%Y-%m-%d").ok()?;
                            Some(ComeCotasMark { date, loss_credit_used: d.loss_credit_used })
                        })
                        .collect(),
                })
            })
            .collect();
        fund_compensation::compensate_with_notes(&mut comp)
    };
    for note in credit_notes {
        if let Some(fund) = funds.iter_mut().find(|f| f.asset_id == note.asset_id) {
            fund.credit_notes.push(note);
        }
    }
    let mut estimates = come_cotas::estimate(&holdings, &rules, today)?;
    fund_compensation::apply_loss_credit(&mut estimates, &loss_credits);
    Ok(FundsOverview { funds, estimates, loss_credits })
}

#[tauri::command]
pub async fn set_fund_term(
    db: tauri::State<'_, DatabaseConnection>,
    asset_id: i32,
    term: String,
) -> Result<(), AppError> {
    set_fund_term_in(db.inner(), asset_id, &term).await
}

pub async fn set_fund_term_in(db: &DatabaseConnection, asset_id: i32, term: &str) -> Result<(), AppError> {
    if FundTerm::parse(term).is_none() {
        return Err(AppError::InvalidInput(
            "term must be longo_prazo, curto_prazo, acoes or entidade_investimento".to_string(),
        ));
    }
    fund_asset(db, asset_id).await?;
    match fund_detail::Entity::find().filter(fund_detail::Column::AssetId.eq(asset_id)).one(db).await? {
        Some(row) => {
            let mut active: fund_detail::ActiveModel = row.into();
            active.term = Set(term.to_string());
            active.update(db).await?;
        }
        None => {
            fund_detail::ActiveModel {
                asset_id: Set(asset_id),
                term: Set(term.to_string()),
                created_at: Set(Utc::now().to_rfc3339()),
                ..Default::default()
            }
            .insert(db)
            .await?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn set_fund_administrator(
    db: tauri::State<'_, DatabaseConnection>,
    asset_id: i32,
    administrator: Option<String>,
) -> Result<(), AppError> {
    set_fund_administrator_in(db.inner(), asset_id, administrator.as_deref()).await
}

/// Grava a administradora (ou o intermediário) do fundo, em texto livre: fundos do mesmo tipo com a mesma administradora
/// compensam prejuízo entre si. Vazio ou ausente apaga (o fundo fica isolado). Mora em `fund_details`, que só existe depois
/// de o usuário escolher o tipo do fundo.
pub async fn set_fund_administrator_in(
    db: &DatabaseConnection,
    asset_id: i32,
    administrator: Option<&str>,
) -> Result<(), AppError> {
    fund_asset(db, asset_id).await?;
    let detail = fund_detail::Entity::find()
        .filter(fund_detail::Column::AssetId.eq(asset_id))
        .one(db)
        .await?
        .ok_or_else(|| AppError::InvalidInput("choose the fund type before the administrator".to_string()))?;
    let value = administrator
        .map(|a| a.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|a| !a.is_empty());
    let mut active: fund_detail::ActiveModel = detail.into();
    active.administrator = Set(value);
    active.update(db).await?;
    Ok(())
}

#[tauri::command]
pub async fn set_fund_structure(
    db: tauri::State<'_, DatabaseConnection>,
    asset_id: i32,
    structure: String,
) -> Result<(), AppError> {
    set_fund_structure_in(db.inner(), asset_id, &structure).await
}

/// Grava se o fundo é `aberto`, `fechado` ou `exclusivo` (Lei 14.754). Só os dois últimos têm estoque de 2023. Mora em
/// `fund_details`, que só existe depois de o usuário escolher o tipo do fundo.
pub async fn set_fund_structure_in(db: &DatabaseConnection, asset_id: i32, structure: &str) -> Result<(), AppError> {
    if !STRUCTURES.contains(&structure) {
        return Err(AppError::InvalidInput("structure must be aberto, fechado or exclusivo".to_string()));
    }
    let detail = fund_detail_of(db, asset_id, "structure").await?;
    let mut active: fund_detail::ActiveModel = detail.into();
    active.structure = Set(structure.to_string());
    active.update(db).await?;
    Ok(())
}

#[tauri::command]
pub async fn set_fund_estoque(
    db: tauri::State<'_, DatabaseConnection>,
    asset_id: i32,
    option: Option<String>,
) -> Result<(), AppError> {
    set_fund_estoque_in(db.inner(), asset_id, option.as_deref()).await
}

/// Grava como o estoque de rendimento até 31/12/2023 foi tributado: `geral` (15%, art. 27), `opcao_8` (8%, art. 28) ou
/// nada (não tributado, ou contestado na Justiça: o app não estima nada).
pub async fn set_fund_estoque_in(db: &DatabaseConnection, asset_id: i32, option: Option<&str>) -> Result<(), AppError> {
    let option = option.map(str::trim).filter(|o| !o.is_empty());
    if option.is_some_and(|o| !ESTOQUE_OPTIONS.contains(&o)) {
        return Err(AppError::InvalidInput("estoque option must be geral or opcao_8".to_string()));
    }
    let detail = fund_detail_of(db, asset_id, "estoque option").await?;
    let mut active: fund_detail::ActiveModel = detail.into();
    active.estoque_option = Set(option.map(str::to_string));
    active.update(db).await?;
    Ok(())
}

/// A linha de `fund_details` de um fundo; sem ela o usuário ainda não escolheu o tipo.
async fn fund_detail_of(db: &DatabaseConnection, asset_id: i32, what: &str) -> Result<fund_detail::Model, AppError> {
    fund_asset(db, asset_id).await?;
    fund_detail::Entity::find()
        .filter(fund_detail::Column::AssetId.eq(asset_id))
        .one(db)
        .await?
        .ok_or_else(|| AppError::InvalidInput(format!("choose the fund type before the {what}")))
}

#[derive(Deserialize)]
pub struct SetFundCvmRequest {
    pub asset_id: i32,
    /// Vazio ou ausente apaga o CNPJ.
    pub cnpj: Option<String>,
    /// Vazio ou ausente = a linha da classe.
    pub subclass: Option<String>,
}

#[tauri::command]
pub async fn set_fund_cvm(
    db: tauri::State<'_, DatabaseConnection>,
    request: SetFundCvmRequest,
) -> Result<(), AppError> {
    set_fund_cvm_in(db.inner(), &SESSION, request).await
}

/// Grava o CNPJ (`assets.cnpj`) e a subclasse do fundo para a leitura da CVM. Mudar um dos dois apaga as cotas já lidas da
/// CVM do fundo (eram de outra série); as lançadas à mão ficam. A subclasse mora em `fund_details`, que só existe depois
/// de o usuário escolher o tipo do fundo.
pub async fn set_fund_cvm_in(
    db: &DatabaseConnection,
    session: &Session,
    request: SetFundCvmRequest,
) -> Result<(), AppError> {
    let asset = fund_asset(db, request.asset_id).await?;
    let cnpj = match request.cnpj.as_deref().map(str::trim).filter(|c| !c.is_empty()) {
        None => None,
        Some(raw) => {
            let digits = normalize_cnpj(raw);
            if digits.len() != 14 {
                return Err(AppError::InvalidInput("CNPJ must have 14 digits".to_string()));
            }
            Some(format_cnpj(&digits))
        }
    };
    let subclass = request.subclass.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);

    let detail = fund_detail::Entity::find().filter(fund_detail::Column::AssetId.eq(asset.id)).one(db).await?;
    if subclass.is_some() && detail.is_none() {
        return Err(AppError::InvalidInput("choose the fund type before the CVM subclass".to_string()));
    }
    let changed = cnpj.as_deref().map(normalize_cnpj) != asset.cnpj.as_deref().map(normalize_cnpj)
        || subclass != detail.as_ref().and_then(|d| d.cvm_subclass.clone());

    let mut active_asset: assets::ActiveModel = asset.clone().into();
    active_asset.cnpj = Set(cnpj);
    active_asset.update(db).await?;
    if let Some(detail) = detail {
        let mut active: fund_detail::ActiveModel = detail.into();
        active.cvm_subclass = Set(subclass);
        active.update(db).await?;
    }
    if changed {
        fund_quota_value::Entity::delete_many()
            .filter(fund_quota_value::Column::AssetId.eq(asset.id))
            .filter(fund_quota_value::Column::Source.eq(SOURCE_CVM))
            .exec(db)
            .await?;
        session.forget(asset.id);
    }
    Ok(())
}

#[tauri::command]
pub async fn upsert_fund_quota(
    db: tauri::State<'_, DatabaseConnection>,
    request: UpsertFundQuotaRequest,
) -> Result<fund_quota_value::Model, AppError> {
    upsert_fund_quota_in(db.inner(), request, Utc::now().date_naive()).await
}

pub async fn upsert_fund_quota_in(
    db: &DatabaseConnection,
    request: UpsertFundQuotaRequest,
    today: NaiveDate,
) -> Result<fund_quota_value::Model, AppError> {
    let date = NaiveDate::parse_from_str(&request.quota_date, "%Y-%m-%d")
        .map_err(|_| AppError::InvalidInput("quota date must be YYYY-MM-DD".to_string()))?;
    if date > today {
        return Err(AppError::InvalidInput("quota date cannot be in the future".to_string()));
    }
    if !request.value.is_finite() || request.value <= 0.0 {
        return Err(AppError::InvalidInput("quota value must be above zero".to_string()));
    }
    fund_asset(db, request.asset_id).await?;

    let existing = fund_quota_value::Entity::find()
        .filter(fund_quota_value::Column::AssetId.eq(request.asset_id))
        .filter(fund_quota_value::Column::QuotaDate.eq(request.quota_date.clone()))
        .one(db)
        .await?;
    match existing {
        Some(row) => {
            let mut active: fund_quota_value::ActiveModel = row.into();
            active.value = Set(request.value);
            // Corrigir à mão uma cota da CVM a torna do usuário: a CVM não a sobrescreve mais.
            active.source = Set(fund_quota_store::SOURCE_MANUAL.to_string());
            Ok(active.update(db).await?)
        }
        None => Ok(fund_quota_value::ActiveModel {
            asset_id: Set(request.asset_id),
            quota_date: Set(request.quota_date),
            value: Set(request.value),
            source: Set(fund_quota_store::SOURCE_MANUAL.to_string()),
            created_at: Set(Utc::now().to_rfc3339()),
            ..Default::default()
        }
        .insert(db)
        .await?),
    }
}

#[tauri::command]
pub async fn delete_fund_quota(
    db: tauri::State<'_, DatabaseConnection>,
    quota_id: i32,
) -> Result<(), AppError> {
    fund_quota_value::Entity::delete_by_id(quota_id).exec(db.inner()).await?;
    Ok(())
}
