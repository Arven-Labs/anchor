// Fase 35, entrega 3 — extrato bancário (OFX e CSV do Inter) pela fila de revisão.
// Os dois formatos viram `ProposedBankTransaction`; `stage_statement` guarda o que é
// novo em `import_pending` (`kind = bank_transaction`) e o popup de Finances aprova,
// edita ou descarta item a item. Aplicação/Resgate do porquinho viram o par de
// transferência entre a conta e uma conta "Porquinho"; o resto é receita ou despesa
// pelo sinal do valor.
//
// A duplicata não usa o `FITID` do OFX (o CSV não tem): é (conta, data, valor com
// sinal) contada com multiplicidade, como os negócios da carteira. Assim o OFX e o
// CSV do mesmo período não se duplicam, e um lançamento digitado à mão igual conta
// como já existente.
use std::collections::{HashMap, HashSet};

use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};

use crate::commands::bank_account::{create_bank_account_in, signed_valor, CreateBankAccountRequest};
use crate::commands::general_transaction::{
    create_general_transaction_in, create_transfer_in, CreateGeneralTransactionRequest, CreateTransferRequest,
};
use crate::commands::fixed_income_io::{bank_totals_by_day, YieldPayload, KIND_YIELD};
use crate::commands::loan::ensure_account_in_workspace;
use crate::commands::porquinho_estimate::{enable_auto_yield_if_unset, run_auto_estimates};
use crate::commands::portfolio_io::{
    normalize_header, parse_date, parse_number, ParseIssue, STATUS_CONFIRMED, STATUS_DISCARDED, STATUS_PENDING,
};
use crate::entity::{bank_account, general_transaction, import_pending};
use crate::error::AppError;

pub const KIND_BANK: &str = "bank_transaction";
const PORQUINHO_ACCOUNT_NAME: &str = "Cdb Porquinho";

/// Um lançamento do extrato, ainda não gravado. `valor` tem sinal (saída negativa).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProposedBankTransaction {
    /// Linha de origem (1 = primeiro lançamento), pra mensagens de erro.
    pub line: usize,
    pub transaction_date: String,
    pub valor: f64,
    /// Histórico do banco (`Pix recebido`, `Aplicação`…).
    pub label: String,
    /// Contraparte ou detalhe.
    pub description: String,
    /// `FITID` do OFX; o CSV não tem.
    pub reference: Option<String>,
    /// Aplicação ou resgate do CDB Porquinho.
    pub porquinho: bool,
    /// `Transferência enviada/recebida` do banco: candidata a aporte ou saque da conta Corretora. Só vira
    /// transferência quando o import recebe uma conta Corretora; sem ela, segue como receita ou despesa.
    #[serde(default)]
    pub broker: bool,
}

pub type BankParsed = Vec<Result<ProposedBankTransaction, ParseIssue>>;

pub struct ParsedStatement {
    pub rows: BankParsed,
    /// Número da conta no arquivo (`ACCTID` / linha `Conta`), só informativo.
    pub account_ref: Option<String>,
}

// ---------------------------------------------------------------------------
// Leitura
// ---------------------------------------------------------------------------

/// cp1252 nos bytes 0x80–0x9F (o resto é latin-1). Os indefinidos viram U+FFFD.
const CP1252_HIGH: [char; 32] = [
    '€', '\u{FFFD}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{FFFD}', 'Ž', '\u{FFFD}', '\u{FFFD}',
    '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{FFFD}', 'ž', 'Ÿ',
];

/// UTF-8 quando o arquivo é UTF-8 válido; senão cp1252 (o OFX do Inter declara
/// `CHARSET:1252`). Texto só ASCII dá o mesmo nos dois.
pub fn decode_text(bytes: &[u8]) -> String {
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.trim_start_matches('\u{feff}').to_string();
    }
    bytes
        .iter()
        .map(|&b| match b {
            0x80..=0x9F => CP1252_HIGH[(b - 0x80) as usize],
            _ => char::from(b),
        })
        .collect()
}

fn is_porquinho(label: &str, text: &str) -> bool {
    let label = normalize_header(label);
    (label.starts_with("aplicacao") || label.starts_with("resgate")) && normalize_header(text).contains("porquinho")
}

fn is_broker_transfer(label: &str) -> bool {
    let label = normalize_header(label);
    label.starts_with("transferencia enviada") || label.starts_with("transferencia recebida")
}

/// Valor de uma tag do OFX/SGML: do `>` até o próximo `<` ou quebra de linha.
fn ofx_tag(block: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let start = block.find(&open)? + open.len();
    let rest = &block[start..];
    let end = rest.find(['<', '\n', '\r']).unwrap_or(rest.len());
    let value = rest[..end].trim();
    (!value.is_empty()).then(|| value.to_string())
}

/// `AAAAMMDD…` → `AAAA-MM-DD`.
fn ofx_date(raw: &str) -> Option<String> {
    let digits: String = raw.chars().take(8).collect();
    if digits.len() != 8 || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    parse_date(&format!("{}-{}-{}", &digits[0..4], &digits[4..6], &digits[6..8]))
}

/// OFX 1.x (SGML, campos sem tag de fechamento). Cada `<STMTTRN>` é um lançamento.
pub fn parse_ofx(text: &str) -> ParsedStatement {
    let account_ref = ofx_tag(text, "ACCTID");
    let mut rows: BankParsed = Vec::new();
    let mut rest = text;
    let mut line = 0;
    while let Some(index) = rest.find("<STMTTRN>") {
        let after = &rest[index + "<STMTTRN>".len()..];
        let end = after.find("</STMTTRN>").or_else(|| after.find("<STMTTRN>")).unwrap_or(after.len());
        let block = &after[..end];
        rest = &after[end..];
        line += 1;
        rows.push(parse_ofx_block(line, block));
    }
    if rows.is_empty() {
        rows.push(Err(ParseIssue { line: 0, message: "não achei nenhum lançamento (<STMTTRN>) no OFX".to_string() }));
    }
    ParsedStatement { rows, account_ref }
}

fn parse_ofx_block(line: usize, block: &str) -> Result<ProposedBankTransaction, ParseIssue> {
    let issue = |message: String| ParseIssue { line, message };
    let raw_date = ofx_tag(block, "DTPOSTED").unwrap_or_default();
    let transaction_date = ofx_date(&raw_date).ok_or_else(|| issue(format!("data inválida: '{raw_date}'")))?;
    let raw_amount = ofx_tag(block, "TRNAMT").unwrap_or_default();
    let valor = parse_number(&raw_amount)
        .map_err(issue)?
        .filter(|v| v.is_finite() && *v != 0.0)
        .ok_or_else(|| issue(format!("valor inválido ou zero: '{raw_amount}'")))?;
    let memo = ofx_tag(block, "MEMO").unwrap_or_default();
    let name = ofx_tag(block, "NAME").unwrap_or_default();
    let (label, detail) = match memo.split_once(':') {
        Some((head, tail)) => (head.trim().to_string(), tail.trim().to_string()),
        None => (memo.clone(), String::new()),
    };
    let label = if label.is_empty() { ofx_tag(block, "TRNTYPE").unwrap_or_default() } else { label };
    let description = if name.is_empty() { detail } else { name.clone() };
    let porquinho = is_porquinho(&label, &format!("{memo} {name}"));
    let broker = is_broker_transfer(&label);
    Ok(ProposedBankTransaction {
        line,
        transaction_date,
        valor,
        label,
        description,
        reference: ofx_tag(block, "FITID"),
        porquinho,
        broker,
    })
}

/// CSV da conta corrente do Inter: um bloco de cabeçalho (`Conta;…`, `Período;…`),
/// a linha `Data Lançamento;Histórico;Descrição;Valor;Saldo` e os lançamentos.
/// A coluna `Saldo` não é confiável e é ignorada.
pub fn parse_inter_csv(text: &str) -> ParsedStatement {
    let lines: Vec<&str> = text.lines().collect();
    let header = lines.iter().position(|l| {
        let n = normalize_header(l);
        n.contains("data lancamento") && n.contains("historico")
    });
    let Some(header) = header else {
        return ParsedStatement {
            rows: vec![Err(ParseIssue {
                line: 0,
                message: "não parece o extrato CSV do Inter: falta a linha 'Data Lançamento;Histórico;…'".to_string(),
            })],
            account_ref: None,
        };
    };
    let account_ref = lines[..header]
        .iter()
        .find(|l| normalize_header(l).starts_with("conta"))
        .and_then(|l| l.split(';').nth(1))
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());

    let body = lines[header + 1..].join("\n");
    let mut reader = csv::ReaderBuilder::new().delimiter(b';').has_headers(false).flexible(true).from_reader(body.as_bytes());
    let mut rows: BankParsed = Vec::new();
    for (index, record) in reader.records().enumerate() {
        let line = index + 1;
        let issue = |message: String| ParseIssue { line, message };
        let record = match record {
            Ok(r) => r,
            Err(err) => {
                rows.push(Err(issue(format!("linha ilegível: {err}"))));
                continue;
            }
        };
        if record.iter().all(|c| c.trim().is_empty()) {
            continue;
        }
        let cell = |i: usize| record.get(i).unwrap_or("").trim().to_string();
        let Some(transaction_date) = parse_date(&cell(0)) else {
            rows.push(Err(issue(format!("data inválida: '{}'", cell(0)))));
            continue;
        };
        let valor = match parse_number(&cell(3)) {
            Ok(Some(v)) if v.is_finite() && v != 0.0 => v,
            Ok(_) => {
                rows.push(Err(issue(format!("valor inválido ou zero: '{}'", cell(3)))));
                continue;
            }
            Err(message) => {
                rows.push(Err(issue(message)));
                continue;
            }
        };
        let (label, description) = (cell(1), cell(2));
        let porquinho = is_porquinho(&label, &description);
        let broker = is_broker_transfer(&label);
        rows.push(Ok(ProposedBankTransaction { line, transaction_date, valor, label, description, reference: None, porquinho, broker }));
    }
    ParsedStatement { rows, account_ref }
}

fn parse_statement_file(path: &str, format: &str) -> Result<ParsedStatement, AppError> {
    let bytes = std::fs::read(path).map_err(|e| AppError::InvalidInput(format!("não consegui ler {path}: {e}")))?;
    let text = decode_text(&bytes);
    match format {
        "inter_ofx" => Ok(parse_ofx(&text)),
        "inter_csv" => Ok(parse_inter_csv(&text)),
        other => Err(AppError::InvalidInput(format!("formato '{other}' desconhecido"))),
    }
}

// ---------------------------------------------------------------------------
// Fila
// ---------------------------------------------------------------------------

type Key = (String, i64);

pub(crate) fn dedup_key(date: &str, valor: f64) -> Key {
    (date.to_string(), (valor * 100.0).round() as i64)
}

/// Lançamentos já gravados na conta e itens da fila (pendentes e descartados),
/// contados por (data, valor com sinal).
async fn known_keys<C: ConnectionTrait>(db: &C, bank_account_id: i32) -> Result<HashMap<Key, usize>, AppError> {
    let mut keys: HashMap<Key, usize> = HashMap::new();
    let ledger = general_transaction::Entity::find()
        .filter(general_transaction::Column::BankAccountId.eq(bank_account_id))
        .all(db)
        .await?;
    for tx in ledger {
        *keys.entry(dedup_key(&tx.transaction_date, signed_valor(&tx.transaction_type, tx.valor))).or_insert(0) += 1;
    }
    let queued = import_pending::Entity::find()
        .filter(import_pending::Column::BankAccountId.eq(bank_account_id))
        .filter(import_pending::Column::Kind.eq(KIND_BANK))
        .all(db)
        .await?;
    for row in queued {
        let Ok(payload) = serde_json::from_str::<BankPayload>(&row.payload) else { continue };
        *keys.entry(dedup_key(&payload.proposed.transaction_date, payload.proposed.valor)).or_insert(0) += 1;
    }
    Ok(keys)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BankPayload {
    pub proposed: ProposedBankTransaction,
}

#[derive(Serialize)]
pub struct StatementStageResult {
    pub staged: usize,
    pub duplicates: usize,
    /// Linhas que ficaram fora da fila, cada uma com o motivo.
    pub skipped: Vec<ParseIssue>,
    /// Conta que o arquivo declara, só informativa.
    pub account_ref: Option<String>,
}

pub async fn stage_statement_parsed(
    db: &DatabaseConnection,
    workspace_id: i32,
    bank_account_id: i32,
    porquinho_account_id: Option<i32>,
    create_porquinho: bool,
    source: &str,
    parsed: ParsedStatement,
) -> Result<StatementStageResult, AppError> {
    stage_statement_with_broker(db, workspace_id, bank_account_id, porquinho_account_id, create_porquinho, None, source, parsed).await
}

/// Como `stage_statement_parsed`, com uma conta Corretora opcional: as `Transferência enviada/recebida`
/// do extrato viram aporte ou saque dela (transferência entre as duas contas) em vez de despesa ou receita.
#[allow(clippy::too_many_arguments)]
pub async fn stage_statement_with_broker(
    db: &DatabaseConnection,
    workspace_id: i32,
    bank_account_id: i32,
    porquinho_account_id: Option<i32>,
    create_porquinho: bool,
    broker_account_id: Option<i32>,
    source: &str,
    parsed: ParsedStatement,
) -> Result<StatementStageResult, AppError> {
    let account = ensure_account_in_workspace(db, bank_account_id, workspace_id).await?;
    if let Some(id) = broker_account_id {
        if id == bank_account_id || Some(id) == porquinho_account_id {
            return Err(AppError::InvalidInput("a conta Corretora tem de ser outra conta".to_string()));
        }
        ensure_account_in_workspace(db, id, workspace_id).await?;
    }
    let mut counterpart = porquinho_account_id;
    if let Some(id) = counterpart {
        if id == bank_account_id {
            return Err(AppError::InvalidInput("a conta Porquinho tem de ser outra conta".to_string()));
        }
        ensure_account_in_workspace(db, id, workspace_id).await?;
    }
    let needs_porquinho = parsed.rows.iter().any(|r| matches!(r, Ok(p) if p.porquinho));
    if counterpart.is_none() && create_porquinho && needs_porquinho {
        let created = create_bank_account_in(
            db,
            CreateBankAccountRequest {
                workspace_id,
                nome: PORQUINHO_ACCOUNT_NAME.to_string(),
                titular: account.titular.clone(),
                currency: Some(account.currency.clone()),
            },
        )
        .await?;
        counterpart = Some(created.id);
    }

    let mut remaining = known_keys(db, bank_account_id).await?;
    let now = chrono::Utc::now().to_rfc3339();
    let txn = db.begin().await?;
    let mut result = StatementStageResult { staged: 0, duplicates: 0, skipped: Vec::new(), account_ref: parsed.account_ref };
    for row in parsed.rows {
        let proposed = match row {
            Ok(p) => p,
            Err(issue) => {
                result.skipped.push(issue);
                continue;
            }
        };
        if proposed.porquinho && counterpart.is_none() {
            result.skipped.push(ParseIssue {
                line: proposed.line,
                message: "aplicação/resgate do porquinho pulado: escolha ou crie uma conta Porquinho".to_string(),
            });
            continue;
        }
        let key = dedup_key(&proposed.transaction_date, proposed.valor);
        if let Some(count) = remaining.get_mut(&key).filter(|c| **c > 0) {
            *count -= 1;
            result.duplicates += 1;
            continue;
        }
        let payload = serde_json::to_string(&BankPayload { proposed: proposed.clone() })
            .map_err(|e| AppError::InvalidInput(e.to_string()))?;
        import_pending::ActiveModel {
            workspace_id: Set(workspace_id),
            bank_account_id: Set(Some(bank_account_id)),
            counterpart_account_id: Set(if proposed.porquinho {
                counterpart
            } else if proposed.broker {
                broker_account_id
            } else {
                None
            }),
            kind: Set(KIND_BANK.to_string()),
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

pub async fn stage_statement_in(
    db: &DatabaseConnection,
    workspace_id: i32,
    bank_account_id: i32,
    porquinho_account_id: Option<i32>,
    create_porquinho: bool,
    broker_account_id: Option<i32>,
    path: &str,
    format: &str,
) -> Result<StatementStageResult, AppError> {
    let parsed = parse_statement_file(path, format)?;
    stage_statement_with_broker(
        db,
        workspace_id,
        bank_account_id,
        porquinho_account_id,
        create_porquinho,
        broker_account_id,
        format,
        parsed,
    )
    .await
}

#[derive(Serialize)]
pub struct StatementPendingView {
    pub id: i32,
    pub bank_account_id: i32,
    pub bank_account_name: String,
    pub counterpart_account_id: Option<i32>,
    pub counterpart_account_name: Option<String>,
    pub source: String,
    pub created_at: String,
    /// `bank_transaction` ou `fixed_income_yield`.
    pub kind: String,
    /// Num rendimento, uma linha sintética (valor = bruto, rótulo = papel) pra tabela da fila.
    pub proposed: ProposedBankTransaction,
    /// `receita`, `despesa` ou `transferencia` (par com o porquinho); `rendimento` nos de renda fixa.
    pub suggested_type: String,
    pub fixed_income: Option<YieldDetail>,
}

/// Detalhe de um rendimento derivado da B3, com a checagem contra o extrato do dia.
#[derive(Serialize)]
pub struct YieldDetail {
    pub instrument: String,
    pub code: String,
    pub quantity: f64,
    pub principal: f64,
    pub gross_value: f64,
    pub gross_yield: f64,
    pub iof_amount: f64,
    pub holding_days: Option<u32>,
    /// `full`, `partial`, `unknown` ou `not_applicable`: de onde veio o IOF.
    pub iof_basis: String,
    pub ir_rate: f64,
    pub ir_amount: f64,
    pub net_yield: f64,
    pub base_unit_price: f64,
    pub base_price_source: String,
    /// O que os resgates do dia no arquivo da B3 explicam (principal + líquido).
    pub day_derived_total: f64,
    /// O que o extrato já aprovado mostra saindo da conta Porquinho no dia; `None` sem extrato.
    pub bank_total_that_day: Option<f64>,
    /// Só nas estimativas do porquinho (`base_price_source = "estimate"`): o percentual do CDI usado.
    pub estimate_pct_of_cdi: Option<f64>,
}

pub async fn list_statement_pending_in(
    db: &DatabaseConnection,
    workspace_id: i32,
) -> Result<Vec<StatementPendingView>, AppError> {
    let rows = import_pending::Entity::find()
        .filter(import_pending::Column::WorkspaceId.eq(workspace_id))
        .filter(import_pending::Column::Kind.is_in([KIND_BANK, KIND_YIELD]))
        .filter(import_pending::Column::Status.eq(STATUS_PENDING))
        .all(db)
        .await?;
    let yield_accounts: HashSet<i32> =
        rows.iter().filter(|r| r.kind == KIND_YIELD).filter_map(|r| r.bank_account_id).collect();
    let bank_totals = bank_totals_by_day(db, &yield_accounts).await?;
    let names: HashMap<i32, String> = bank_account::Entity::find()
        .filter(bank_account::Column::WorkspaceId.eq(workspace_id))
        .all(db)
        .await?
        .into_iter()
        .map(|a| (a.id, a.nome))
        .collect();
    let mut items: Vec<StatementPendingView> = rows
        .into_iter()
        .filter_map(|row| {
            let account_id = row.bank_account_id?;
            if row.kind == KIND_YIELD {
                let p = serde_json::from_str::<YieldPayload>(&row.payload).ok()?.proposed;
                let bank_total = bank_totals.get(&(account_id, p.transaction_date.clone())).copied();
                return Some(StatementPendingView {
                    id: row.id,
                    bank_account_id: account_id,
                    bank_account_name: names.get(&account_id).cloned().unwrap_or_default(),
                    counterpart_account_id: None,
                    counterpart_account_name: None,
                    source: row.source,
                    created_at: row.created_at,
                    kind: row.kind,
                    proposed: ProposedBankTransaction {
                        line: p.line,
                        transaction_date: p.transaction_date.clone(),
                        valor: p.gross_yield,
                        label: "Yield".to_string(),
                        description: format!("{} {}", p.instrument.to_uppercase(), p.code),
                        reference: None,
                        porquinho: false,
                        broker: false,
                    },
                    suggested_type: "rendimento".to_string(),
                    fixed_income: Some(YieldDetail {
                        instrument: p.instrument,
                        code: p.code,
                        quantity: p.quantity,
                        principal: p.principal,
                        gross_value: p.gross_value,
                        gross_yield: p.gross_yield,
                        iof_amount: p.iof_amount,
                        holding_days: p.holding_days,
                        iof_basis: p.iof_basis,
                        ir_rate: p.ir_rate,
                        ir_amount: p.ir_amount,
                        net_yield: p.net_yield,
                        base_unit_price: p.base_unit_price,
                        base_price_source: p.base_price_source,
                        day_derived_total: p.day_derived_total,
                        bank_total_that_day: bank_total,
                        estimate_pct_of_cdi: p.estimate_pct_of_cdi,
                    }),
                });
            }
            let payload: BankPayload = serde_json::from_str(&row.payload).ok()?;
            let is_transfer = (payload.proposed.porquinho || payload.proposed.broker) && row.counterpart_account_id.is_some();
            let suggested_type = if is_transfer {
                "transferencia"
            } else if payload.proposed.valor < 0.0 {
                "despesa"
            } else {
                "receita"
            };
            Some(StatementPendingView {
                id: row.id,
                bank_account_id: account_id,
                bank_account_name: names.get(&account_id).cloned().unwrap_or_default(),
                counterpart_account_id: row.counterpart_account_id,
                counterpart_account_name: row.counterpart_account_id.and_then(|id| names.get(&id).cloned()),
                source: row.source,
                created_at: row.created_at,
                kind: row.kind,
                proposed: payload.proposed,
                suggested_type: suggested_type.to_string(),
                fixed_income: None,
            })
        })
        .collect();
    items.sort_by(|a, b| (&a.proposed.transaction_date, a.id).cmp(&(&b.proposed.transaction_date, b.id)));
    Ok(items)
}

/// Item aguardando revisão do Workspace, de extrato ou de rendimento.
pub(crate) async fn statement_row(db: &DatabaseConnection, workspace_id: i32, id: i32) -> Result<import_pending::Model, AppError> {
    let row = import_pending::Entity::find_by_id(id)
        .one(db)
        .await?
        .filter(|r| (r.kind == KIND_BANK || r.kind == KIND_YIELD) && r.workspace_id == workspace_id)
        .ok_or_else(|| AppError::InvalidInput(format!("pendente {id} não existe neste Workspace")))?;
    if row.status != STATUS_PENDING {
        return Err(AppError::InvalidInput(format!("pendente {id} não está aguardando revisão (status '{}')", row.status)));
    }
    Ok(row)
}

/// O que o usuário deixou na tela ao aprovar.
#[derive(Clone, Debug, Deserialize)]
pub struct EditedBankItem {
    pub transaction_date: String,
    /// `receita` ou `despesa`; ignorado numa transferência com o porquinho.
    pub transaction_type: String,
    /// Positivo; o sentido vem do tipo (ou, na transferência, do sinal original).
    pub valor: f64,
    pub notes: Option<String>,
    pub category_id: Option<i32>,
}

/// Aprova um pendente do extrato. Grava só esse item; se algo recusar, nada muda
/// e o item continua pendente.
pub async fn confirm_statement_pending_in(
    db: &DatabaseConnection,
    workspace_id: i32,
    id: i32,
    edited: EditedBankItem,
) -> Result<(), AppError> {
    let row = statement_row(db, workspace_id, id).await?;
    if row.kind != KIND_BANK {
        return Err(AppError::InvalidInput(format!("pendente {id} não é uma linha de extrato")));
    }
    let payload: BankPayload = serde_json::from_str(&row.payload)
        .map_err(|e| AppError::InvalidInput(format!("pendente {id} ilegível: {e}")))?;
    let bank_account_id = row
        .bank_account_id
        .ok_or_else(|| AppError::InvalidInput(format!("pendente {id} sem conta")))?;
    if !edited.valor.is_finite() || edited.valor <= 0.0 {
        return Err(AppError::InvalidInput("o valor tem de ser positivo".to_string()));
    }
    let transaction_date = parse_date(&edited.transaction_date)
        .ok_or_else(|| AppError::InvalidInput(format!("data inválida: '{}'", edited.transaction_date)))?;

    // Valor com sinal que vai pro ledger. Se a edição mudou data ou valor, o item
    // original do arquivo deixa de casar com o lançamento; a linha fica na fila como
    // `confirmed` só pra reimportar o mesmo extrato não trazê-lo de volta.
    let is_pair = payload.proposed.porquinho || payload.proposed.broker;
    let edited_signed = match (is_pair, row.counterpart_account_id) {
        (true, Some(_)) => edited.valor.copysign(payload.proposed.valor),
        _ => signed_valor(&edited.transaction_type, edited.valor),
    };
    let key_changed = dedup_key(&transaction_date, edited_signed)
        != dedup_key(&payload.proposed.transaction_date, payload.proposed.valor);

    match (is_pair, row.counterpart_account_id) {
        (true, Some(counterpart)) => {
            // Saída da conta = aplicação (conta → porquinho) ou aporte (conta → corretora); entrada = resgate ou saque.
            let (from, to) =
                if payload.proposed.valor < 0.0 { (bank_account_id, counterpart) } else { (counterpart, bank_account_id) };
            create_transfer_in(
                db,
                CreateTransferRequest {
                    workspace_id,
                    from_account_id: from,
                    to_account_id: to,
                    valor_saida: edited.valor,
                    valor_entrada: edited.valor,
                    transaction_date,
                    notes: edited.notes,
                },
            )
            .await?;
            finish_pending(db, row, key_changed).await?;
            // Com o porquinho como contraparte, liga a estimativa automática do rendimento dela.
            if payload.proposed.porquinho {
                enable_auto_yield_if_unset(db, counterpart).await?;
            }
        }
        _ => {
            let txn = db.begin().await?;
            create_general_transaction_in(
                &txn,
                CreateGeneralTransactionRequest {
                    workspace_id,
                    bank_account_id,
                    transaction_type: edited.transaction_type,
                    category_id: edited.category_id,
                    valor: edited.valor,
                    transaction_date,
                    notes: edited.notes,
                },
            )
            .await?;
            finish_pending(&txn, row, key_changed).await?;
            txn.commit().await?;
        }
    }
    // Se foi o último item da revisão, estima o rendimento do porquinho sozinho (melhor esforço, nunca falha a aprovação).
    run_auto_estimates(db, workspace_id).await;
    Ok(())
}

/// Tira o item aprovado da fila, ou o guarda como `confirmed` quando a edição mudou
/// a chave de duplicata (ver `confirm_statement_pending_in`).
pub(crate) async fn finish_pending<C: ConnectionTrait>(db: &C, row: import_pending::Model, keep_marker: bool) -> Result<(), AppError> {
    if keep_marker {
        let mut active: import_pending::ActiveModel = row.into();
        active.status = Set(STATUS_CONFIRMED.to_string());
        active.update(db).await?;
    } else {
        import_pending::Entity::delete_by_id(row.id).exec(db).await?;
    }
    Ok(())
}

pub async fn discard_statement_pending_in(db: &DatabaseConnection, workspace_id: i32, id: i32) -> Result<(), AppError> {
    let row = statement_row(db, workspace_id, id).await?;
    let mut active: import_pending::ActiveModel = row.into();
    active.status = Set(STATUS_DISCARDED.to_string());
    active.update(db).await?;
    // Descartar o último item também fecha a revisão.
    run_auto_estimates(db, workspace_id).await;
    Ok(())
}

// ---------------------------------------------------------------------------
// Comandos
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn stage_statement(
    db: tauri::State<'_, DatabaseConnection>,
    workspace_id: i32,
    bank_account_id: i32,
    porquinho_account_id: Option<i32>,
    create_porquinho: bool,
    broker_account_id: Option<i32>,
    path: String,
    format: String,
) -> Result<StatementStageResult, AppError> {
    stage_statement_in(db.inner(), workspace_id, bank_account_id, porquinho_account_id, create_porquinho, broker_account_id, &path, &format)
        .await
}

#[tauri::command]
pub async fn list_statement_pending(
    db: tauri::State<'_, DatabaseConnection>,
    workspace_id: i32,
) -> Result<Vec<StatementPendingView>, AppError> {
    list_statement_pending_in(db.inner(), workspace_id).await
}

#[tauri::command]
pub async fn confirm_statement_pending(
    db: tauri::State<'_, DatabaseConnection>,
    workspace_id: i32,
    id: i32,
    edited: EditedBankItem,
) -> Result<(), AppError> {
    confirm_statement_pending_in(db.inner(), workspace_id, id, edited).await
}

#[tauri::command]
pub async fn discard_statement_pending(
    db: tauri::State<'_, DatabaseConnection>,
    workspace_id: i32,
    id: i32,
) -> Result<(), AppError> {
    discard_statement_pending_in(db.inner(), workspace_id, id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    // Dados inventados: nenhuma conta, nome ou valor de extrato real.
    const OFX: &str = "OFXHEADER:100\nDATA:OFXSGML\n\n<OFX>\n<BANKMSGSRSV1>\n<STMTTRNRS>\n<STMTRS>\n<CURDEF>BRL\n<BANKACCTFROM>\n<BANKID>999\n<ACCTID>12345\n</BANKACCTFROM>\n<BANKTRANLIST>\n<DTSTART>20250101\n<DTEND>20250131\n\
<STMTTRN>\n<TRNTYPE>CREDIT\n<DTPOSTED>20250115\n<TRNAMT>1234.50\n<FITID>1001\n<NAME>Fulano de Tal\n<MEMO>Pix recebido: \"Cp :1-Fulano de Tal\"\n</STMTTRN>\n\
<STMTTRN>\n<TRNTYPE>PAYMENT\n<DTPOSTED>20250116\n<TRNAMT>-20.00\n<FITID>1002\n<NAME>Loja Exemplo\n<MEMO>Compra no debito: \"Loja Exemplo\"\n</STMTTRN>\n\
<STMTTRN>\n<TRNTYPE>PAYMENT\n<DTPOSTED>20250116\n<TRNAMT>-500.00\n<FITID>1003\n<NAME>Cdb Porquinho Banco X\n<MEMO>Aplicacao: \"CDB PORQUINHO\"\n</STMTTRN>\n\
<STMTTRN>\n<TRNTYPE>CREDIT\n<DTPOSTED>20250120\n<TRNAMT>12,34\n<FITID>1004\n<NAME>Cdb Porquinho Banco X\n<MEMO>Resgate: \"CDB PORQUINHO\"\n</STMTTRN>\n\
</BANKTRANLIST>\n</STMTRS>\n</STMTTRNRS>\n</BANKMSGSRSV1>\n</OFX>\n";

    const CSV: &str = " Extrato Conta Corrente \nConta ;12345\nPeríodo ;01/01/2025 a 31/01/2025\nSaldo ;0,00\n\nData Lançamento;Histórico;Descrição;Valor;Saldo\n\
15/01/2025;Pix recebido;Fulano de Tal;1.234,50;-0,00\n\
16/01/2025;Compra no débito;Loja Exemplo;-20,00;-20,00\n\
16/01/2025;Aplicação;Cdb Porquinho Banco X;-500,00;-500,00\n\
20/01/2025;Resgate;Cdb Porquinho Banco X;12,34;0,00\n";

    fn ok(parsed: &ParsedStatement) -> Vec<&ProposedBankTransaction> {
        parsed.rows.iter().map(|r| r.as_ref().expect("linha lida")).collect()
    }

    #[test]
    fn cp1252_and_utf8_text_both_decode() {
        assert_eq!(decode_text(&[0x41, 0xE7, 0xE3, 0x6F]), "A\u{e7}\u{e3}o");
        assert_eq!(decode_text(&[0x80, 0x96]), "€–");
        assert_eq!(decode_text("Aplicação".as_bytes()), "Aplicação");
        assert_eq!(decode_text(b"\xef\xbb\xbfabc"), "abc", "BOM sai");
    }

    #[test]
    fn an_ofx_with_unclosed_sgml_tags_becomes_signed_rows() {
        let parsed = parse_ofx(OFX);
        assert_eq!(parsed.account_ref.as_deref(), Some("12345"));
        let rows = ok(&parsed);
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].transaction_date, "2025-01-15");
        assert_eq!(rows[0].valor, 1234.5);
        assert_eq!(rows[0].label, "Pix recebido");
        assert_eq!(rows[0].description, "Fulano de Tal");
        assert_eq!(rows[0].reference.as_deref(), Some("1001"));
        assert_eq!(rows[1].valor, -20.0);
        assert!(!rows[0].porquinho && !rows[1].porquinho);
        assert!(rows[2].porquinho && rows[2].valor == -500.0, "aplicação");
        assert!(rows[3].porquinho && rows[3].valor == 12.34, "resgate, vírgula decimal também");
    }

    #[test]
    fn an_ofx_with_a_bad_date_or_zero_amount_reports_that_line() {
        let parsed = parse_ofx("<STMTTRN>\n<DTPOSTED>2025\n<TRNAMT>1.00\n</STMTTRN>\n<STMTTRN>\n<DTPOSTED>20250115\n<TRNAMT>0.00\n</STMTTRN>");
        assert_eq!(parsed.rows.len(), 2);
        assert_eq!(parsed.rows[0].as_ref().unwrap_err().line, 1);
        assert!(parsed.rows[1].as_ref().unwrap_err().message.contains("zero"));
        assert!(parse_ofx("nada").rows[0].is_err(), "arquivo sem lançamento é recusado");
    }

    #[test]
    fn an_inter_csv_skips_the_header_block_and_the_balance_column() {
        let parsed = parse_inter_csv(CSV);
        assert_eq!(parsed.account_ref.as_deref(), Some("12345"));
        let rows = ok(&parsed);
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].valor, 1234.5, "ponto de milhar e vírgula decimal");
        assert_eq!(rows[1].valor, -20.0);
        assert_eq!(rows[0].reference, None);
        assert!(rows[2].porquinho && rows[3].porquinho);
        assert!(parse_inter_csv("a;b\n1;2\n").rows[0].is_err());
    }

    /// Lê os extratos reais (`ANCHOR_INTER_OFX` e `ANCHOR_INTER_CSV`). Só imprime
    /// contagens e se os dois formatos concordam; nunca nomes, contas ou valores.
    #[test]
    #[ignore]
    fn real_inter_samples_parse_and_agree() {
        let read = |var: &str| decode_text(&std::fs::read(std::env::var(var).expect(var)).expect("arquivo abre"));
        let (ofx, csv) = (parse_ofx(&read("ANCHOR_INTER_OFX")), parse_inter_csv(&read("ANCHOR_INTER_CSV")));
        for (name, parsed) in [("ofx", &ofx), ("csv", &csv)] {
            let good: Vec<_> = parsed.rows.iter().filter_map(|r| r.as_ref().ok()).collect();
            println!(
                "{name}: {} lidas, {} com erro, {} do porquinho, {} entradas, {} saídas",
                good.len(),
                parsed.rows.len() - good.len(),
                good.iter().filter(|p| p.porquinho).count(),
                good.iter().filter(|p| p.valor > 0.0).count(),
                good.iter().filter(|p| p.valor < 0.0).count(),
            );
            for issue in parsed.rows.iter().filter_map(|r| r.as_ref().err()) {
                println!("{name} linha {}: {}", issue.line, issue.message);
            }
        }
        let keys = |p: &ParsedStatement| {
            let mut keys: Vec<Key> = p.rows.iter().filter_map(|r| r.as_ref().ok()).map(|r| dedup_key(&r.transaction_date, r.valor)).collect();
            keys.sort();
            keys
        };
        println!("(data, valor) iguais nos dois formatos: {}", keys(&ofx) == keys(&csv));
        println!("conta declarada igual nos dois: {}", ofx.account_ref == csv.account_ref);
    }

    #[test]
    fn the_ofx_and_the_csv_of_the_same_statement_agree_on_date_and_value() {
        let key = |p: &ParsedStatement| {
            let mut keys: Vec<(String, i64)> =
                ok(p).iter().map(|r| dedup_key(&r.transaction_date, r.valor)).collect();
            keys.sort();
            keys
        };
        assert_eq!(key(&parse_ofx(OFX)), key(&parse_inter_csv(CSV)));
    }
}
