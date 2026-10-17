// Fase 34 — exportar e importar lançamentos da carteira. A importação tem duas
// etapas: `preview_import` lê o arquivo e devolve cada linha classificada
// (nova, duplicada, com erro, ou ativo novo que precisa de classe) sem gravar
// nada; `commit_import` grava tudo numa transação só, ou nada se alguma linha
// falhar. O núcleo (`ProposedTransaction` → `preview_rows` → `commit_rows`) não
// sabe de onde as linhas vieram: CSV do Anchor, planilha da B3 ou qualquer outra
// origem entram só como mais um leitor na frente dele.

use std::collections::HashMap;

use chrono::{Datelike, NaiveDate};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};

use crate::commands::currency::workspace_of_portfolio;
use crate::commands::transaction::{create_transaction_in, CreateTransactionRequest};
use crate::entity::{assets, custodia, transactions};
use crate::error::AppError;

const CSV_HEADER: [&str; 12] = [
    "date",
    "type",
    "ticker",
    "asset_class",
    "currency",
    "quantity",
    "unit_price",
    "total_value",
    "fee",
    "custody",
    "payment_type",
    "notes",
];

/// Classes que o importador sabe criar sozinho, com a moeda e a exposição padrão.
const CLASS_DEFAULTS: [(&str, &str, &str); 13] = [
    ("acao_br", "BRL", "BR"),
    ("fii", "BRL", "BR"),
    ("etf_br", "BRL", "BR"),
    ("bdr", "BRL", "BR"),
    ("cripto", "USD", "US"),
    ("metal", "BRL", "BR"),
    ("acao_internacional", "USD", "US"),
    ("reit", "USD", "US"),
    ("etf_us", "USD", "US"),
    ("tesouro_direto", "BRL", "BR"),
    ("renda_fixa", "BRL", "BR"),
    ("imovel", "BRL", "BR"),
    ("empresa_nao_listada", "BRL", "BR"),
];

/// Um lançamento proposto por qualquer origem (arquivo, planilha, IA no futuro),
/// ainda não gravado.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProposedTransaction {
    /// Linha de origem (1 = primeira linha de dados), pra mensagens de erro.
    pub line: usize,
    pub transaction_date: String,
    pub transaction_type: String,
    /// Vazio em aporte e retirada.
    pub ticker: String,
    pub asset_class: Option<String>,
    pub currency: Option<String>,
    pub quantity: Option<f64>,
    pub unit_price: Option<f64>,
    pub total_value: f64,
    pub fee: Option<f64>,
    pub custody: Option<String>,
    pub payment_type: Option<String>,
    pub notes: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ParseIssue {
    pub line: usize,
    pub message: String,
}

pub type Parsed = Vec<Result<ProposedTransaction, ParseIssue>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportStatus {
    New,
    Duplicate,
    Error,
    /// Ativo que ainda não existe e cuja classe não dá pra inferir.
    NeedsClass,
}

#[derive(Clone, Debug, Serialize)]
pub struct ImportRow {
    pub line: usize,
    pub proposed: Option<ProposedTransaction>,
    pub status: ImportStatus,
    pub message: Option<String>,
    pub suggested_class: Option<String>,
}

#[derive(Serialize)]
pub struct ImportPreview {
    pub rows: Vec<ImportRow>,
}

/// O que o usuário confirmou na prévia: a linha e, pra ativo novo, a classe.
#[derive(Clone, Debug, Deserialize)]
pub struct CommitItem {
    pub proposed: ProposedTransaction,
    pub asset_class: Option<String>,
}

#[derive(Serialize)]
pub struct CommitResult {
    pub created: usize,
    pub skipped_duplicates: usize,
    /// Preenchido quando alguma linha falhou: nada foi gravado.
    pub errors: Vec<ParseIssue>,
}

// ---------------------------------------------------------------------------
// Leitura de valores
// ---------------------------------------------------------------------------

/// `AAAA-MM-DD` ou `DD/MM/AAAA`.
pub fn parse_date(raw: &str) -> Option<String> {
    let raw = raw.trim();
    // O chrono aceita ano com menos de 4 dígitos ("05-01-26" vira o ano 5), então
    // a forma é conferida antes e o ano precisa ser plausível.
    if raw.len() != 10 {
        return None;
    }
    NaiveDate::parse_from_str(raw, "%Y-%m-%d")
        .or_else(|_| NaiveDate::parse_from_str(raw, "%d/%m/%Y"))
        .ok()
        .filter(|d| d.year() >= 1900)
        .map(|d| d.format("%Y-%m-%d").to_string())
}

/// Número com ponto (formato do Anchor) ou com vírgula decimal (formato BR, com
/// ou sem separador de milhar). Vazio é `None`.
pub fn parse_number(raw: &str) -> Result<Option<f64>, String> {
    let cleaned: String = raw.trim().chars().filter(|c| !c.is_whitespace() && *c != 'R' && *c != '$').collect();
    if cleaned.is_empty() || cleaned == "-" {
        return Ok(None);
    }
    let normalized = if cleaned.contains(',') {
        cleaned.replace('.', "").replace(',', ".")
    } else {
        cleaned.clone()
    };
    normalized.parse::<f64>().map(Some).map_err(|_| format!("número inválido: '{raw}'"))
}

fn non_empty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

// ---------------------------------------------------------------------------
// CSV do Anchor
// ---------------------------------------------------------------------------

pub fn parse_anchor_csv(text: &str) -> Parsed {
    let mut reader = csv::ReaderBuilder::new().flexible(true).trim(csv::Trim::All).from_reader(text.as_bytes());
    let headers: HashMap<String, usize> = match reader.headers() {
        Ok(h) => h.iter().enumerate().map(|(i, name)| (name.to_lowercase(), i)).collect(),
        Err(err) => return vec![Err(ParseIssue { line: 0, message: format!("CSV ilegível: {err}") })],
    };
    for required in ["date", "type", "total_value"] {
        if !headers.contains_key(required) {
            return vec![Err(ParseIssue { line: 0, message: format!("coluna '{required}' ausente no cabeçalho") })];
        }
    }

    let mut out = Vec::new();
    for (index, record) in reader.records().enumerate() {
        let line = index + 1;
        let record = match record {
            Ok(r) => r,
            Err(err) => {
                out.push(Err(ParseIssue { line, message: format!("linha ilegível: {err}") }));
                continue;
            }
        };
        let cell = |name: &str| -> String {
            headers.get(name).and_then(|i| record.get(*i)).unwrap_or("").to_string()
        };
        out.push(build_row(line, &cell));
    }
    out
}

fn build_row(line: usize, cell: &dyn Fn(&str) -> String) -> Result<ProposedTransaction, ParseIssue> {
    let issue = |message: String| ParseIssue { line, message };
    let transaction_date = parse_date(&cell("date"))
        .ok_or_else(|| issue(format!("data inválida: '{}' (esperado AAAA-MM-DD ou DD/MM/AAAA)", cell("date"))))?;
    let number = |name: &str| parse_number(&cell(name)).map_err(&issue);
    let total_value = number("total_value")?.ok_or_else(|| issue("total_value vazio".to_string()))?;
    Ok(ProposedTransaction {
        line,
        transaction_date,
        transaction_type: cell("type").trim().to_lowercase(),
        ticker: normalize_ticker(&cell("ticker")),
        asset_class: non_empty(&cell("asset_class")),
        currency: non_empty(&cell("currency")).map(|c| c.to_uppercase()),
        quantity: number("quantity")?,
        unit_price: number("unit_price")?,
        total_value,
        fee: number("fee")?,
        custody: non_empty(&cell("custody")),
        payment_type: non_empty(&cell("payment_type")),
        notes: non_empty(&cell("notes")),
    })
}

pub fn export_transactions_csv_text(rows: &[ProposedTransaction]) -> Result<String, AppError> {
    let mut writer = csv::Writer::from_writer(Vec::new());
    writer.write_record(CSV_HEADER).map_err(csv_error)?;
    for row in rows {
        let num = |v: Option<f64>| v.map(|n| n.to_string()).unwrap_or_default();
        writer
            .write_record([
                row.transaction_date.as_str(),
                row.transaction_type.as_str(),
                row.ticker.as_str(),
                row.asset_class.as_deref().unwrap_or(""),
                row.currency.as_deref().unwrap_or(""),
                num(row.quantity).as_str(),
                num(row.unit_price).as_str(),
                row.total_value.to_string().as_str(),
                num(row.fee).as_str(),
                row.custody.as_deref().unwrap_or(""),
                row.payment_type.as_deref().unwrap_or(""),
                row.notes.as_deref().unwrap_or(""),
            ])
            .map_err(csv_error)?;
    }
    let bytes = writer.into_inner().map_err(|e| AppError::InvalidInput(format!("CSV: {e}")))?;
    String::from_utf8(bytes).map_err(|e| AppError::InvalidInput(format!("CSV: {e}")))
}

fn csv_error(err: csv::Error) -> AppError {
    AppError::InvalidInput(format!("CSV: {err}"))
}

// ---------------------------------------------------------------------------
// Planilha "Negociação" da B3
// ---------------------------------------------------------------------------

fn normalize_header(raw: &str) -> String {
    raw.trim()
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' => 'a',
            'é' | 'ê' => 'e',
            'í' => 'i',
            'ó' | 'ô' | 'õ' => 'o',
            'ú' => 'u',
            'ç' => 'c',
            other => other,
        })
        .collect()
}

/// Ticker em caixa alta e sem o `F` do fracionário (`PETR4F` → `PETR4`).
pub fn normalize_ticker(raw: &str) -> String {
    let ticker = raw.trim().to_uppercase();
    let is_fractional = ticker.len() >= 6
        && ticker.ends_with('F')
        && ticker[..ticker.len() - 1].chars().rev().take_while(|c| c.is_ascii_digit()).count() >= 1
        && ticker.chars().take(4).all(|c| c.is_ascii_alphabetic());
    if is_fractional {
        ticker[..ticker.len() - 1].to_string()
    } else {
        ticker
    }
}

/// Linhas da planilha já como texto (a primeira é o cabeçalho). As colunas são
/// achadas pelo nome, então a ordem não importa. Só mercado à vista e
/// fracionário entram; opções, termo e o resto viram erro explicado.
pub fn parse_b3_rows(rows: &[Vec<String>]) -> Parsed {
    let Some(header) = rows.first() else {
        return vec![Err(ParseIssue { line: 0, message: "planilha vazia".to_string() })];
    };
    let find = |needles: &[&str]| {
        header.iter().position(|h| {
            let h = normalize_header(h);
            needles.iter().all(|n| h.contains(n))
        })
    };
    let columns = (
        find(&["data"]),
        find(&["tipo"]),
        find(&["mercado"]),
        find(&["instituicao"]),
        find(&["codigo"]),
        find(&["quantidade"]),
        find(&["preco"]),
        find(&["valor"]),
    );
    let (Some(c_date), Some(c_kind), Some(c_market), Some(c_inst), Some(c_code), Some(c_qty), Some(c_price), Some(c_value)) =
        columns
    else {
        return vec![Err(ParseIssue {
            line: 0,
            message: "não parece a planilha 'Negociação' da B3: faltam colunas (Data do Negócio, Tipo de Movimentação, Mercado, Instituição, Código de Negociação, Quantidade, Preço, Valor)".to_string(),
        })];
    };

    let mut out = Vec::new();
    for (index, row) in rows.iter().enumerate().skip(1) {
        let line = index;
        if row.iter().all(|c| c.trim().is_empty()) {
            continue;
        }
        let get = |i: usize| row.get(i).map(String::as_str).unwrap_or("");
        let issue = |message: String| ParseIssue { line, message };
        let market = normalize_header(get(c_market));
        if !(market.contains("vista") || market.contains("fracion")) {
            out.push(Err(issue(format!("mercado '{}' não suportado (só à vista e fracionário)", get(c_market)))));
            continue;
        }
        let kind = match normalize_header(get(c_kind)).as_str() {
            "compra" => "compra",
            "venda" => "venda",
            other => {
                out.push(Err(issue(format!("tipo de movimentação '{other}' não suportado"))));
                continue;
            }
        };
        let Some(transaction_date) = parse_date(get(c_date)) else {
            out.push(Err(issue(format!("data inválida: '{}'", get(c_date)))));
            continue;
        };
        let parsed = (|| -> Result<(f64, f64, f64), String> {
            let quantity = parse_number(get(c_qty))?.ok_or("quantidade vazia")?;
            let price = parse_number(get(c_price))?.ok_or("preço vazio")?;
            let value = parse_number(get(c_value))?.unwrap_or(quantity * price);
            Ok((quantity, price, value))
        })();
        let (quantity, unit_price, total_value) = match parsed {
            Ok(values) => values,
            Err(message) => {
                out.push(Err(issue(message)));
                continue;
            }
        };
        out.push(Ok(ProposedTransaction {
            line,
            transaction_date,
            transaction_type: kind.to_string(),
            ticker: normalize_ticker(get(c_code)),
            asset_class: None,
            currency: Some("BRL".to_string()),
            quantity: Some(quantity),
            unit_price: Some(unit_price),
            total_value,
            fee: None,
            custody: non_empty(get(c_inst)),
            payment_type: None,
            notes: Some("Importado da B3 (Negociação)".to_string()),
        }));
    }
    out
}

/// Lê a primeira aba do xlsx como texto, linha a linha.
pub fn read_xlsx_rows(path: &str) -> Result<Vec<Vec<String>>, AppError> {
    use calamine::{open_workbook_auto, Data, Reader};
    let mut workbook =
        open_workbook_auto(path).map_err(|e| AppError::InvalidInput(format!("planilha ilegível: {e}")))?;
    let name = workbook
        .sheet_names()
        .first()
        .cloned()
        .ok_or_else(|| AppError::InvalidInput("planilha sem abas".to_string()))?;
    let range = workbook
        .worksheet_range(&name)
        .map_err(|e| AppError::InvalidInput(format!("aba '{name}' ilegível: {e}")))?;
    let text = |cell: &Data| match cell {
        Data::Empty => String::new(),
        Data::String(s) => s.clone(),
        Data::Float(f) => f.to_string(),
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => b.to_string(),
        Data::DateTime(dt) => dt
            .as_datetime()
            .map(|d| d.format("%d/%m/%Y").to_string())
            .unwrap_or_default(),
        Data::DateTimeIso(s) | Data::DurationIso(s) => s.clone(),
        Data::Error(_) => String::new(),
    };
    Ok(range.rows().map(|row| row.iter().map(&text).collect()).collect())
}

// ---------------------------------------------------------------------------
// Prévia
// ---------------------------------------------------------------------------

/// Classe provável de um ticker da B3; `None` quando é ambíguo (final 11 pode
/// ser FII, ETF ou unit) ou desconhecido.
pub fn infer_class(ticker: &str) -> Option<&'static str> {
    let ticker = normalize_ticker(ticker);
    if ticker.len() < 5 || !ticker.chars().take(4).all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    let suffix = &ticker[4..];
    if !suffix.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    match suffix {
        "3" | "4" | "5" | "6" | "7" | "8" => Some("acao_br"),
        "32" | "33" | "34" | "35" | "39" => Some("bdr"),
        _ => None,
    }
}

type DedupKey = (String, String, String, i64, i64);

fn dedup_key(ticker: &str, date: &str, kind: &str, quantity: Option<f64>, total: f64) -> DedupKey {
    (
        ticker.to_string(),
        date.to_string(),
        kind.to_string(),
        (quantity.unwrap_or(0.0) * 1e6).round() as i64,
        (total * 100.0).round() as i64,
    )
}

/// Lançamentos já gravados na carteira, contados por chave de duplicata.
async fn existing_keys<C: ConnectionTrait>(
    db: &C,
    portfolio_id: i32,
) -> Result<HashMap<DedupKey, usize>, AppError> {
    let txs = transactions::Entity::find().filter(transactions::Column::PortfolioId.eq(portfolio_id)).all(db).await?;
    let ids: Vec<i32> = txs.iter().filter_map(|t| t.asset_id).collect();
    let tickers: HashMap<i32, String> = if ids.is_empty() {
        HashMap::new()
    } else {
        assets::Entity::find()
            .filter(assets::Column::Id.is_in(ids))
            .all(db)
            .await?
            .into_iter()
            .map(|a| (a.id, a.ticker))
            .collect()
    };
    let mut keys: HashMap<DedupKey, usize> = HashMap::new();
    for tx in txs {
        let ticker = tx.asset_id.and_then(|id| tickers.get(&id).cloned()).unwrap_or_default();
        *keys.entry(dedup_key(&ticker, &tx.transaction_date, &tx.transaction_type, tx.quantity, tx.total_value)).or_insert(0) += 1;
    }
    Ok(keys)
}

async fn known_tickers<C: ConnectionTrait>(db: &C) -> Result<HashMap<String, assets::Model>, AppError> {
    Ok(assets::Entity::find().all(db).await?.into_iter().map(|a| (a.ticker.to_uppercase(), a)).collect())
}

pub async fn preview_rows<C: ConnectionTrait>(
    db: &C,
    portfolio_id: i32,
    parsed: Parsed,
) -> Result<Vec<ImportRow>, AppError> {
    let mut remaining = existing_keys(db, portfolio_id).await?;
    let assets_by_ticker = known_tickers(db).await?;
    let mut rows = Vec::with_capacity(parsed.len());
    for item in parsed {
        let proposed = match item {
            Ok(p) => p,
            Err(issue) => {
                rows.push(ImportRow {
                    line: issue.line,
                    proposed: None,
                    status: ImportStatus::Error,
                    message: Some(issue.message),
                    suggested_class: None,
                });
                continue;
            }
        };
        let line = proposed.line;
        let key = dedup_key(
            &proposed.ticker,
            &proposed.transaction_date,
            &proposed.transaction_type,
            proposed.quantity,
            proposed.total_value,
        );
        // Cada lançamento já gravado cobre uma linha do arquivo: dois negócios
        // iguais no mesmo dia continuam sendo dois.
        if let Some(count) = remaining.get_mut(&key).filter(|c| **c > 0) {
            *count -= 1;
            rows.push(ImportRow {
                line,
                proposed: Some(proposed),
                status: ImportStatus::Duplicate,
                message: Some("já existe um lançamento igual nesta carteira".to_string()),
                suggested_class: None,
            });
            continue;
        }
        let needs_asset = !proposed.ticker.is_empty() && !assets_by_ticker.contains_key(&proposed.ticker);
        let class = proposed.asset_class.clone().or_else(|| infer_class(&proposed.ticker).map(str::to_string));
        let (status, message) = if needs_asset && class.is_none() {
            (ImportStatus::NeedsClass, Some(format!("ativo '{}' novo: escolha a classe", proposed.ticker)))
        } else {
            (ImportStatus::New, None)
        };
        rows.push(ImportRow { line, proposed: Some(proposed), status, message, suggested_class: class });
    }
    Ok(rows)
}

// ---------------------------------------------------------------------------
// Gravação
// ---------------------------------------------------------------------------

fn class_defaults(class: &str) -> Option<(&'static str, &'static str)> {
    CLASS_DEFAULTS.iter().find(|(c, _, _)| *c == class).map(|(_, currency, country)| (*currency, *country))
}

async fn resolve_asset<C: ConnectionTrait>(
    db: &C,
    known: &mut HashMap<String, assets::Model>,
    item: &CommitItem,
) -> Result<i32, String> {
    let ticker = &item.proposed.ticker;
    if let Some(asset) = known.get(&ticker.to_uppercase()) {
        return Ok(asset.id);
    }
    let class = item
        .asset_class
        .clone()
        .or_else(|| item.proposed.asset_class.clone())
        .or_else(|| infer_class(ticker).map(str::to_string))
        .ok_or_else(|| format!("ativo '{ticker}' novo sem classe"))?;
    let (default_currency, country) = class_defaults(&class).ok_or_else(|| format!("classe '{class}' desconhecida"))?;
    let created = assets::ActiveModel {
        ticker: Set(ticker.clone()),
        name: Set(ticker.clone()),
        asset_class: Set(class),
        currency: Set(item.proposed.currency.clone().unwrap_or_else(|| default_currency.to_string())),
        exchange: Set(None),
        exposure_type: Set("pais".to_string()),
        exposure_value: Set(country.to_string()),
        created_at: Set(chrono::Utc::now().to_rfc3339()),
        ..Default::default()
    }
    .insert(db)
    .await
    .map_err(|e| e.to_string())?;
    known.insert(ticker.to_uppercase(), created.clone());
    Ok(created.id)
}

async fn resolve_custody<C: ConnectionTrait>(
    db: &C,
    workspace_id: i32,
    cache: &mut HashMap<String, i32>,
    name: &str,
) -> Result<i32, String> {
    if let Some(id) = cache.get(&name.to_lowercase()) {
        return Ok(*id);
    }
    let existing = custodia::Entity::find()
        .filter(custodia::Column::WorkspaceId.eq(workspace_id))
        .all(db)
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|c| c.instituicao.eq_ignore_ascii_case(name));
    let id = match existing {
        Some(c) => c.id,
        None => {
            custodia::ActiveModel {
                workspace_id: Set(workspace_id),
                instituicao: Set(name.to_string()),
                titular: Set("Importado".to_string()),
                created_at: Set(chrono::Utc::now().to_rfc3339()),
                ..Default::default()
            }
            .insert(db)
            .await
            .map_err(|e| e.to_string())?
            .id
        }
    };
    cache.insert(name.to_lowercase(), id);
    Ok(id)
}

/// Grava os itens confirmados. Tudo ou nada: se qualquer linha falhar, nada fica
/// gravado e `errors` diz quais. Duplicatas (conferidas de novo aqui) são puladas.
pub async fn commit_rows(
    db: &DatabaseConnection,
    portfolio_id: i32,
    mut items: Vec<CommitItem>,
) -> Result<CommitResult, AppError> {
    let workspace_id = workspace_of_portfolio(db, portfolio_id).await?;
    // Compras antes das vendas do mesmo dia, pra a validação de saldo enxergar a ordem real.
    items.sort_by_key(|i| (i.proposed.transaction_date.clone(), i.proposed.transaction_type == "venda", i.proposed.line));

    let txn = db.begin().await?;
    let mut known = known_tickers(&txn).await?;
    let mut remaining = existing_keys(&txn, portfolio_id).await?;
    let mut custody_cache: HashMap<String, i32> = HashMap::new();
    let mut created = 0;
    let mut skipped = 0;
    let mut errors: Vec<ParseIssue> = Vec::new();

    for item in &items {
        let p = &item.proposed;
        let key = dedup_key(&p.ticker, &p.transaction_date, &p.transaction_type, p.quantity, p.total_value);
        if let Some(count) = remaining.get_mut(&key).filter(|c| **c > 0) {
            *count -= 1;
            skipped += 1;
            continue;
        }
        let outcome = async {
            let asset_id = if p.ticker.is_empty() { None } else { Some(resolve_asset(&txn, &mut known, item).await?) };
            let custodia_id = match &p.custody {
                Some(name) => Some(resolve_custody(&txn, workspace_id, &mut custody_cache, name).await?),
                None => None,
            };
            create_transaction_in(
                &txn,
                CreateTransactionRequest {
                    portfolio_id,
                    asset_id,
                    custodia_id,
                    transfer_to_custodia_id: None,
                    transaction_type: p.transaction_type.clone(),
                    payment_type: p.payment_type.clone(),
                    quantity: p.quantity,
                    unit_price: p.unit_price,
                    total_value: p.total_value,
                    fee: p.fee,
                    transaction_date: p.transaction_date.clone(),
                    notes: p.notes.clone(),
                    fi_emissor: None,
                    fi_indexador: None,
                    fi_taxa_percentual: None,
                    fi_data_vencimento: None,
                    fi_liquidez: None,
                },
            )
            .await
            .map_err(|e| e.to_string())?;
            Ok::<(), String>(())
        }
        .await;
        match outcome {
            Ok(()) => created += 1,
            Err(message) => errors.push(ParseIssue { line: p.line, message }),
        }
    }

    if errors.is_empty() {
        txn.commit().await?;
        Ok(CommitResult { created, skipped_duplicates: skipped, errors })
    } else {
        txn.rollback().await?;
        Ok(CommitResult { created: 0, skipped_duplicates: skipped, errors })
    }
}

// ---------------------------------------------------------------------------
// Exportação
// ---------------------------------------------------------------------------

pub async fn proposed_from_portfolio(
    db: &DatabaseConnection,
    portfolio_id: i32,
) -> Result<Vec<ProposedTransaction>, AppError> {
    let mut txs = transactions::Entity::find().filter(transactions::Column::PortfolioId.eq(portfolio_id)).all(db).await?;
    txs.sort_by(|a, b| (&a.transaction_date, a.id).cmp(&(&b.transaction_date, b.id)));
    let assets_by_id: HashMap<i32, assets::Model> =
        assets::Entity::find().all(db).await?.into_iter().map(|a| (a.id, a)).collect();
    let custodias: HashMap<i32, String> =
        custodia::Entity::find().all(db).await?.into_iter().map(|c| (c.id, c.instituicao)).collect();
    Ok(txs
        .into_iter()
        .enumerate()
        .map(|(index, tx)| {
            let asset = tx.asset_id.and_then(|id| assets_by_id.get(&id));
            ProposedTransaction {
                line: index + 1,
                transaction_date: tx.transaction_date,
                transaction_type: tx.transaction_type,
                ticker: asset.map(|a| a.ticker.clone()).unwrap_or_default(),
                asset_class: asset.map(|a| a.asset_class.clone()),
                currency: asset.map(|a| a.currency.clone()),
                quantity: tx.quantity,
                unit_price: tx.unit_price,
                total_value: tx.total_value,
                fee: tx.fee,
                custody: tx.custodia_id.and_then(|id| custodias.get(&id).cloned()),
                payment_type: tx.payment_type,
                notes: tx.notes,
            }
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Comandos
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn export_transactions_csv(
    db: tauri::State<'_, DatabaseConnection>,
    portfolio_id: i32,
    dest_path: String,
) -> Result<usize, AppError> {
    let rows = proposed_from_portfolio(db.inner(), portfolio_id).await?;
    let text = export_transactions_csv_text(&rows)?;
    std::fs::write(&dest_path, text).map_err(|e| AppError::InvalidInput(format!("não consegui gravar {dest_path}: {e}")))?;
    Ok(rows.len())
}

pub async fn preview_import_in(
    db: &DatabaseConnection,
    portfolio_id: i32,
    path: &str,
    format: &str,
) -> Result<ImportPreview, AppError> {
    let parsed = match format {
        "anchor_csv" => {
            let text = std::fs::read_to_string(path)
                .map_err(|e| AppError::InvalidInput(format!("não consegui ler {path}: {e}")))?;
            parse_anchor_csv(text.trim_start_matches('\u{feff}'))
        }
        "b3_negociacao" => parse_b3_rows(&read_xlsx_rows(path)?),
        other => return Err(AppError::InvalidInput(format!("formato '{other}' desconhecido"))),
    };
    Ok(ImportPreview { rows: preview_rows(db, portfolio_id, parsed).await? })
}

#[tauri::command]
pub async fn preview_import(
    db: tauri::State<'_, DatabaseConnection>,
    portfolio_id: i32,
    path: String,
    format: String,
) -> Result<ImportPreview, AppError> {
    preview_import_in(db.inner(), portfolio_id, &path, &format).await
}

#[tauri::command]
pub async fn commit_import(
    db: tauri::State<'_, DatabaseConnection>,
    portfolio_id: i32,
    items: Vec<CommitItem>,
) -> Result<CommitResult, AppError> {
    commit_rows(db.inner(), portfolio_id, items).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(lines: &[&[&str]]) -> Vec<Vec<String>> {
        lines.iter().map(|l| l.iter().map(|c| c.to_string()).collect()).collect()
    }

    const B3_HEADER: [&str; 9] = [
        "Data do Negócio",
        "Tipo de Movimentação",
        "Mercado",
        "Prazo/Vencimento",
        "Instituição",
        "Código de Negociação",
        "Quantidade",
        "Preço",
        "Valor",
    ];

    #[test]
    fn numbers_and_dates_accept_both_formats() {
        assert_eq!(parse_number("1.234,56"), Ok(Some(1234.56)));
        assert_eq!(parse_number("12.5"), Ok(Some(12.5)));
        assert_eq!(parse_number("R$ 10,00"), Ok(Some(10.0)));
        assert_eq!(parse_number(""), Ok(None));
        assert!(parse_number("abc").is_err());
        assert_eq!(parse_date("05/01/2026"), Some("2026-01-05".to_string()));
        assert_eq!(parse_date("2026-01-05"), Some("2026-01-05".to_string()));
        assert_eq!(parse_date("31/02/2026"), None);
        assert_eq!(parse_date("05-01-26"), None, "ano de 2 dígitos não vale");
        assert_eq!(parse_date("0005-01-26"), None);
    }

    #[test]
    fn the_fractional_suffix_is_removed_from_the_ticker() {
        assert_eq!(normalize_ticker("petr4f"), "PETR4");
        assert_eq!(normalize_ticker("PETR4"), "PETR4");
        assert_eq!(normalize_ticker("BOVA11F"), "BOVA11");
        assert_eq!(normalize_ticker("BBDC3"), "BBDC3");
    }

    #[test]
    fn class_inference_is_confident_only_for_unambiguous_tickers() {
        assert_eq!(infer_class("PETR4"), Some("acao_br"));
        assert_eq!(infer_class("PETR4F"), Some("acao_br"));
        assert_eq!(infer_class("AAPL34"), Some("bdr"));
        assert_eq!(infer_class("HGLG11"), None);
        assert_eq!(infer_class("BOVA11"), None);
        assert_eq!(infer_class("XYZ"), None);
    }

    #[test]
    fn b3_rows_become_proposed_trades_and_explain_what_they_skip() {
        let mut data: Vec<Vec<&str>> = vec![B3_HEADER.to_vec()];
        data.push(vec!["05/01/2026", "Compra", "Mercado à Vista", "-", "XP INVESTIMENTOS", "PETR4", "100", "30,50", "3.050,00"]);
        data.push(vec!["06/01/2026", "Venda", "Mercado Fracionário", "-", "XP INVESTIMENTOS", "PETR4F", "10", "31,00", "310,00"]);
        data.push(vec!["07/01/2026", "Compra", "Opção de Compra", "-", "XP INVESTIMENTOS", "PETRA100", "1", "1,00", "1,00"]);
        data.push(vec!["", "", "", "", "", "", "", "", ""]);
        let owned: Vec<Vec<String>> = data.iter().map(|r| r.iter().map(|c| c.to_string()).collect()).collect();

        let parsed = parse_b3_rows(&owned);
        assert_eq!(parsed.len(), 3, "linha em branco é ignorada");
        let first = parsed[0].as_ref().unwrap();
        assert_eq!(first.transaction_date, "2026-01-05");
        assert_eq!(first.transaction_type, "compra");
        assert_eq!(first.ticker, "PETR4");
        assert_eq!(first.quantity, Some(100.0));
        assert_eq!(first.unit_price, Some(30.5));
        assert_eq!(first.total_value, 3050.0);
        assert_eq!(first.custody.as_deref(), Some("XP INVESTIMENTOS"));
        assert_eq!(first.line, 1);
        let second = parsed[1].as_ref().unwrap();
        assert_eq!(second.ticker, "PETR4", "fracionário vira o ticker normal");
        assert_eq!(second.transaction_type, "venda");
        assert!(parsed[2].as_ref().unwrap_err().message.contains("não suportado"));
    }

    #[test]
    fn a_sheet_without_the_expected_columns_is_rejected_whole() {
        let parsed = parse_b3_rows(&rows(&[&["Nome", "Idade"], &["a", "1"]]));
        assert_eq!(parsed.len(), 1);
        assert!(parsed[0].as_ref().unwrap_err().message.contains("Negociação"));
    }

    #[test]
    fn the_anchor_csv_round_trips() {
        let original = vec![ProposedTransaction {
            line: 1,
            transaction_date: "2026-01-05".to_string(),
            transaction_type: "compra".to_string(),
            ticker: "PETR4".to_string(),
            asset_class: Some("acao_br".to_string()),
            currency: Some("BRL".to_string()),
            quantity: Some(100.0),
            unit_price: Some(30.5),
            total_value: 3050.0,
            fee: Some(4.9),
            custody: Some("XP".to_string()),
            payment_type: None,
            notes: Some("com vírgula, e \"aspas\"".to_string()),
        }];
        let text = export_transactions_csv_text(&original).unwrap();
        let parsed = parse_anchor_csv(&text);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].as_ref().unwrap(), &original[0]);
    }

    #[test]
    fn a_csv_missing_required_columns_or_with_bad_values_reports_the_line() {
        let parsed = parse_anchor_csv("foo,bar\n1,2\n");
        assert!(parsed[0].as_ref().unwrap_err().message.contains("date"));

        let parsed = parse_anchor_csv("date,type,ticker,total_value\n2026-01-05,compra,PETR4,100\n05-01-26,compra,PETR4,100\n");
        assert!(parsed[0].is_ok());
        assert_eq!(parsed[1].as_ref().unwrap_err().line, 2);
    }
}
