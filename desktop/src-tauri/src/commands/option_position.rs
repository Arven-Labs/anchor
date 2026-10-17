// Fase 18, 3ª fatia (Sessão 116) — posições em opções da B3 (comprada ou
// lançada) numa tabela própria, isolada de patrimônio, Resumo, rentabilidade e
// IR. O resultado realizado e a marcação a mercado são sempre derivados na
// leitura (`domain::option_position`), nunca gravados.
//
// A lista (`list_option_positions`) lê só o banco. A marcação a mercado
// (`get_option_marks`) usa o último preço negociado da B3, o que na primeira
// carga do dia baixa ~80 MB; por isso é um comando à parte, chamado por clique.

use chrono::NaiveDate;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};

use crate::domain::option_position::{self as calc, CLOSE_KINDS, EXPIRED, LONG, SHORT};
use crate::entity::{option_position, portfolio};
use crate::error::AppError;
use crate::finance_core::b3_options::{self, OptionsMarket};

fn invalid(detail: impl Into<String>) -> AppError {
    AppError::InvalidInput(detail.into())
}

fn parse_date(field: &str, value: &str) -> Result<NaiveDate, AppError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|e| invalid(format!("invalid {field}: {e}")))
}

fn clean_text(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

fn ticker(field: &str, value: &str) -> Result<String, AppError> {
    let value = value.trim().to_uppercase();
    if value.is_empty() {
        return Err(invalid(format!("{field} is required")));
    }
    Ok(value)
}

fn non_negative(field: &str, value: Option<f64>) -> Result<Option<f64>, AppError> {
    match value {
        Some(v) if !v.is_finite() || v < 0.0 => Err(invalid(format!("{field} cannot be negative"))),
        other => Ok(other),
    }
}

#[derive(Deserialize)]
pub struct CreateOptionPositionRequest {
    pub portfolio_id: i32,
    pub series_ticker: String,
    pub underlying_ticker: String,
    pub option_type: String,
    pub strike: f64,
    pub expiration: String,
    pub side: String,
    pub quantity: f64,
    pub premium: f64,
    pub open_date: String,
    pub open_fee: Option<f64>,
    pub notes: Option<String>,
}

#[tauri::command]
pub async fn create_option_position(
    db: tauri::State<'_, DatabaseConnection>,
    request: CreateOptionPositionRequest,
) -> Result<option_position::Model, AppError> {
    create_option_position_in(db.inner(), request).await
}

pub async fn create_option_position_in(
    db: &DatabaseConnection,
    request: CreateOptionPositionRequest,
) -> Result<option_position::Model, AppError> {
    let series_ticker = ticker("series ticker", &request.series_ticker)?;
    let underlying_ticker = ticker("underlying ticker", &request.underlying_ticker)?;
    if request.option_type != "call" && request.option_type != "put" {
        return Err(invalid("option type must be call or put"));
    }
    if request.side != LONG && request.side != SHORT {
        return Err(invalid("side must be long or short"));
    }
    if !request.strike.is_finite() || request.strike <= 0.0 {
        return Err(invalid("strike must be greater than zero"));
    }
    if !request.quantity.is_finite() || request.quantity <= 0.0 {
        return Err(invalid("quantity must be greater than zero"));
    }
    if !request.premium.is_finite() || request.premium < 0.0 {
        return Err(invalid("premium cannot be negative"));
    }
    let open_fee = non_negative("fee", request.open_fee)?;
    let expiration = parse_date("expiration", &request.expiration)?;
    let open_date = parse_date("open date", &request.open_date)?;
    if expiration < open_date {
        return Err(invalid("the expiration cannot be before the open date"));
    }

    portfolio::Entity::find_by_id(request.portfolio_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("portfolio {}", request.portfolio_id)))?;

    Ok(option_position::ActiveModel {
        portfolio_id: Set(request.portfolio_id),
        series_ticker: Set(series_ticker),
        underlying_ticker: Set(underlying_ticker),
        option_type: Set(request.option_type),
        strike: Set(request.strike),
        expiration: Set(request.expiration),
        side: Set(request.side),
        quantity: Set(request.quantity),
        premium: Set(request.premium),
        open_date: Set(request.open_date),
        open_fee: Set(open_fee),
        notes: Set(clean_text(request.notes)),
        created_at: Set(chrono::Utc::now().to_rfc3339()),
        ..Default::default()
    }
    .insert(db)
    .await?)
}

#[derive(Serialize, Debug)]
pub struct OptionPositionView {
    pub id: i32,
    pub portfolio_id: i32,
    pub series_ticker: String,
    pub underlying_ticker: String,
    pub option_type: String,
    pub strike: f64,
    pub expiration: String,
    pub side: String,
    pub quantity: f64,
    pub premium: f64,
    pub open_date: String,
    pub open_fee: Option<f64>,
    pub notes: Option<String>,
    pub close_kind: Option<String>,
    pub close_price: Option<f64>,
    pub close_date: Option<String>,
    pub close_fee: Option<f64>,
    pub is_open: bool,
    /// Aberta com o vencimento já passado: falta encerrar.
    pub is_expired: bool,
    pub days_to_expiry: i64,
    /// Só nas fechadas.
    pub realized_pnl: Option<f64>,
    pub include_in_net_worth: bool,
    /// Último preço guardado pela atualização de preços, e a data do negócio.
    pub mark_price: Option<f64>,
    pub mark_date: Option<String>,
    pub marked_at: Option<String>,
    /// Valor no patrimônio (comprada +, lançada −); só nas abertas. Não leva em conta a flag.
    pub net_worth_value: Option<f64>,
    /// O valor veio do prêmio, por falta de preço guardado.
    pub value_at_cost: bool,
    /// Resultado a mercado com o preço guardado (sem taxa de saída); só nas abertas com preço.
    pub market_pnl: Option<f64>,
}

fn view(row: option_position::Model, today: NaiveDate) -> OptionPositionView {
    let is_open = row.close_date.is_none();
    // A data foi validada na criação; se estiver corrompida, trata como "hoje".
    let expiration = NaiveDate::parse_from_str(&row.expiration, "%Y-%m-%d").unwrap_or(today);
    let realized_pnl = row.close_price.map(|price| {
        calc::result(&row.side, row.premium, row.quantity, price, row.open_fee, row.close_fee)
    });
    let is_expired = calc::is_expired(expiration, today, is_open);
    let value = is_open
        .then(|| calc::net_worth_value(&row.side, row.quantity, row.premium, row.mark_price, is_expired));
    let market_pnl = row
        .mark_price
        .filter(|_| is_open)
        .map(|price| calc::result(&row.side, row.premium, row.quantity, price, row.open_fee, None));
    OptionPositionView {
        is_open,
        is_expired,
        days_to_expiry: calc::days_to_expiry(expiration, today),
        realized_pnl,
        include_in_net_worth: row.include_in_net_worth,
        mark_price: row.mark_price,
        mark_date: row.mark_date,
        marked_at: row.marked_at,
        net_worth_value: value.map(|v| v.value),
        value_at_cost: value.is_some_and(|v| v.at_cost),
        market_pnl,
        id: row.id,
        portfolio_id: row.portfolio_id,
        series_ticker: row.series_ticker,
        underlying_ticker: row.underlying_ticker,
        option_type: row.option_type,
        strike: row.strike,
        expiration: row.expiration,
        side: row.side,
        quantity: row.quantity,
        premium: row.premium,
        open_date: row.open_date,
        open_fee: row.open_fee,
        notes: row.notes,
        close_kind: row.close_kind,
        close_price: row.close_price,
        close_date: row.close_date,
        close_fee: row.close_fee,
    }
}

/// As posições da carteira: abertas primeiro (vencimento mais próximo antes),
/// depois as fechadas (encerramento mais recente antes). Só lê o banco.
pub async fn list_option_positions_view(
    db: &DatabaseConnection,
    portfolio_id: i32,
) -> Result<Vec<OptionPositionView>, AppError> {
    let rows = option_position::Entity::find()
        .filter(option_position::Column::PortfolioId.eq(portfolio_id))
        .order_by_asc(option_position::Column::Expiration)
        .order_by_asc(option_position::Column::Id)
        .all(db)
        .await?;
    let today = chrono::Utc::now().date_naive();
    let (mut open, mut closed): (Vec<_>, Vec<_>) =
        rows.into_iter().map(|row| view(row, today)).partition(|v| v.is_open);
    closed.sort_by(|a, b| b.close_date.cmp(&a.close_date).then(b.id.cmp(&a.id)));
    open.extend(closed);
    Ok(open)
}

#[tauri::command]
pub async fn list_option_positions(
    db: tauri::State<'_, DatabaseConnection>,
    portfolio_id: i32,
) -> Result<Vec<OptionPositionView>, AppError> {
    list_option_positions_view(db.inner(), portfolio_id).await
}

#[derive(Deserialize)]
pub struct CloseOptionPositionRequest {
    pub id: i32,
    /// `closed`, `expired` ou `exercised`.
    pub close_kind: String,
    /// Ignorado em `expired` (vira 0); obrigatório nos outros dois.
    pub close_price: Option<f64>,
    pub close_date: String,
    pub close_fee: Option<f64>,
}

#[tauri::command]
pub async fn close_option_position(
    db: tauri::State<'_, DatabaseConnection>,
    request: CloseOptionPositionRequest,
) -> Result<option_position::Model, AppError> {
    close_option_position_in(db.inner(), request).await
}

pub async fn close_option_position_in(
    db: &DatabaseConnection,
    request: CloseOptionPositionRequest,
) -> Result<option_position::Model, AppError> {
    if !CLOSE_KINDS.contains(&request.close_kind.as_str()) {
        return Err(invalid(format!("close kind must be one of {CLOSE_KINDS:?}")));
    }
    let close_date = parse_date("close date", &request.close_date)?;
    let close_fee = non_negative("fee", request.close_fee)?;
    let close_price = if request.close_kind == EXPIRED {
        0.0
    } else {
        match non_negative("close price", request.close_price)? {
            Some(price) => price,
            None => return Err(invalid("the close price is required")),
        }
    };

    let row = option_position::Entity::find_by_id(request.id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("option position {}", request.id)))?;
    if row.close_date.is_some() {
        return Err(invalid("this position is already closed"));
    }
    if close_date < parse_date("open date", &row.open_date)? {
        return Err(invalid("the close date cannot be before the open date"));
    }

    let mut active: option_position::ActiveModel = row.into();
    active.close_kind = Set(Some(request.close_kind));
    active.close_price = Set(Some(close_price));
    active.close_date = Set(Some(request.close_date));
    active.close_fee = Set(close_fee);
    Ok(active.update(db).await?)
}

#[tauri::command]
pub async fn delete_option_position(
    db: tauri::State<'_, DatabaseConnection>,
    id: i32,
) -> Result<(), AppError> {
    delete_option_position_in(db.inner(), id).await
}

pub async fn delete_option_position_in(db: &DatabaseConnection, id: i32) -> Result<(), AppError> {
    let result = option_position::Entity::delete_by_id(id).exec(db).await?;
    if result.rows_affected == 0 {
        return Err(AppError::NotFound(format!("option position {id}")));
    }
    Ok(())
}

#[derive(Serialize, Debug, PartialEq)]
pub struct OptionMark {
    pub position_id: i32,
    pub series_ticker: String,
    /// `None` em série que nunca negociou.
    pub last_price: Option<f64>,
    pub last_trade_date: Option<String>,
    /// Resultado a mercado (último negócio, sem taxa de saída); `None` sem preço.
    pub unrealized_pnl: Option<f64>,
}

#[derive(Serialize, Debug)]
pub struct OptionMarksView {
    /// A B3 não respondeu e os arquivos são de antes.
    pub stale: bool,
    pub marks: Vec<OptionMark>,
}

/// Marcação a mercado das posições **abertas** a partir de um mercado já carregado.
pub async fn option_marks_in(
    db: &DatabaseConnection,
    portfolio_id: i32,
    market: &OptionsMarket,
) -> Result<Vec<OptionMark>, AppError> {
    let rows = option_position::Entity::find()
        .filter(option_position::Column::PortfolioId.eq(portfolio_id))
        .filter(option_position::Column::CloseDate.is_null())
        .order_by_asc(option_position::Column::Expiration)
        .order_by_asc(option_position::Column::Id)
        .all(db)
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let trade = market.last_trades.get(&row.series_ticker);
            OptionMark {
                position_id: row.id,
                last_trade_date: trade.map(|t| t.trade_date.to_string()),
                last_price: trade.map(|t| t.close_price),
                unrealized_pnl: trade.map(|t| {
                    calc::result(&row.side, row.premium, row.quantity, t.close_price, row.open_fee, None)
                }),
                series_ticker: row.series_ticker,
            }
        })
        .collect())
}

/// Guarda o último preço das posições que tiveram negócio, pro patrimônio (que
/// não usa rede) valorizá-las. Série sem negócio mantém o preço anterior.
pub async fn save_option_marks_in(db: &DatabaseConnection, marks: &[OptionMark]) -> Result<(), AppError> {
    let marked_at = chrono::Utc::now().to_rfc3339();
    for mark in marks {
        let (Some(price), Some(date)) = (mark.last_price, mark.last_trade_date.as_ref()) else {
            continue;
        };
        let Some(row) = option_position::Entity::find_by_id(mark.position_id).one(db).await? else {
            continue;
        };
        let mut active: option_position::ActiveModel = row.into();
        active.mark_price = Set(Some(price));
        active.mark_date = Set(Some(date.clone()));
        active.marked_at = Set(Some(marked_at.clone()));
        active.update(db).await?;
    }
    Ok(())
}

/// Último preço negociado e resultado a mercado das posições abertas, e guarda o
/// preço em cada posição (é ele que o patrimônio usa). A primeira carga do dia
/// baixa ~80 MB da B3 (~2 min); depois, 24 h de cache.
#[tauri::command]
pub async fn get_option_marks(
    db: tauri::State<'_, DatabaseConnection>,
    portfolio_id: i32,
) -> Result<OptionMarksView, AppError> {
    let cached = b3_options::market().await?;
    let marks = option_marks_in(db.inner(), portfolio_id, &cached.value.value).await?;
    save_option_marks_in(db.inner(), &marks).await?;
    Ok(OptionMarksView { stale: cached.stale, marks })
}

#[derive(Deserialize)]
pub struct SetOptionNetWorthRequest {
    pub id: i32,
    pub include: bool,
}

/// Liga ou desliga a posição no patrimônio.
#[tauri::command]
pub async fn set_option_position_net_worth(
    db: tauri::State<'_, DatabaseConnection>,
    request: SetOptionNetWorthRequest,
) -> Result<option_position::Model, AppError> {
    set_option_position_net_worth_in(db.inner(), request).await
}

pub async fn set_option_position_net_worth_in(
    db: &DatabaseConnection,
    request: SetOptionNetWorthRequest,
) -> Result<option_position::Model, AppError> {
    let row = option_position::Entity::find_by_id(request.id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("option position {}", request.id)))?;
    let mut active: option_position::ActiveModel = row.into();
    active.include_in_net_worth = Set(request.include);
    Ok(active.update(db).await?)
}
