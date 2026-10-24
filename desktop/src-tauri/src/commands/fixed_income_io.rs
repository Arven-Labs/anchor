// Fase 35, entrega 4 — rendimento do porquinho (e de LCI/LCA/CRI/CRA) derivado da
// planilha "Movimentação" da B3. O extrato do banco só mostra o que entrou e saiu da
// conta Porquinho; o rendimento aparece como resgate maior que o aplicado. Cada resgate
// da B3 se explica sozinho: o valor é o bruto (quantidade × preço unitário da curva), o
// principal é a quantidade × o preço unitário da aplicação, e a diferença é o rendimento
// bruto, de onde saem o IOF (CDB resgatado em menos de 30 dias, tabela do decreto, pelos
// dias aplicados de cada lote) e o IR (taxa fixa por papel em `tax_rules`, editável, sobre o
// que sobra do IOF). Não depende de saldo de abertura, só da data da aplicação, que o arquivo
// traz quando a aplicação está nele. A tabela regressiva do IR não é modelada.
//
// Cada resgate vira um item da fila (`kind = fixed_income_yield`) na conta Porquinho;
// aprovar grava duas linhas: receita bruta e despesa "IR retido" (categoria Taxes).
use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::NaiveDate;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set, TransactionTrait};
use serde::{Deserialize, Serialize};

use crate::commands::general_transaction::{create_general_transaction_in, CreateGeneralTransactionRequest};
use crate::commands::loan::ensure_account_in_workspace;
use crate::commands::portfolio_io::{
    movimentacao_columns, normalize_header, parse_date, parse_number, read_xlsx_rows, MovimentacaoColumns, ParseIssue,
    ProposedTransaction, MOVIMENTACAO_COLUMNS_MESSAGE, STATUS_PENDING,
};
use crate::commands::statement_io::{dedup_key, finish_pending, statement_row};
use crate::commands::tax::{ensure_taxes_category, ensure_yield_category, load_rule_set};
use crate::domain::tax::iof::iof_rate_for_days;
use crate::domain::tax::rules::{fixed_income_ir_key, RuleSet};
use crate::entity::{general_transaction, import_pending};
use crate::error::AppError;

pub const KIND_YIELD: &str = "fixed_income_yield";
const INSTRUMENTS: [&str; 5] = ["cdb", "lci", "lca", "cri", "cra"];

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// Um resgate de renda fixa com o rendimento derivado, ainda não gravado.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProposedYield {
    pub line: usize,
    pub transaction_date: String,
    /// `cdb`, `lci`, `lca`, `cri` ou `cra`.
    pub instrument: String,
    pub code: String,
    pub quantity: f64,
    pub principal: f64,
    /// Valor do resgate na B3 (bruto, antes do IR).
    pub gross_value: f64,
    pub gross_yield: f64,
    /// IOF sobre o rendimento (só CDB, só na parte com data de aplicação conhecida no
    /// arquivo); descontado antes do IR.
    #[serde(default)]
    pub iof_amount: f64,
    /// Dias aplicados (média ponderada dos lotes resgatados), quando conhecidos.
    #[serde(default)]
    pub holding_days: Option<u32>,
    /// `full` (toda a quantidade tem aplicação no arquivo), `partial`, `unknown` ou `not_applicable`.
    #[serde(default = "unknown_basis")]
    pub iof_basis: String,
    pub ir_rate: f64,
    /// IR sobre o rendimento depois do IOF.
    pub ir_amount: f64,
    pub net_yield: f64,
    pub base_unit_price: f64,
    /// `application` (preço da aplicação do mesmo código no arquivo) ou `file-wide`
    /// (preço comum a todas as aplicações do arquivo).
    pub base_price_source: String,
    /// Σ (principal + líquido) de todos os resgates do mesmo dia no arquivo.
    pub day_derived_total: f64,
    /// Só nas estimativas do porquinho (`base_price_source = "estimate"`, entrega 10): o percentual do CDI usado.
    #[serde(default)]
    pub estimate_pct_of_cdi: Option<f64>,
}

fn unknown_basis() -> String {
    "unknown".to_string()
}

pub type YieldParsed = Vec<Result<ProposedYield, ParseIssue>>;

/// Lotes de uma aplicação consumidos por um resgate (PEPS): (dias aplicados, cotas).
struct Holding {
    portions: Vec<(i64, f64)>,
    covered: f64,
}

/// `CDB - CDB926EMY03 - BANCO` → (`cdb`, `CDB926EMY03`). O primeiro pedaço é o papel.
fn parse_product(product: &str) -> Option<(String, String)> {
    let mut parts = product.split(" - ");
    let instrument = parts.next()?.trim().to_lowercase();
    let code = parts.next()?.trim().to_string();
    (INSTRUMENTS.contains(&instrument.as_str()) && !code.is_empty()).then_some((instrument, code))
}

/// Preço unitário das aplicações: por código e o comum a todas (se for um só).
fn application_prices(rows: &[Vec<String>], cols: &MovimentacaoColumns) -> (HashMap<String, f64>, Option<f64>) {
    let get = |row: &Vec<String>, i: usize| row.get(i).map(String::as_str).unwrap_or("").to_string();
    let mut application_price: HashMap<String, f64> = HashMap::new();
    let mut all_prices: Vec<f64> = Vec::new();
    for row in rows.iter().skip(1) {
        if !normalize_header(&get(row, cols.movement)).contains("aplicacao") {
            continue;
        }
        let Some((_, code)) = parse_product(&get(row, cols.product)) else { continue };
        if let Ok(Some(price)) = parse_number(&get(row, cols.price)) {
            application_price.insert(code, price);
            all_prices.push(price);
        }
    }
    let common_price = match all_prices.first() {
        Some(first) if all_prices.iter().all(|p| (p - first).abs() < 1e-12) => Some(*first),
        _ => None,
    };
    (application_price, common_price)
}

/// Lotes de cada código (data da aplicação, cotas). Os resgates, em ordem cronológica,
/// consomem os lotes mais antigos primeiro (PEPS): é daí que saem os dias aplicados do IOF e
/// o quanto de cada resgate tem aplicação no arquivo. A chave é a linha do resgate.
fn file_holdings(rows: &[Vec<String>], cols: &MovimentacaoColumns) -> HashMap<usize, Holding> {
    let get = |row: &Vec<String>, i: usize| row.get(i).map(String::as_str).unwrap_or("").to_string();
    let mut lots: HashMap<String, Vec<(NaiveDate, f64)>> = HashMap::new();
    let mut redemptions: Vec<(NaiveDate, usize, String, f64)> = Vec::new();
    for (index, row) in rows.iter().enumerate().skip(1) {
        let Some((_, code)) = parse_product(&get(row, cols.product)) else { continue };
        let Some(date) = parse_date(&get(row, cols.date)).and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()) else {
            continue;
        };
        let Ok(Some(quantity)) = parse_number(&get(row, cols.quantity)) else { continue };
        let movement = normalize_header(&get(row, cols.movement));
        if movement.contains("aplicacao") {
            lots.entry(code).or_default().push((date, quantity));
        } else if movement.contains("resgate") || movement.contains("vencimento") {
            redemptions.push((date, index, code, quantity));
        }
    }
    for code_lots in lots.values_mut() {
        code_lots.sort_by_key(|lot| lot.0);
    }
    redemptions.sort_by_key(|r| (r.0, r.1));
    let mut holdings: HashMap<usize, Holding> = HashMap::new();
    for (date, line, code, quantity) in redemptions {
        let mut left = quantity;
        let mut holding = Holding { portions: Vec::new(), covered: 0.0 };
        for lot in lots.get_mut(&code).into_iter().flatten() {
            if left <= 1e-9 {
                break;
            }
            if lot.1 <= 1e-9 || lot.0 > date {
                continue;
            }
            let take = lot.1.min(left);
            lot.1 -= take;
            left -= take;
            holding.covered += take;
            holding.portions.push(((date - lot.0).num_days(), take));
        }
        holdings.insert(line, holding);
    }
    holdings
}

pub fn parse_fixed_income_rows(rows: &[Vec<String>], rules: &RuleSet) -> YieldParsed {
    let Some(header) = rows.first() else {
        return vec![Err(ParseIssue { line: 0, message: "planilha vazia".to_string() })];
    };
    let Some(cols) = movimentacao_columns(header) else {
        return vec![Err(ParseIssue { line: 0, message: MOVIMENTACAO_COLUMNS_MESSAGE.to_string() })];
    };
    let get = |row: &Vec<String>, i: usize| row.get(i).map(String::as_str).unwrap_or("").to_string();

    let (application_price, common_price) = application_prices(rows, &cols);
    let holdings = file_holdings(rows, &cols);

    let mut out: YieldParsed = Vec::new();
    for (index, row) in rows.iter().enumerate().skip(1) {
        let line = index;
        if row.iter().all(|c| c.trim().is_empty()) {
            continue;
        }
        let issue = |message: String| ParseIssue { line, message };
        let movement = normalize_header(&get(row, cols.movement));
        let product = get(row, cols.product);
        let Some((instrument, code)) = parse_product(&product) else {
            out.push(Err(issue("não é renda fixa: os proventos e negócios entram pelo Ledger".to_string())));
            continue;
        };
        if movement.contains("aplicacao") {
            out.push(Err(issue("aplicação pulada: o dinheiro entra pelo extrato".to_string())));
            continue;
        }
        if !(movement.contains("resgate") || movement.contains("vencimento")) {
            out.push(Err(issue(format!("movimentação '{}' de renda fixa não suportada", get(row, cols.movement).trim()))));
            continue;
        }
        let Some(transaction_date) = parse_date(&get(row, cols.date)) else {
            out.push(Err(issue(format!("data inválida: '{}'", get(row, cols.date)))));
            continue;
        };
        let amounts = (|| -> Result<(f64, f64), String> {
            let quantity = parse_number(&get(row, cols.quantity))?.filter(|q| *q > 0.0).ok_or("quantidade vazia")?;
            let value = parse_number(&get(row, cols.value))?.ok_or("valor vazio")?;
            Ok((quantity, value))
        })();
        let (quantity, gross_value) = match amounts {
            Ok(values) => values,
            Err(message) => {
                out.push(Err(issue(message)));
                continue;
            }
        };
        let (base_unit_price, base_price_source) = match (application_price.get(&code), common_price) {
            (Some(price), _) => (*price, "application"),
            (None, Some(price)) => (price, "file-wide"),
            (None, None) => {
                out.push(Err(issue(format!(
                    "sem preço-base para {code}: não há aplicação dele no arquivo e as aplicações têm preços diferentes"
                ))));
                continue;
            }
        };
        let principal = round2(quantity * base_unit_price);
        let gross_yield = round2(gross_value - principal);
        if gross_yield <= 0.0 {
            out.push(Err(issue("sem rendimento: o valor do resgate não passa do principal".to_string())));
            continue;
        }
        let date = match NaiveDate::parse_from_str(&transaction_date, "%Y-%m-%d") {
            Ok(date) => date,
            Err(e) => {
                out.push(Err(issue(format!("data inválida: {e}"))));
                continue;
            }
        };
        let key = fixed_income_ir_key(&instrument).expect("papel já validado");
        let ir_rate = match rules.get(key, date) {
            Ok(rate) => rate,
            Err(e) => {
                out.push(Err(issue(e.to_string())));
                continue;
            }
        };
        // IOF só no CDB e só na parte cuja aplicação está no arquivo; o resto fica sem IOF e
        // marcado (`partial`/`unknown`) pra o usuário acertar com o que o banco pagou.
        let (iof_amount, holding_days, iof_basis) = if instrument == "cdb" {
            match holdings.get(&line).filter(|h| h.covered > 1e-9) {
                Some(h) => {
                    let iof: f64 = h
                        .portions
                        .iter()
                        .map(|(days, units)| gross_yield * (units / quantity) * iof_rate_for_days(*days))
                        .sum();
                    let weighted_days: f64 = h.portions.iter().map(|(days, units)| *days as f64 * units).sum::<f64>() / h.covered;
                    let basis = if h.covered >= quantity - 1e-9 { "full" } else { "partial" };
                    (round2(iof), Some(weighted_days.round().max(0.0) as u32), basis)
                }
                None => (0.0, None, "unknown"),
            }
        } else {
            (0.0, None, "not_applicable")
        };
        let ir_amount = round2((gross_yield - iof_amount) * ir_rate);
        out.push(Ok(ProposedYield {
            line,
            transaction_date,
            instrument,
            code,
            quantity,
            principal,
            gross_value,
            gross_yield,
            iof_amount,
            holding_days,
            iof_basis: iof_basis.to_string(),
            ir_rate,
            ir_amount,
            net_yield: round2(gross_yield - iof_amount - ir_amount),
            base_unit_price,
            base_price_source: base_price_source.to_string(),
            day_derived_total: 0.0,
            estimate_pct_of_cdi: None,
        }));
    }

    let mut per_day: BTreeMap<String, f64> = BTreeMap::new();
    for item in out.iter().flatten() {
        *per_day.entry(item.transaction_date.clone()).or_insert(0.0) += item.principal + item.net_yield;
    }
    for item in out.iter_mut().flatten() {
        item.day_derived_total = round2(per_day[&item.transaction_date]);
    }
    out
}

/// Posição de carteira da renda fixa (entrega 8): a aplicação vira `compra` e o resgate/vencimento vira
/// `venda` **ao custo** (quantidade × preço-base da aplicação), de um ativo `renda_fixa` com o código do
/// papel. O rendimento não entra aqui, porque já entra na conta pelo leitor acima; vender ao custo deixa o
/// ganho realizado em zero e não o conta duas vezes. A chave do mapa é a linha da planilha.
///
/// A venda leva a quantidade inteira do resgate. Se a aplicação está fora do arquivo, ela só passa na
/// confirmação quando a posição já foi importada antes (a validação recusa vender mais do que a carteira tem);
/// a nota do lançamento avisa quando o arquivo não cobre o resgate.
pub fn parse_fixed_income_positions(rows: &[Vec<String>]) -> HashMap<usize, Result<ProposedTransaction, ParseIssue>> {
    let mut out = HashMap::new();
    let Some(cols) = rows.first().and_then(|header| movimentacao_columns(header)) else {
        return out;
    };
    let get = |row: &Vec<String>, i: usize| row.get(i).map(String::as_str).unwrap_or("").to_string();
    let (application_price, common_price) = application_prices(rows, &cols);
    let holdings = file_holdings(rows, &cols);

    for (index, row) in rows.iter().enumerate().skip(1) {
        let line = index;
        let Some((instrument, code)) = parse_product(&get(row, cols.product)) else { continue };
        let issue = |message: String| ParseIssue { line, message };
        let movement = normalize_header(&get(row, cols.movement));
        let is_application = movement.contains("aplicacao");
        let is_redemption = movement.contains("resgate") || movement.contains("vencimento");
        if !is_application && !is_redemption {
            out.insert(
                line,
                Err(issue(format!("movimentação '{}' de renda fixa não suportada", get(row, cols.movement).trim()))),
            );
            continue;
        }
        let Some(transaction_date) = parse_date(&get(row, cols.date)) else {
            out.insert(line, Err(issue(format!("data inválida: '{}'", get(row, cols.date)))));
            continue;
        };
        let quantity = match parse_number(&get(row, cols.quantity)) {
            Ok(Some(q)) if q > 0.0 => q,
            Ok(_) => {
                out.insert(line, Err(issue("quantidade vazia".to_string())));
                continue;
            }
            Err(message) => {
                out.insert(line, Err(issue(message)));
                continue;
            }
        };
        let custody = Some(get(row, cols.institution).trim().to_string()).filter(|s| !s.is_empty());
        let (transaction_type, unit_price, total_value, notes) = if is_application {
            let price = parse_number(&get(row, cols.price)).ok().flatten();
            let value = parse_number(&get(row, cols.value)).ok().flatten();
            let amounts = match (price, value) {
                (Some(p), Some(v)) => Some((p, v)),
                (Some(p), None) => Some((p, round2(quantity * p))),
                (None, Some(v)) => Some((v / quantity, v)),
                (None, None) => None,
            };
            let Some((unit_price, total_value)) = amounts else {
                out.insert(line, Err(issue("aplicação sem preço nem valor".to_string())));
                continue;
            };
            (
                "compra",
                unit_price,
                total_value,
                format!("Importado da B3 (Movimentação); aplicação em {} {code}", instrument.to_uppercase()),
            )
        } else {
            let base = match (application_price.get(&code), common_price) {
                (Some(price), _) => *price,
                (None, Some(price)) => price,
                (None, None) => {
                    out.insert(
                        line,
                        Err(issue(format!(
                            "sem preço-base para {code}: não há aplicação dele no arquivo e as aplicações têm preços diferentes"
                        ))),
                    );
                    continue;
                }
            };
            let covered = holdings.get(&line).map_or(0.0, |h| h.covered);
            let mut notes = format!(
                "Importado da B3 (Movimentação); {} de {} {code} ao custo (principal): o rendimento entra pela conta",
                if movement.contains("vencimento") { "vencimento" } else { "resgate" },
                instrument.to_uppercase()
            );
            if covered < quantity - 1e-9 {
                notes.push_str("; a aplicação não está toda no arquivo, a venda só passa se a posição já foi importada");
            }
            ("venda", base, round2(quantity * base), notes)
        };
        out.insert(
            line,
            Ok(ProposedTransaction {
                line,
                transaction_date,
                transaction_type: transaction_type.to_string(),
                ticker: code,
                asset_class: Some("renda_fixa".to_string()),
                currency: Some("BRL".to_string()),
                quantity: Some(quantity),
                unit_price: Some(unit_price),
                total_value,
                fee: None,
                custody,
                payment_type: None,
                notes: Some(notes),
                ..Default::default()
            }),
        );
    }
    out
}

// ---------------------------------------------------------------------------
// Fila
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct YieldPayload {
    pub proposed: ProposedYield,
}

#[derive(Serialize)]
pub struct YieldStageResult {
    pub staged: usize,
    pub duplicates: usize,
    pub skipped: Vec<ParseIssue>,
}

/// Receitas já gravadas na conta e itens de rendimento da fila (qualquer status),
/// contados por (data, rendimento bruto em centavos).
async fn known_yield_keys(
    db: &DatabaseConnection,
    pocket_account_id: i32,
) -> Result<HashMap<(String, i64), usize>, AppError> {
    let mut keys: HashMap<(String, i64), usize> = HashMap::new();
    let ledger = general_transaction::Entity::find()
        .filter(general_transaction::Column::BankAccountId.eq(pocket_account_id))
        .filter(general_transaction::Column::TransactionType.eq("receita"))
        .all(db)
        .await?;
    for tx in ledger {
        *keys.entry(dedup_key(&tx.transaction_date, tx.valor)).or_insert(0) += 1;
    }
    let queued = import_pending::Entity::find()
        .filter(import_pending::Column::BankAccountId.eq(pocket_account_id))
        .filter(import_pending::Column::Kind.eq(KIND_YIELD))
        .all(db)
        .await?;
    for row in queued {
        let Ok(payload) = serde_json::from_str::<YieldPayload>(&row.payload) else { continue };
        *keys.entry(dedup_key(&payload.proposed.transaction_date, payload.proposed.gross_yield)).or_insert(0) += 1;
    }
    Ok(keys)
}

/// Origem (`import_pending.source`) das propostas da estimativa do porquinho (entrega 10).
pub const ESTIMATE_SOURCE: &str = "porquinho_estimate";
/// Começo da nota da receita que a aprovação de uma estimativa grava (`confirm_yield_pending_in`: "Yield {papel} (gross)").
const ESTIMATE_BOOKED_NOTE_PREFIX: &str = "Yield CDB Porquinho";

/// O valor da B3 é exato e a estimativa não: antes de enfileirar os da B3, apaga as estimativas **ainda pendentes** dos mesmos
/// dias e pula o dia que já tem uma estimativa **aprovada** (receita já lançada), para o rendimento não contar duas vezes.
async fn reconcile_with_estimates(
    db: &DatabaseConnection,
    pocket_account_id: i32,
    parsed: YieldParsed,
) -> Result<YieldParsed, AppError> {
    let days: HashSet<String> = parsed.iter().flatten().map(|p| p.transaction_date.clone()).collect();
    if days.is_empty() {
        return Ok(parsed);
    }
    let pending_estimates = import_pending::Entity::find()
        .filter(import_pending::Column::BankAccountId.eq(pocket_account_id))
        .filter(import_pending::Column::Kind.eq(KIND_YIELD))
        .filter(import_pending::Column::Source.eq(ESTIMATE_SOURCE))
        .filter(import_pending::Column::Status.eq(STATUS_PENDING))
        .all(db)
        .await?;
    for row in pending_estimates {
        let Ok(payload) = serde_json::from_str::<YieldPayload>(&row.payload) else { continue };
        if days.contains(&payload.proposed.transaction_date) {
            import_pending::Entity::delete_by_id(row.id).exec(db).await?;
        }
    }
    let booked: HashSet<String> = general_transaction::Entity::find()
        .filter(general_transaction::Column::BankAccountId.eq(pocket_account_id))
        .filter(general_transaction::Column::TransactionType.eq("receita"))
        .all(db)
        .await?
        .into_iter()
        .filter(|tx| tx.notes.as_deref().is_some_and(|n| n.starts_with(ESTIMATE_BOOKED_NOTE_PREFIX)))
        .map(|tx| tx.transaction_date)
        .collect();
    Ok(parsed
        .into_iter()
        .map(|row| match row {
            Ok(p) if booked.contains(&p.transaction_date) => Err(ParseIssue {
                line: p.line,
                message: format!(
                    "o dia {} já tem um rendimento estimado lançado: edite esse lançamento se o valor da B3 for diferente",
                    p.transaction_date
                ),
            }),
            other => other,
        })
        .collect())
}

pub async fn stage_yield_parsed(
    db: &DatabaseConnection,
    workspace_id: i32,
    pocket_account_id: i32,
    source: &str,
    parsed: YieldParsed,
) -> Result<YieldStageResult, AppError> {
    ensure_account_in_workspace(db, pocket_account_id, workspace_id).await?;
    let parsed = if source == ESTIMATE_SOURCE {
        parsed
    } else {
        reconcile_with_estimates(db, pocket_account_id, parsed).await?
    };
    let mut remaining = known_yield_keys(db, pocket_account_id).await?;
    let now = chrono::Utc::now().to_rfc3339();
    let txn = db.begin().await?;
    let mut result = YieldStageResult { staged: 0, duplicates: 0, skipped: Vec::new() };
    for row in parsed {
        let proposed = match row {
            Ok(p) => p,
            Err(issue) => {
                result.skipped.push(issue);
                continue;
            }
        };
        let key = dedup_key(&proposed.transaction_date, proposed.gross_yield);
        if let Some(count) = remaining.get_mut(&key).filter(|c| **c > 0) {
            *count -= 1;
            result.duplicates += 1;
            continue;
        }
        let payload = serde_json::to_string(&YieldPayload { proposed }).map_err(|e| AppError::InvalidInput(e.to_string()))?;
        import_pending::ActiveModel {
            workspace_id: Set(workspace_id),
            bank_account_id: Set(Some(pocket_account_id)),
            kind: Set(KIND_YIELD.to_string()),
            source: Set(source.to_string()),
            payload: Set(payload),
            status: Set(STATUS_PENDING.to_string()),
            created_at: Set(now.clone()),
            ..Default::default()
        }
        .insert(&txn)
        .await?;
        result.staged += 1;
    }
    txn.commit().await?;
    Ok(result)
}

pub async fn stage_fixed_income_yield_in(
    db: &DatabaseConnection,
    workspace_id: i32,
    pocket_account_id: i32,
    path: &str,
) -> Result<YieldStageResult, AppError> {
    let rows = read_xlsx_rows(path)?;
    let rules = load_rule_set(db).await?;
    stage_yield_parsed(db, workspace_id, pocket_account_id, "b3_fixed_income", parse_fixed_income_rows(&rows, &rules)).await
}

/// O que o usuário deixou na tela ao aprovar um rendimento.
#[derive(Clone, Debug, Deserialize)]
pub struct EditedYield {
    pub transaction_date: String,
    pub gross_yield: f64,
    #[serde(default)]
    pub iof_amount: f64,
    pub ir_amount: f64,
    pub notes: Option<String>,
}

/// Aprova um rendimento: receita bruta na conta Porquinho e, para cada imposto retido
/// (IOF, IR) maior que zero, uma despesa na categoria Taxes, numa transação só.
pub async fn confirm_yield_pending_in(
    db: &DatabaseConnection,
    workspace_id: i32,
    id: i32,
    edited: EditedYield,
) -> Result<(), AppError> {
    let row = statement_row(db, workspace_id, id).await?;
    if row.kind != KIND_YIELD {
        return Err(AppError::InvalidInput(format!("pendente {id} não é um rendimento")));
    }
    let payload: YieldPayload = serde_json::from_str(&row.payload)
        .map_err(|e| AppError::InvalidInput(format!("pendente {id} ilegível: {e}")))?;
    let account_id = row.bank_account_id.ok_or_else(|| AppError::InvalidInput(format!("pendente {id} sem conta")))?;
    if !edited.gross_yield.is_finite() || edited.gross_yield <= 0.0 {
        return Err(AppError::InvalidInput("o rendimento bruto tem de ser positivo".to_string()));
    }
    if [edited.ir_amount, edited.iof_amount].iter().any(|v| !v.is_finite() || *v < 0.0)
        || edited.ir_amount + edited.iof_amount > edited.gross_yield + 1e-9
    {
        return Err(AppError::InvalidInput(
            "o IOF e o IR retidos não podem ser negativos nem passar do rendimento bruto".to_string(),
        ));
    }
    let date = parse_date(&edited.transaction_date)
        .ok_or_else(|| AppError::InvalidInput(format!("data inválida: '{}'", edited.transaction_date)))?;
    let proposed = &payload.proposed;
    let label = format!("{} {}", proposed.instrument.to_uppercase(), proposed.code);
    let key_changed = dedup_key(&date, edited.gross_yield) != dedup_key(&proposed.transaction_date, proposed.gross_yield);

    let txn = db.begin().await?;
    let yield_category = ensure_yield_category(&txn, workspace_id).await?;
    create_general_transaction_in(
        &txn,
        CreateGeneralTransactionRequest {
            workspace_id,
            bank_account_id: account_id,
            transaction_type: "receita".to_string(),
            category_id: Some(yield_category),
            valor: edited.gross_yield,
            transaction_date: date.clone(),
            notes: Some(edited.notes.clone().unwrap_or_else(|| format!("Yield {label} (gross)"))),
        },
    )
    .await?;
    let withheld = [
        (edited.iof_amount, format!("IOF withheld on {label} yield")),
        (edited.ir_amount, format!("IR withheld on {label} yield ({:.1}%)", proposed.ir_rate * 100.0)),
    ];
    for (amount, notes) in withheld.into_iter().filter(|(amount, _)| *amount > 0.0) {
        let taxes = ensure_taxes_category(&txn, workspace_id).await?;
        create_general_transaction_in(
            &txn,
            CreateGeneralTransactionRequest {
                workspace_id,
                bank_account_id: account_id,
                transaction_type: "despesa".to_string(),
                category_id: Some(taxes),
                valor: amount,
                transaction_date: date.clone(),
                notes: Some(notes),
            },
        )
        .await?;
    }
    finish_pending(&txn, row, key_changed).await?;
    txn.commit().await?;
    Ok(())
}

/// Total que saiu da conta Porquinho por data (pernas `transferencia_saida` já no
/// ledger): é o que o extrato mostra como resgate recebido naquele dia.
pub async fn bank_totals_by_day(
    db: &DatabaseConnection,
    account_ids: &HashSet<i32>,
) -> Result<HashMap<(i32, String), f64>, AppError> {
    let mut totals: HashMap<(i32, String), f64> = HashMap::new();
    if account_ids.is_empty() {
        return Ok(totals);
    }
    let rows = general_transaction::Entity::find()
        .filter(general_transaction::Column::BankAccountId.is_in(account_ids.iter().copied().collect::<Vec<_>>()))
        .filter(general_transaction::Column::TransactionType.eq("transferencia_saida"))
        .all(db)
        .await?;
    for tx in rows {
        *totals.entry((tx.bank_account_id, tx.transaction_date)).or_insert(0.0) += tx.valor;
    }
    Ok(totals)
}

// ---------------------------------------------------------------------------
// Comandos
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn stage_fixed_income_yield(
    db: tauri::State<'_, DatabaseConnection>,
    workspace_id: i32,
    pocket_account_id: i32,
    path: String,
) -> Result<YieldStageResult, AppError> {
    stage_fixed_income_yield_in(db.inner(), workspace_id, pocket_account_id, &path).await
}

#[tauri::command]
pub async fn confirm_yield_pending(
    db: tauri::State<'_, DatabaseConnection>,
    workspace_id: i32,
    id: i32,
    edited: EditedYield,
) -> Result<(), AppError> {
    confirm_yield_pending_in(db.inner(), workspace_id, id, edited).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::tax::rules::Rule;

    // Dados inventados: nenhum código, valor ou data de planilha real.
    const HEADER: [&str; 8] = [
        "Entrada/Saída",
        "Data",
        "Movimentação",
        "Produto",
        "Instituição",
        "Quantidade",
        "Preço unitário",
        "Valor da Operação",
    ];

    fn rows(lines: &[&[&str]]) -> Vec<Vec<String>> {
        let mut all = vec![HEADER.iter().map(|c| c.to_string()).collect::<Vec<_>>()];
        all.extend(lines.iter().map(|l| l.iter().map(|c| c.to_string()).collect()));
        all
    }

    fn rules() -> RuleSet {
        let d = NaiveDate::from_ymd_opt(2005, 1, 1).unwrap();
        let rule = |key: &str, value: f64| Rule { key: key.to_string(), effective_from: d, value };
        RuleSet::new(vec![
            rule("fixed_income_ir_cdb", 0.225),
            rule("fixed_income_ir_lci", 0.0),
            rule("fixed_income_ir_lca", 0.0),
            rule("fixed_income_ir_cri", 0.0),
            rule("fixed_income_ir_cra", 0.0),
        ])
    }

    fn ok(parsed: &YieldParsed, i: usize) -> &ProposedYield {
        parsed[i].as_ref().unwrap_or_else(|e| panic!("linha {i}: {}", e.message))
    }

    #[test]
    fn an_application_is_a_buy_and_a_redemption_is_a_sale_at_cost() {
        let sheet = rows(&[
            &["Debito", "02/03/2026", "APLICAÇÃO", "CDB - CDBAAA111 - BANCO X", "BANCO X", "1000", "0.01", "10"],
            &["Credito", "20/03/2026", "RESGATE", "CDB - CDBAAA111 - BANCO X", "BANCO X", "400", "0.0102", "4.08"],
        ]);
        let positions = parse_fixed_income_positions(&sheet);
        let buy = positions[&1].as_ref().unwrap();
        assert_eq!((buy.transaction_type.as_str(), buy.ticker.as_str(), buy.asset_class.as_deref()), ("compra", "CDBAAA111", Some("renda_fixa")));
        assert_eq!((buy.quantity, buy.unit_price, buy.total_value), (Some(1000.0), Some(0.01), 10.0));
        assert_eq!(buy.transaction_date, "2026-03-02");
        assert_eq!(buy.custody.as_deref(), Some("BANCO X"));

        // A venda é ao custo: o preço é o da aplicação, não o do resgate; o rendimento (4,08 − 4,00) fica de fora.
        let sell = positions[&2].as_ref().unwrap();
        assert_eq!(sell.transaction_type, "venda");
        assert_eq!((sell.quantity, sell.unit_price, sell.total_value), (Some(400.0), Some(0.01), 4.0));
        assert!(!sell.notes.as_deref().unwrap().contains("não está toda"), "a aplicação cobre o resgate");
    }

    #[test]
    fn a_redemption_without_its_application_in_the_file_is_flagged_and_uses_the_common_price() {
        let sheet = rows(&[
            &["Debito", "02/03/2026", "APLICAÇÃO", "CDB - CDBAAA111 - BANCO X", "BANCO X", "100", "0.01", "1"],
            &["Credito", "20/03/2026", "RESGATE", "CDB - CDBBBB222 - BANCO X", "BANCO X", "300", "0.0105", "3.15"],
        ]);
        let positions = parse_fixed_income_positions(&sheet);
        let sell = positions[&2].as_ref().unwrap();
        assert_eq!((sell.quantity, sell.unit_price, sell.total_value), (Some(300.0), Some(0.01), 3.0));
        assert!(sell.notes.as_deref().unwrap().contains("não está toda"));
    }

    #[test]
    fn position_rows_skip_with_a_reason_when_the_price_base_is_ambiguous_or_the_movement_is_unknown() {
        let sheet = rows(&[
            &["Debito", "02/03/2026", "APLICAÇÃO", "CDB - CDBAAA111 - BANCO X", "BANCO X", "100", "0.01", "1"],
            &["Debito", "03/03/2026", "APLICAÇÃO", "CDB - CDBBBB222 - BANCO X", "BANCO X", "100", "0.02", "2"],
            &["Credito", "20/03/2026", "RESGATE", "CDB - CDBCCC333 - BANCO X", "BANCO X", "300", "0.0105", "3.15"],
            &["Credito", "21/03/2026", "Juros", "CDB - CDBAAA111 - BANCO X", "BANCO X", "1", "-", "-"],
            &["Credito", "22/03/2026", "Dividendo", "ABCD3 - EMPRESA A S.A.", "CORRETORA", "10", "0.1", "1"],
        ]);
        let positions = parse_fixed_income_positions(&sheet);
        assert!(positions[&3].as_ref().unwrap_err().message.contains("sem preço-base"));
        assert!(positions[&4].as_ref().unwrap_err().message.contains("não suportada"));
        assert!(!positions.contains_key(&5), "linha que não é renda fixa fica com o outro leitor");
    }

    #[test]
    fn a_cdb_redemption_splits_principal_gross_ir_and_net() {
        let parsed = parse_fixed_income_rows(
            &rows(&[
                &["Credito", "01/01/2026", "APLICAÇÃO", "CDB - CDBAAA", "BANCO", "10000", "0.01", "100"],
                &["Debito", "05/03/2026", "RESGATE ANTECIPADO/", "CDB - CDBAAA - BANCO", "BANCO", "5000", "0.0101", "50.50"],
            ]),
            &rules(),
        );
        assert!(parsed[0].as_ref().unwrap_err().message.contains("aplicação"));
        let r = ok(&parsed, 1);
        assert_eq!((r.instrument.as_str(), r.code.as_str()), ("cdb", "CDBAAA"));
        assert_eq!((r.principal, r.gross_value, r.gross_yield), (50.0, 50.5, 0.5));
        assert_eq!(r.ir_rate, 0.225);
        assert_eq!((r.iof_amount, r.holding_days, r.iof_basis.as_str()), (0.0, Some(63), "full"), "mais de 30 dias: sem IOF");
        assert_eq!((r.ir_amount, r.net_yield), (0.11, 0.39), "0,5 × 22,5% = 0,1125 → 0,11");
        assert_eq!(r.base_price_source, "application");
        assert_eq!(r.transaction_date, "2026-03-05");
    }

    #[test]
    fn a_short_holding_pays_iof_first_and_ir_on_what_is_left() {
        let parsed = parse_fixed_income_rows(
            &rows(&[
                &["Credito", "01/03/2026", "APLICAÇÃO", "CDB - CDBAAA", "BANCO", "10000", "0.01", "100"],
                &["Debito", "05/03/2026", "RESGATE ANTECIPADO/", "CDB - CDBAAA - BANCO", "BANCO", "5000", "0.0101", "50.50"],
            ]),
            &rules(),
        );
        let r = ok(&parsed, 1);
        // 4 dias aplicados: IOF de 86% do rendimento de 0,50 = 0,43; IR de 22,5% sobre 0,07 = 0,02.
        assert_eq!((r.holding_days, r.iof_basis.as_str()), (Some(4), "full"));
        assert_eq!((r.gross_yield, r.iof_amount, r.ir_amount, r.net_yield), (0.5, 0.43, 0.02, 0.05));
    }

    #[test]
    fn redemptions_consume_the_oldest_lots_first_and_the_iof_is_weighted() {
        let parsed = parse_fixed_income_rows(
            &rows(&[
                &["Credito", "01/03/2026", "APLICAÇÃO", "CDB - CDBAAA", "BANCO", "10000", "0.01", "100"],
                &["Credito", "11/03/2026", "APLICAÇÃO", "CDB - CDBAAA", "BANCO", "10000", "0.01", "100"],
                &["Debito", "21/03/2026", "RESGATE ANTECIPADO/", "CDB - CDBAAA - BANCO", "BANCO", "15000", "0.0104", "156"],
            ]),
            &rules(),
        );
        let r = ok(&parsed, 2);
        // 10000 cotas do lote de 20 dias (IOF 33%) e 5000 do de 10 dias (66%) sobre o rendimento bruto de 6,00.
        assert_eq!((r.principal, r.gross_yield), (150.0, 6.0));
        assert_eq!((r.holding_days, r.iof_basis.as_str()), (Some(17), "full"));
        assert_eq!(r.iof_amount, 2.64, "6,00 × (2/3 × 33% + 1/3 × 66%)");
        assert_eq!((r.ir_amount, r.net_yield), (0.76, 2.6), "IR de 22,5% sobre 3,36");
    }

    #[test]
    fn units_without_an_application_in_the_file_get_no_iof_and_are_marked() {
        let partial = parse_fixed_income_rows(
            &rows(&[
                &["Credito", "01/03/2026", "APLICAÇÃO", "CDB - CDBAAA", "BANCO", "2000", "0.01", "20"],
                &["Debito", "05/03/2026", "RESGATE", "CDB - CDBAAA - BANCO", "BANCO", "4000", "0.0110", "44"],
            ]),
            &rules(),
        );
        let r = ok(&partial, 1);
        assert_eq!((r.iof_basis.as_str(), r.holding_days), ("partial", Some(4)));
        assert_eq!(r.gross_yield, 4.0);
        assert_eq!(r.iof_amount, 1.72, "só a metade com aplicação conhecida: 4,00 × 1/2 × 86%");

        let unknown = parse_fixed_income_rows(
            &rows(&[
                &["Credito", "01/03/2026", "APLICAÇÃO", "CDB - CDBAAA", "BANCO", "2000", "0.01", "20"],
                &["Debito", "05/03/2026", "RESGATE", "CDB - CDBBBB - BANCO", "BANCO", "1000", "0.0110", "11"],
            ]),
            &rules(),
        );
        let r = ok(&unknown, 1);
        assert_eq!((r.iof_basis.as_str(), r.holding_days, r.iof_amount), ("unknown", None, 0.0));
    }

    #[test]
    fn exempt_instruments_never_get_iof() {
        let parsed = parse_fixed_income_rows(
            &rows(&[
                &["Credito", "01/03/2026", "APLICAÇÃO", "LCI - LCIAAA", "BANCO", "1000", "0.01", "10"],
                &["Debito", "05/03/2026", "RESGATE", "LCI - LCIAAA - BANCO", "BANCO", "1000", "0.0110", "11"],
            ]),
            &rules(),
        );
        let r = ok(&parsed, 1);
        assert_eq!((r.iof_amount, r.iof_basis.as_str(), r.net_yield), (0.0, "not_applicable", 1.0));
    }

    #[test]
    fn a_code_without_its_application_uses_the_common_price_of_the_file() {
        let parsed = parse_fixed_income_rows(
            &rows(&[
                &["Credito", "01/03/2026", "APLICAÇÃO", "CDB - CDBAAA", "BANCO", "10000", "0.01", "100"],
                &["Debito", "05/03/2026", "RESGATE ANTECIPADO/", "CDB - CDBBBB - BANCO", "BANCO", "2000", "0.0102", "20.40"],
            ]),
            &rules(),
        );
        let r = ok(&parsed, 1);
        assert_eq!((r.principal, r.gross_yield, r.base_price_source.as_str()), (20.0, 0.4, "file-wide"));
    }

    #[test]
    fn different_application_prices_make_an_unknown_code_unresolvable() {
        let parsed = parse_fixed_income_rows(
            &rows(&[
                &["Credito", "01/03/2026", "APLICAÇÃO", "CDB - CDBAAA", "BANCO", "10000", "0.01", "100"],
                &["Credito", "01/03/2026", "APLICAÇÃO", "CDB - CDBCCC", "BANCO", "1", "1000", "1000"],
                &["Debito", "05/03/2026", "RESGATE ANTECIPADO/", "CDB - CDBBBB - BANCO", "BANCO", "2000", "0.0102", "20.40"],
            ]),
            &rules(),
        );
        assert!(parsed[2].as_ref().unwrap_err().message.contains("preço-base"));
    }

    #[test]
    fn exempt_instruments_pay_no_ir() {
        let parsed = parse_fixed_income_rows(
            &rows(&[
                &["Credito", "01/03/2026", "APLICAÇÃO", "LCI - LCIAAA", "BANCO", "1000", "0.01", "10"],
                &["Debito", "09/03/2026", "RESGATE", "LCI - LCIAAA - BANCO", "BANCO", "1000", "0.0102", "10.20"],
            ]),
            &rules(),
        );
        let r = ok(&parsed, 1);
        assert_eq!((r.instrument.as_str(), r.ir_rate, r.ir_amount, r.net_yield), ("lci", 0.0, 0.0, 0.2));
    }

    #[test]
    fn rows_that_cannot_be_a_yield_are_skipped_with_a_reason() {
        let parsed = parse_fixed_income_rows(
            &rows(&[
                &["Credito", "01/03/2026", "APLICAÇÃO", "CDB - CDBAAA", "BANCO", "1000", "0.01", "10"],
                &["Debito", "05/03/2026", "RESGATE ANTECIPADO/", "CDB - CDBAAA - BANCO", "BANCO", "1000", "0.01", "10"],
                &["Credito", "05/03/2026", "Dividendo", "ABCD3 - EMPRESA A", "CORRETORA", "100", "0.05", "5"],
                &["Credito", "05/03/2026", "Juros", "CDB - CDBAAA - BANCO", "BANCO", "1", "-", "1"],
                &["Debito", "05/03/2026", "RESGATE", "DEB - DEBAAA", "BANCO", "1", "1", "1"],
            ]),
            &rules(),
        );
        let reasons: Vec<&str> = parsed.iter().skip(1).map(|r| r.as_ref().unwrap_err().message.as_str()).collect();
        assert!(reasons[0].contains("sem rendimento"));
        assert!(reasons[1].contains("não é renda fixa"));
        assert!(reasons[2].contains("não suportada"));
        assert!(reasons[3].contains("não é renda fixa"), "papel desconhecido não vira rendimento");
    }

    #[test]
    fn a_missing_tax_rule_is_an_error_not_a_zero_rate() {
        let parsed = parse_fixed_income_rows(
            &rows(&[
                &["Credito", "01/03/2026", "APLICAÇÃO", "CDB - CDBAAA", "BANCO", "1000", "0.01", "10"],
                &["Debito", "05/03/2026", "RESGATE", "CDB - CDBAAA - BANCO", "BANCO", "1000", "0.0102", "10.20"],
            ]),
            &RuleSet::new(vec![]),
        );
        assert!(parsed[1].as_ref().unwrap_err().message.contains("fixed_income_ir_cdb"));
    }

    #[test]
    fn redemptions_of_the_same_day_share_the_derived_total() {
        let parsed = parse_fixed_income_rows(
            &rows(&[
                &["Credito", "01/01/2026", "APLICAÇÃO", "CDB - CDBAAA", "BANCO", "10000", "0.01", "100"],
                &["Debito", "05/03/2026", "RESGATE", "CDB - CDBAAA - BANCO", "BANCO", "1000", "0.0104", "10.40"],
                &["Debito", "05/03/2026", "RESGATE", "CDB - CDBAAA - BANCO", "BANCO", "2000", "0.0102", "20.40"],
                &["Debito", "06/03/2026", "RESGATE", "CDB - CDBAAA - BANCO", "BANCO", "1000", "0.0104", "10.40"],
            ]),
            &rules(),
        );
        // Rendimento bruto 0,40 → IR 0,09 → líquido 0,31 (nos dois primeiros: 40 centavos cada).
        // Dia 05: (10 + 0,31) + (20 + 0,31) = 30,62; dia 06: 10 + 0,31 = 10,31.
        assert_eq!(ok(&parsed, 1).day_derived_total, 30.62);
        assert_eq!(ok(&parsed, 2).day_derived_total, 30.62);
        assert_eq!(ok(&parsed, 3).day_derived_total, 10.31);
    }

    #[test]
    fn a_sheet_without_the_columns_is_rejected_whole() {
        let parsed = parse_fixed_income_rows(&[vec!["a".to_string()], vec!["b".to_string()]], &rules());
        assert_eq!(parsed.len(), 1);
        assert!(parsed[0].as_ref().unwrap_err().message.contains("Movimentação"));
    }

    /// Roda o leitor na Movimentação real (`ANCHOR_B3_MOVIMENTACAO`) com as taxas de
    /// fábrica. Só imprime contagens e faixas, nunca códigos ou valores.
    #[test]
    #[ignore]
    fn real_movimentacao_sample_yields() {
        let path = std::env::var("ANCHOR_B3_MOVIMENTACAO").expect("defina ANCHOR_B3_MOVIMENTACAO");
        let d = NaiveDate::from_ymd_opt(2005, 1, 1).unwrap();
        let factory = RuleSet::new(vec![Rule { key: "fixed_income_ir_cdb".to_string(), effective_from: d, value: 0.225 }]);
        let parsed = parse_fixed_income_rows(&read_xlsx_rows(&path).expect("planilha abre"), &factory);
        let good: Vec<&ProposedYield> = parsed.iter().filter_map(|r| r.as_ref().ok()).collect();
        println!("{} rendimentos derivados, {} linhas puladas", good.len(), parsed.len() - good.len());
        let mut reasons: BTreeMap<String, usize> = BTreeMap::new();
        for issue in parsed.iter().filter_map(|r| r.as_ref().err()) {
            *reasons.entry(issue.message.clone()).or_insert(0) += 1;
        }
        for (reason, count) in reasons {
            println!("{count}x {reason}");
        }
        let sources: BTreeMap<&str, usize> = good.iter().fold(BTreeMap::new(), |mut acc, p| {
            *acc.entry(p.base_price_source.as_str()).or_insert(0) += 1;
            acc
        });
        println!("origem do preço-base: {sources:?}");

        // Se o extrato do mesmo período vier em `ANCHOR_INTER_CSV`, confere o líquido previsto
        // (principal + rendimento depois de IOF e IR) com o resgate mais próximo do extrato, em até
        // 3 dias. Só vale para quem tem a aplicação no arquivo (`full`): o resto não tem IOF.
        if let Ok(statement_path) = std::env::var("ANCHOR_INTER_CSV") {
            use crate::commands::statement_io::{decode_text, parse_inter_csv};
            let statement = parse_inter_csv(&decode_text(&std::fs::read(statement_path).expect("extrato abre")));
            let redemptions: Vec<(NaiveDate, f64)> = statement
                .rows
                .iter()
                .filter_map(|r| r.as_ref().ok())
                .filter(|p| p.porquinho && p.valor > 0.0)
                .filter_map(|p| Some((NaiveDate::parse_from_str(&p.transaction_date, "%Y-%m-%d").ok()?, p.valor)))
                .collect();
            let mut errors: Vec<f64> = Vec::new();
            for p in good.iter().filter(|p| p.iof_basis == "full") {
                let day = NaiveDate::parse_from_str(&p.transaction_date, "%Y-%m-%d").unwrap();
                let predicted = p.principal + p.net_yield;
                if let Some(error) = redemptions
                    .iter()
                    .filter(|(d, _)| (*d - day).num_days().abs() <= 3)
                    .map(|(_, v)| (v - predicted).abs())
                    .reduce(f64::min)
                {
                    errors.push(error);
                }
            }
            errors.sort_by(|a, b| a.partial_cmp(b).unwrap());
            if !errors.is_empty() {
                println!(
                    "conferência com o extrato: {} resgates com aplicação conhecida | dentro de R$ 0,01: {} | dentro de R$ 0,03: {} | erro mediano R$ {:.3}",
                    errors.len(),
                    errors.iter().filter(|e| **e <= 0.0101).count(),
                    errors.iter().filter(|e| **e <= 0.0301).count(),
                    errors[errors.len() / 2],
                );
            }
        }
        if let (Some(min), Some(max)) = (
            good.iter().map(|p| p.gross_yield / p.principal * 100.0).reduce(f64::min),
            good.iter().map(|p| p.gross_yield / p.principal * 100.0).reduce(f64::max),
        ) {
            println!("rendimento bruto sobre o principal: {min:.2}% a {max:.2}%");
        }
    }
}
