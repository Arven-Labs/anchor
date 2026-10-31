// Fase 34 — exportar e importar lançamentos da carteira. A importação tem duas
// etapas: `preview_import` lê o arquivo e devolve cada linha classificada
// (nova, duplicada, com erro, ou ativo novo que precisa de classe) sem gravar
// nada; `commit_import` grava tudo numa transação só, ou nada se alguma linha
// falhar. O núcleo (`ProposedTransaction` → `preview_rows` → `commit_rows`) não
// sabe de onde as linhas vieram: CSV do Anchor, planilha da B3 ou qualquer outra
// origem entram só como mais um leitor na frente dele.

use std::collections::{HashMap, HashSet};

use chrono::{Datelike, NaiveDate};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};

use crate::commands::currency::workspace_of_portfolio;
use crate::commands::tax::load_rule_set;
use crate::domain::proventos::{
    jscp_gross_from_net, PAYMENT_TYPE_ALUGUEL, PAYMENT_TYPE_DIVIDENDO, PAYMENT_TYPE_REEMBOLSO,
};
use crate::domain::tax::rules::{RuleSet, JCP_IRRF_RATE};
use crate::commands::fund::{ESTOQUE_OPTIONS, STRUCTURES};
use crate::commands::transaction::{create_transaction_in, CreateTransactionRequest};
use crate::domain::tax::come_cotas::FundTerm;
use crate::entity::{assets, custodia, fund_detail, import_pending, transactions};
use crate::error::AppError;

const CSV_HEADER: [&str; 16] = [
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
    "loss_credit_used",
    "fund_term",
    "fund_structure",
    "fund_estoque_option",
];

/// Classes que o importador sabe criar sozinho, com a moeda e a exposição padrão.
const CLASS_DEFAULTS: [(&str, &str, &str); 15] = [
    ("acao_br", "BRL", "BR"),
    ("fii", "BRL", "BR"),
    ("etf_br", "BRL", "BR"),
    ("etf_renda_fixa", "BRL", "BR"),
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
    ("fundo", "BRL", "BR"),
];

/// Um lançamento proposto por qualquer origem (arquivo, planilha, IA no futuro),
/// ainda não gravado.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
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
    /// Só no come-cotas: rendimento abatido por crédito de prejuízo. Vazio = não informado; 0 = nenhum usado.
    #[serde(default)]
    pub loss_credit_used: Option<f64>,
    /// Configuração do fundo (`fund_details`), repetida em cada linha do mesmo ticker. Só aplicada onde o fundo ainda
    /// não tem o campo.
    #[serde(default)]
    pub fund_term: Option<String>,
    #[serde(default)]
    pub fund_structure: Option<String>,
    #[serde(default)]
    pub fund_estoque_option: Option<String>,
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

/// Tipo que tira quantidade da carteira (venda ou come-cotas): na ordenação do mesmo dia vem depois das compras.
fn reduces_quantity(transaction_type: &str) -> bool {
    matches!(transaction_type, "venda" | "come_cotas")
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
        loss_credit_used: number("loss_credit_used")?,
        fund_term: non_empty(&cell("fund_term")).map(|v| v.to_lowercase()),
        fund_structure: non_empty(&cell("fund_structure")).map(|v| v.to_lowercase()),
        fund_estoque_option: non_empty(&cell("fund_estoque_option")).map(|v| v.to_lowercase()),
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
                num(row.loss_credit_used).as_str(),
                row.fund_term.as_deref().unwrap_or(""),
                row.fund_structure.as_deref().unwrap_or(""),
                row.fund_estoque_option.as_deref().unwrap_or(""),
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

pub(crate) fn normalize_header(raw: &str) -> String {
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
        // "Renda Fixa" é como a B3 lista ETFs de renda fixa; a classe
        // não dá pra deduzir do mercado, então segue sem classe e é escolhida na revisão.
        if !(market.contains("vista") || market.contains("fracion") || market.contains("renda fixa")) {
            out.push(Err(issue(format!(
                "mercado '{}' não suportado (só à vista, fracionário e renda fixa)",
                get(c_market)
            ))));
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
            ..Default::default()
        }));
    }
    out
}

/// `ABCD3 - EMPRESA A S.A.` → `ABCD3`. `None` quando o que vem antes do
/// primeiro " - " não tem cara de ticker da B3 (ex.: `CDB - CDB926...`).
fn product_ticker(product: &str) -> Option<String> {
    let head = product.split(" - ").next()?;
    let ticker = normalize_ticker(head);
    let letters = ticker.chars().take_while(|c| c.is_ascii_alphabetic()).count();
    let digits = ticker.chars().skip(letters).take_while(|c| c.is_ascii_digit()).count();
    (letters == 4 && (1..=2).contains(&digits) && ticker.len() == letters + digits).then_some(ticker)
}

/// Posição de cada coluna da planilha "Movimentação", achada pelo nome.
pub(crate) struct MovimentacaoColumns {
    pub flow: usize,
    pub date: usize,
    pub movement: usize,
    pub product: usize,
    pub institution: usize,
    pub quantity: usize,
    pub price: usize,
    pub value: usize,
}

pub(crate) fn movimentacao_columns(header: &[String]) -> Option<MovimentacaoColumns> {
    let find = |needle: &str| header.iter().position(|h| normalize_header(h).contains(needle));
    Some(MovimentacaoColumns {
        flow: find("entrada")?,
        date: find("data")?,
        movement: find("movimentacao")?,
        product: find("produto")?,
        institution: find("instituicao")?,
        quantity: find("quantidade")?,
        price: find("preco")?,
        value: find("valor")?,
    })
}

pub(crate) const MOVIMENTACAO_COLUMNS_MESSAGE: &str = "não parece a planilha 'Movimentação' da B3: faltam colunas (Entrada/Saída, Data, Movimentação, Produto, Instituição, Quantidade, Preço unitário, Valor da Operação)";

/// Planilha "Movimentação" da Área do Investidor. Viram lançamento os proventos em
/// dinheiro (`Dividendo`, `Rendimento`, JCP), o pagamento do aluguel de ações (`Empréstimo`
/// com valor, tipo `aluguel`) e o `Reembolso` de provento do aluguel (tipo `reembolso`). A renda fixa
/// (CDB, LCI, LCA, CRI, CRA) vira posição: aplicação é `compra` e resgate é `venda` ao custo
/// (`fixed_income_io::parse_fixed_income_positions`). O resto é pulado com o motivo, em vez de
/// adivinhado: liquidação (repete os negócios da "Negociação") e o `Empréstimo` sem valor (só custódia).
///
/// **Aluguel e reembolso**: entram como creditados, sem reconstituir bruto. O aluguel vem líquido do IR
/// retido na fonte (a B3 diz que o rendimento "já vem líquido"; é o valor que a DIRPF pede em tributação
/// exclusiva). O reembolso é igual ao provento das ações emprestadas: na amostra, o de um JCP bate com
/// quantidade emprestada × preço unitário × 0,85, o mesmo líquido de 15% do JCP, e o de dividendo não tem retenção.
///
/// **JCP**: na amostra real, o "Valor da Operação" é o **líquido** (o preço unitário é o
/// bruto por ação, e valor ÷ (quantidade × preço) dá 0,85 em todas as linhas). O app guarda o
/// bruto em `total_value` e desconta o IR na leitura, então o bruto é reconstituído do
/// líquido com a alíquota de `jcp_irrf_rate` vigente na data do pagamento.
pub fn parse_b3_movimentacao_rows(rows: &[Vec<String>], rules: &RuleSet) -> Parsed {
    let Some(header) = rows.first() else {
        return vec![Err(ParseIssue { line: 0, message: "planilha vazia".to_string() })];
    };
    let Some(MovimentacaoColumns {
        flow: c_flow,
        date: c_date,
        movement: c_move,
        product: c_product,
        institution: c_inst,
        quantity: c_qty,
        price: c_price,
        value: c_value,
    }) = movimentacao_columns(header)
    else {
        return vec![Err(ParseIssue {
            line: 0,
            message: MOVIMENTACAO_COLUMNS_MESSAGE.to_string(),
        })];
    };

    let mut out = Vec::new();
    let mut positions = crate::commands::fixed_income_io::parse_fixed_income_positions(rows);
    for (index, row) in rows.iter().enumerate().skip(1) {
        let line = index;
        if row.iter().all(|c| c.trim().is_empty()) {
            continue;
        }
        let get = |i: usize| row.get(i).map(String::as_str).unwrap_or("");
        let issue = |message: String| ParseIssue { line, message };
        let movement = normalize_header(get(c_move));
        let product = get(c_product).trim();

        let is_jcp = movement.contains("juros sobre capital");
        if movement.contains("liquidacao") {
            out.push(Err(issue("liquidação de negócio pulada: os negócios vêm da planilha 'Negociação'".to_string())));
            continue;
        }
        // Renda fixa: aplicação vira compra e resgate vira venda ao custo (posição de carteira); o
        // rendimento entra pela conta, por Finances → Transactions → Fixed-income yield.
        if let Some(position) = positions.remove(&line) {
            out.push(position);
            continue;
        }
        if movement.contains("aplicacao") || movement.contains("resgate") || product.to_uppercase().starts_with("CDB") {
            out.push(Err(issue("aplicação ou resgate de produto que não é renda fixa reconhecida pulado".to_string())));
            continue;
        }
        let is_rental = movement.contains("emprestimo");
        let is_refund = movement.contains("reembolso");
        // O aluguel tem duas linhas: a do contrato (sem valor, a B3 só move a custódia) e a do pagamento (com valor).
        if is_rental && !matches!(parse_number(get(c_value)), Ok(Some(value)) if value > 0.0) {
            out.push(Err(issue(
                "movimento de custódia do aluguel (Empréstimo sem valor) pulado: a renda vem na linha com valor".to_string(),
            )));
            continue;
        }
        if movement != "dividendo" && movement != "rendimento" && !is_jcp && !is_rental && !is_refund {
            out.push(Err(issue(format!("movimentação '{}' não suportada", get(c_move).trim()))));
            continue;
        }
        if normalize_header(get(c_flow)) != "credito" {
            out.push(Err(issue(format!("provento com sentido '{}' (esperado crédito)", get(c_flow).trim()))));
            continue;
        }
        let Some(ticker) = product_ticker(product) else {
            out.push(Err(issue(format!("não achei o ticker em '{product}'"))));
            continue;
        };
        let Some(transaction_date) = parse_date(get(c_date)) else {
            out.push(Err(issue(format!("data inválida: '{}'", get(c_date)))));
            continue;
        };
        let parsed = (|| -> Result<(Option<f64>, Option<f64>, f64), String> {
            let quantity = parse_number(get(c_qty))?;
            let price = parse_number(get(c_price))?;
            let value = parse_number(get(c_value))?.ok_or("valor vazio")?;
            Ok((quantity, price, value))
        })();
        let (quantity, unit_price, total_value) = match parsed {
            Ok(values) => values,
            Err(message) => {
                out.push(Err(issue(message)));
                continue;
            }
        };
        let (total_value, payment_type, notes) = if is_jcp {
            let rate = NaiveDate::parse_from_str(&transaction_date, "%Y-%m-%d")
                .map_err(|e| e.to_string())
                .and_then(|date| rules.get(JCP_IRRF_RATE, date).map_err(|e| e.to_string()));
            match rate {
                Ok(rate) => (
                    jscp_gross_from_net(total_value, rate),
                    "jscp",
                    format!(
                        "Importado da B3 (Movimentação); JCP: líquido {total_value:.2}, bruto reconstituído à alíquota de {:.1}%",
                        rate * 100.0
                    ),
                ),
                Err(message) => {
                    out.push(Err(issue(message)));
                    continue;
                }
            }
        } else if is_rental {
            (
                total_value,
                PAYMENT_TYPE_ALUGUEL,
                "Importado da B3 (Movimentação); aluguel de ações, valor líquido do IR retido na fonte (como creditado)".to_string(),
            )
        } else if is_refund {
            (
                total_value,
                PAYMENT_TYPE_REEMBOLSO,
                "Importado da B3 (Movimentação); reembolso de provento do aluguel, valor como creditado (igual ao provento das ações emprestadas: líquido no JCP, sem IR no dividendo)".to_string(),
            )
        } else {
            (total_value, PAYMENT_TYPE_DIVIDENDO, "Importado da B3 (Movimentação)".to_string())
        };
        // O reembolso vem com quantidade 0: não é por ação, então fica sem quantidade.
        let quantity = if is_refund { None } else { quantity };
        out.push(Ok(ProposedTransaction {
            line,
            transaction_date,
            transaction_type: "provento".to_string(),
            ticker,
            asset_class: None,
            currency: Some("BRL".to_string()),
            quantity,
            unit_price,
            total_value,
            fee: None,
            custody: non_empty(get(c_inst)),
            payment_type: Some(payment_type.to_string()),
            notes: Some(notes),
            ..Default::default()
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

/// Itens da fila de pendentes (aguardando ou descartados), contados por chave de
/// duplicata: reimportar o mesmo arquivo não recria o que já está na fila.
async fn pending_keys<C: ConnectionTrait>(
    db: &C,
    portfolio_id: i32,
) -> Result<HashMap<DedupKey, usize>, AppError> {
    let rows = import_pending::Entity::find()
        .filter(import_pending::Column::PortfolioId.eq(portfolio_id))
        .filter(import_pending::Column::Kind.eq(KIND_TRANSACTION))
        .all(db)
        .await?;
    let mut keys: HashMap<DedupKey, usize> = HashMap::new();
    for row in rows {
        let Ok(payload) = serde_json::from_str::<PendingPayload>(&row.payload) else { continue };
        let p = payload.proposed;
        *keys.entry(dedup_key(&p.ticker, &p.transaction_date, &p.transaction_type, p.quantity, p.total_value)).or_insert(0) += 1;
    }
    Ok(keys)
}

pub async fn preview_rows<C: ConnectionTrait>(
    db: &C,
    portfolio_id: i32,
    parsed: Parsed,
) -> Result<Vec<ImportRow>, AppError> {
    let mut remaining = existing_keys(db, portfolio_id).await?;
    for (key, count) in pending_keys(db, portfolio_id).await? {
        *remaining.entry(key).or_insert(0) += count;
    }
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

/// Cria o `fund_details` do fundo a partir das colunas `fund_*` da linha. Se o fundo já tem `fund_details`, a configuração
/// do usuário vale e o arquivo não mexe nela. Quem chama só passa linhas com alguma coluna `fund_*`.
async fn apply_fund_details<C: ConnectionTrait>(db: &C, asset_id: i32, p: &ProposedTransaction) -> Result<(), String> {
    let term = p.fund_term.as_deref().ok_or("fund_structure e fund_estoque_option precisam de fund_term")?;
    if FundTerm::parse(term).is_none() {
        return Err(format!("fund_term inválido: '{term}' (longo_prazo, curto_prazo, acoes ou entidade_investimento)"));
    }
    let structure = p.fund_structure.as_deref().unwrap_or("aberto");
    if !STRUCTURES.contains(&structure) {
        return Err(format!("fund_structure inválido: '{structure}' (aberto, fechado ou exclusivo)"));
    }
    let estoque = p.fund_estoque_option.as_deref();
    if estoque.is_some_and(|o| !ESTOQUE_OPTIONS.contains(&o)) {
        return Err(format!("fund_estoque_option inválido: '{}' (geral ou opcao_8)", estoque.unwrap_or("")));
    }
    let asset = assets::Entity::find_by_id(asset_id).one(db).await.map_err(|e| e.to_string())?;
    if asset.is_none_or(|a| a.asset_class != "fundo") {
        return Err("as colunas fund_* só valem para ativos da classe fundo".to_string());
    }
    let existing = fund_detail::Entity::find()
        .filter(fund_detail::Column::AssetId.eq(asset_id))
        .one(db)
        .await
        .map_err(|e| e.to_string())?;
    if existing.is_some() {
        return Ok(());
    }
    fund_detail::ActiveModel {
        asset_id: Set(asset_id),
        term: Set(term.to_string()),
        structure: Set(structure.to_string()),
        estoque_option: Set(estoque.map(str::to_string)),
        created_at: Set(chrono::Utc::now().to_rfc3339()),
        ..Default::default()
    }
    .insert(db)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Grava os itens confirmados. Tudo ou nada: se qualquer linha falhar, nada fica
/// gravado e `errors` diz quais. Duplicatas (conferidas de novo aqui) são puladas.
pub async fn commit_rows(
    db: &DatabaseConnection,
    portfolio_id: i32,
    mut items: Vec<CommitItem>,
) -> Result<CommitResult, AppError> {
    let workspace_id = workspace_of_portfolio(db, portfolio_id).await?;
    // Compras antes das vendas do mesmo dia, pra a validação de saldo enxergar a ordem real (o come-cotas tira cotas
    // como uma venda).
    items.sort_by_key(|i| {
        (i.proposed.transaction_date.clone(), reduces_quantity(&i.proposed.transaction_type), i.proposed.line)
    });

    let txn = db.begin().await?;
    let mut known = known_tickers(&txn).await?;
    let mut remaining = existing_keys(&txn, portfolio_id).await?;
    let mut custody_cache: HashMap<String, i32> = HashMap::new();
    let mut fund_details_done: HashSet<i32> = HashSet::new();
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
                    foreign_tax_withheld: None,
                    foreign_tax_paid_on: None,
                    loss_credit_used: p.loss_credit_used,
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
            if let Some(asset_id) = asset_id {
                let has_fund_columns =
                    p.fund_term.is_some() || p.fund_structure.is_some() || p.fund_estoque_option.is_some();
                if has_fund_columns && fund_details_done.insert(asset_id) {
                    apply_fund_details(&txn, asset_id, p).await?;
                }
            }
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
    let fund_details: HashMap<i32, fund_detail::Model> =
        fund_detail::Entity::find().all(db).await?.into_iter().map(|d| (d.asset_id, d)).collect();
    Ok(txs
        .into_iter()
        .enumerate()
        .map(|(index, tx)| {
            let asset = tx.asset_id.and_then(|id| assets_by_id.get(&id));
            let detail = tx.asset_id.and_then(|id| fund_details.get(&id));
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
                loss_credit_used: tx.loss_credit_used,
                fund_term: detail.map(|d| d.term.clone()),
                fund_structure: detail.map(|d| d.structure.clone()),
                fund_estoque_option: detail.and_then(|d| d.estoque_option.clone()),
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

fn parse_file(path: &str, format: &str, rules: &RuleSet) -> Result<Parsed, AppError> {
    Ok(match format {
        "anchor_csv" => {
            let text = std::fs::read_to_string(path)
                .map_err(|e| AppError::InvalidInput(format!("não consegui ler {path}: {e}")))?;
            parse_anchor_csv(text.trim_start_matches('\u{feff}'))
        }
        "b3_negociacao" => parse_b3_rows(&read_xlsx_rows(path)?),
        "b3_movimentacao" => parse_b3_movimentacao_rows(&read_xlsx_rows(path)?, rules),
        other => return Err(AppError::InvalidInput(format!("formato '{other}' desconhecido"))),
    })
}

pub async fn preview_import_in(
    db: &DatabaseConnection,
    portfolio_id: i32,
    path: &str,
    format: &str,
) -> Result<ImportPreview, AppError> {
    let parsed = parse_file(path, format, &load_rule_set(db).await?)?;
    Ok(ImportPreview { rows: preview_rows(db, portfolio_id, parsed).await? })
}

// ---------------------------------------------------------------------------
// Fila de pendentes (Fase 35)
// ---------------------------------------------------------------------------

pub const KIND_TRANSACTION: &str = "transaction";
pub(crate) const STATUS_PENDING: &str = "pending";
pub(crate) const STATUS_DISCARDED: &str = "discarded";
/// Aprovado com edição que mudou a chave de duplicata: a linha fica só pra reimportar
/// o mesmo arquivo não trazer o item original de volta.
pub(crate) const STATUS_CONFIRMED: &str = "confirmed";

/// O que a fila guarda de cada item: a linha proposta e a classe sugerida ou
/// já escolhida (só importa para ativo novo).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PendingPayload {
    pub proposed: ProposedTransaction,
    pub asset_class: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PendingItemView {
    pub id: i32,
    pub kind: String,
    pub source: String,
    pub created_at: String,
    pub proposed: ProposedTransaction,
    pub asset_class: Option<String>,
    /// O ticker ainda não existe no catálogo, então a revisão precisa de uma classe.
    pub needs_class: bool,
}

#[derive(Serialize)]
pub struct StageResult {
    pub staged: usize,
    pub duplicates: usize,
    /// Linhas que o leitor não conseguiu interpretar; ficam fora da fila.
    pub errors: Vec<ParseIssue>,
}

/// Lê o arquivo e guarda na fila o que é novo (inclusive o que precisa de
/// classe). Duplicatas só são contadas e erros de leitura só são devolvidos.
pub async fn stage_import_in(
    db: &DatabaseConnection,
    portfolio_id: i32,
    path: &str,
    format: &str,
) -> Result<StageResult, AppError> {
    let rules = load_rule_set(db).await?;
    stage_parsed(db, portfolio_id, format, parse_file(path, format, &rules)?).await
}

/// O mesmo que `stage_import_in` para linhas já lidas, de qualquer origem.
pub async fn stage_parsed(
    db: &DatabaseConnection,
    portfolio_id: i32,
    source: &str,
    parsed: Parsed,
) -> Result<StageResult, AppError> {
    let workspace_id = workspace_of_portfolio(db, portfolio_id).await?;
    let rows = preview_rows(db, portfolio_id, parsed).await?;
    let now = chrono::Utc::now().to_rfc3339();
    let txn = db.begin().await?;
    let mut result = StageResult { staged: 0, duplicates: 0, errors: Vec::new() };
    for row in rows {
        match (row.status, row.proposed) {
            (ImportStatus::Error, _) | (_, None) => result.errors.push(ParseIssue {
                line: row.line,
                message: row.message.unwrap_or_else(|| "linha ilegível".to_string()),
            }),
            (ImportStatus::Duplicate, _) => result.duplicates += 1,
            (_, Some(proposed)) => {
                let payload = PendingPayload { proposed, asset_class: row.suggested_class };
                import_pending::ActiveModel {
                    workspace_id: Set(workspace_id),
                    portfolio_id: Set(Some(portfolio_id)),
                    kind: Set(KIND_TRANSACTION.to_string()),
                    source: Set(source.to_string()),
                    payload: Set(serde_json::to_string(&payload).map_err(|e| AppError::InvalidInput(e.to_string()))?),
                    status: Set(STATUS_PENDING.to_string()),
                    created_at: Set(now.clone()),
                    ..Default::default()
                }
                .insert(&txn)
                .await?;
                result.staged += 1;
            }
        }
    }
    txn.commit().await?;
    Ok(result)
}

/// Pendentes em ordem de revisão: por data, compras antes de vendas do dia, depois pelo id.
pub async fn list_import_pending_in(
    db: &DatabaseConnection,
    portfolio_id: i32,
) -> Result<Vec<PendingItemView>, AppError> {
    let rows = import_pending::Entity::find()
        .filter(import_pending::Column::PortfolioId.eq(portfolio_id))
        .filter(import_pending::Column::Status.eq(STATUS_PENDING))
        .all(db)
        .await?;
    let known = known_tickers(db).await?;
    let mut items: Vec<PendingItemView> = rows
        .into_iter()
        .filter_map(|row| {
            let payload: PendingPayload = serde_json::from_str(&row.payload).ok()?;
            let needs_class = !payload.proposed.ticker.is_empty() && !known.contains_key(&payload.proposed.ticker);
            Some(PendingItemView {
                id: row.id,
                kind: row.kind,
                source: row.source,
                created_at: row.created_at,
                proposed: payload.proposed,
                asset_class: payload.asset_class,
                needs_class,
            })
        })
        .collect();
    items.sort_by_key(|i| (i.proposed.transaction_date.clone(), reduces_quantity(&i.proposed.transaction_type), i.id));
    Ok(items)
}

async fn pending_row(db: &DatabaseConnection, portfolio_id: i32, id: i32) -> Result<import_pending::Model, AppError> {
    let row = import_pending::Entity::find_by_id(id)
        .one(db)
        .await?
        .filter(|r| r.kind == KIND_TRANSACTION && r.portfolio_id == Some(portfolio_id))
        .ok_or_else(|| AppError::InvalidInput(format!("pendente {id} não existe nesta carteira")))?;
    if row.status != STATUS_PENDING {
        return Err(AppError::InvalidInput(format!("pendente {id} não está aguardando revisão (status '{}')", row.status)));
    }
    Ok(row)
}

/// Aprova um pendente já com as edições do usuário. Grava só esse item; se a
/// validação recusar (ex.: venda acima do saldo), nada muda e o item continua
/// pendente. Um lançamento igual já gravado conta como aprovado sem duplicar.
pub async fn confirm_import_pending_in(
    db: &DatabaseConnection,
    portfolio_id: i32,
    id: i32,
    proposed: ProposedTransaction,
    asset_class: Option<String>,
) -> Result<CommitResult, AppError> {
    let row = pending_row(db, portfolio_id, id).await?;
    // Se a edição mudou a chave de duplicata, o item original do arquivo deixa de casar
    // com o lançamento gravado; a linha fica como `confirmed` pra reimportar não trazê-lo de volta.
    let key_of = |p: &ProposedTransaction| {
        dedup_key(&p.ticker, &p.transaction_date, &p.transaction_type, p.quantity, p.total_value)
    };
    let key_changed = serde_json::from_str::<PendingPayload>(&row.payload)
        .map(|original| key_of(&original.proposed) != key_of(&proposed))
        .unwrap_or(false);
    let result = commit_rows(db, portfolio_id, vec![CommitItem { proposed, asset_class }]).await?;
    if result.errors.is_empty() {
        if key_changed {
            let mut active: import_pending::ActiveModel = row.into();
            active.status = Set(STATUS_CONFIRMED.to_string());
            active.update(db).await?;
        } else {
            import_pending::Entity::delete_by_id(row.id).exec(db).await?;
        }
    }
    Ok(result)
}

pub async fn discard_import_pending_in(db: &DatabaseConnection, portfolio_id: i32, id: i32) -> Result<(), AppError> {
    let row = pending_row(db, portfolio_id, id).await?;
    let mut active: import_pending::ActiveModel = row.into();
    active.status = Set(STATUS_DISCARDED.to_string());
    active.update(db).await?;
    Ok(())
}

/// Tratamento fiscal que a classe implica, em texto curto para a revisão.
pub fn class_tax_hint_text(asset_class: &str, ticker: &str) -> &'static str {
    use crate::domain::tax::classify::{treatment, TaxTreatment};
    if asset_class == "etf_renda_fixa" {
        return "Fixed-income ETF: IR is withheld at source, so it is left out of the monthly assessment";
    }
    match treatment(asset_class, ticker) {
        TaxTreatment::StockExemptEligible => "Common stock: sales up to the monthly limit may be exempt",
        TaxTreatment::CommonNoExemption => "Taxed as common variable income, never exempt (unit, stock ETF or BDR)",
        TaxTreatment::Fii => "FII/Fiagro: its own tax bucket, no exemption",
        TaxTreatment::Crypto => "Crypto: its own assessment (DARF 4600), sales up to the monthly limit are exempt",
        TaxTreatment::ForeignWarning => "Foreign asset: annual assessment, only warned about",
        TaxTreatment::Ignored => "Not included in the tax assessment",
    }
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

#[tauri::command]
pub async fn stage_import(
    db: tauri::State<'_, DatabaseConnection>,
    portfolio_id: i32,
    path: String,
    format: String,
) -> Result<StageResult, AppError> {
    stage_import_in(db.inner(), portfolio_id, &path, &format).await
}

#[tauri::command]
pub async fn list_import_pending(
    db: tauri::State<'_, DatabaseConnection>,
    portfolio_id: i32,
) -> Result<Vec<PendingItemView>, AppError> {
    list_import_pending_in(db.inner(), portfolio_id).await
}

#[tauri::command]
pub async fn confirm_import_pending(
    db: tauri::State<'_, DatabaseConnection>,
    portfolio_id: i32,
    id: i32,
    proposed: ProposedTransaction,
    asset_class: Option<String>,
) -> Result<CommitResult, AppError> {
    confirm_import_pending_in(db.inner(), portfolio_id, id, proposed, asset_class).await
}

#[tauri::command]
pub async fn discard_import_pending(
    db: tauri::State<'_, DatabaseConnection>,
    portfolio_id: i32,
    id: i32,
) -> Result<(), AppError> {
    discard_import_pending_in(db.inner(), portfolio_id, id).await
}

#[tauri::command]
pub fn class_tax_hint(asset_class: String, ticker: String) -> String {
    class_tax_hint_text(&asset_class, &ticker).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::tax::rules::test_rule_set;

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
    fn a_renda_fixa_market_is_read_without_a_class() {
        let owned = rows(&[
            &B3_HEADER,
            &["16/10/2026", "Compra", "Renda Fixa", "-", "XP", "WXYZ11", "70", "112.11", "7847.7"],
        ]);
        let parsed = parse_b3_rows(&owned);
        let row = parsed[0].as_ref().unwrap();
        assert_eq!(row.ticker, "WXYZ11");
        assert_eq!(row.asset_class, None, "a classe é escolhida na revisão");
        assert_eq!(infer_class(&row.ticker), None);
    }

    const MOVEMENT_HEADER: [&str; 8] = [
        "Entrada/Saída",
        "Data",
        "Movimentação",
        "Produto",
        "Instituição",
        "Quantidade",
        "Preço unitário",
        "Valor da Operação",
    ];

    #[test]
    fn movement_rows_import_cash_dividends_and_skip_the_rest_with_a_reason() {
        let owned = rows(&[
            &MOVEMENT_HEADER,
            &["Credito", "30/09/2026", "Dividendo", "ABCD3 - EMPRESA A S.A.", "CORRETORA", "100", "0.05", "5"],
            &["Credito", "29/09/2026", "Rendimento", "WXYZ11 - FUNDO IMOBILIARIO", "CORRETORA", "10", "1.2", "12"],
            &["Credito", "28/09/2026", "Juros Sobre Capital Próprio", "ABCD3 - EMPRESA A S.A.", "CORRETORA", "100", "0.1", "10"],
            &["Credito", "27/09/2026", "Transferência - Liquidação", "ABCD3 - EMPRESA A S.A.", "CORRETORA", "100", "9", "900"],
            &["Credito", "26/09/2026", "APLICAÇÃO", "CDB - CDB123", "BANCO", "1000", "0.01", "10"],
            &["Debito", "25/09/2026", "RESGATE ANTECIPADO/", "CDB - CDB123 - BANCO", "BANCO", "1000", "0.0101", "10.1"],
            &["Credito", "24/09/2026", "Empréstimo", "ABCD3 - EMPRESA A S.A.      ", "CORRETORA", "300", "-", "1.5"],
            &["Credito", "23/09/2026", "Reembolso", "ABCD3 - EMPRESA A S.A.      ", "CORRETORA", "0", "-", "2.5"],
            &["Credito", "22/09/2026", "Bonificação", "ABCD3 - EMPRESA A S.A.", "CORRETORA", "5", "-", "-"],
            &["Credito", "21/09/2026", "Empréstimo", "ABCD3 - EMPRESA A S.A.      ", "CORRETORA", "300", "-", "-"],
        ]);
        let parsed = parse_b3_movimentacao_rows(&owned, &test_rule_set());
        assert_eq!(parsed.len(), 10);

        let dividend = parsed[0].as_ref().unwrap();
        assert_eq!(dividend.transaction_type, "provento");
        assert_eq!(dividend.ticker, "ABCD3");
        assert_eq!(dividend.transaction_date, "2026-09-30");
        assert_eq!((dividend.quantity, dividend.unit_price, dividend.total_value), (Some(100.0), Some(0.05), 5.0));
        assert_eq!(dividend.payment_type.as_deref(), Some("dividendo"));
        assert_eq!(parsed[1].as_ref().unwrap().ticker, "WXYZ11", "rendimento de FII também é provento");

        // JCP: o valor da planilha é o líquido; o bruto é reconstituído com a alíquota da data (17,5% desde 2026).
        let jcp = parsed[2].as_ref().unwrap();
        assert_eq!(jcp.payment_type.as_deref(), Some("jscp"));
        assert_eq!((jcp.quantity, jcp.unit_price, jcp.total_value), (Some(100.0), Some(0.1), 12.12));
        assert!(jcp.notes.as_deref().unwrap().contains("líquido 10.00") && jcp.notes.as_deref().unwrap().contains("17.5%"));

        // Aluguel: a linha com valor é a renda (sem reconstituir bruto); o preço unitário vem vazio.
        let rental = parsed[6].as_ref().unwrap();
        assert_eq!(rental.transaction_type, "provento");
        assert_eq!(rental.payment_type.as_deref(), Some("aluguel"));
        assert_eq!((rental.quantity, rental.unit_price, rental.total_value), (Some(300.0), None, 1.5));
        // Reembolso: só valor, a quantidade 0 da planilha não vira quantidade.
        let refund = parsed[7].as_ref().unwrap();
        assert_eq!(refund.payment_type.as_deref(), Some("reembolso"));
        assert_eq!((refund.quantity, refund.total_value), (None, 2.5));

        // Renda fixa: a aplicação vira compra e o resgate, venda ao custo (o preço da aplicação, não o do resgate).
        let application = parsed[4].as_ref().unwrap();
        assert_eq!((application.transaction_type.as_str(), application.asset_class.as_deref()), ("compra", Some("renda_fixa")));
        assert_eq!((application.ticker.as_str(), application.quantity, application.total_value), ("CDB123", Some(1000.0), 10.0));
        let redemption = parsed[5].as_ref().unwrap();
        assert_eq!(redemption.transaction_type, "venda");
        assert_eq!((redemption.quantity, redemption.unit_price, redemption.total_value), (Some(1000.0), Some(0.01), 10.0));
        assert!(redemption.notes.as_deref().unwrap().contains("ao custo"));

        let reasons: Vec<String> = [3, 8, 9].iter().map(|&i| parsed[i].as_ref().unwrap_err().message.clone()).collect();
        assert!(reasons[0].contains("liquidação") && reasons[0].contains("Negociação"));
        assert!(reasons[1].contains("Bonificação") && reasons[1].contains("não suportada"));
        assert!(reasons[2].contains("custódia") && reasons[2].contains("aluguel"));
    }

    #[test]
    fn a_jcp_gross_is_rebuilt_from_the_net_with_the_rate_of_the_payment_date() {
        let sheet = rows(&[
            &MOVEMENT_HEADER,
            &["Credito", "15/12/2025", "Juros Sobre Capital Próprio", "ABCD3 - EMPRESA A S.A.", "CORRETORA", "100", "0.1470", "12.34"],
            &["Credito", "02/01/2026", "Juros Sobre Capital Próprio", "ABCD3 - EMPRESA A S.A.", "CORRETORA", "100", "0.1470", "12.34"],
        ]);
        let parsed = parse_b3_movimentacao_rows(&sheet, &test_rule_set());
        // 2025: IRRF de 15% → bruto 14,52 (14,52 × 0,85 = 12,342 → 12,34). 2026: 17,5% → bruto 14,96 (× 0,825 = 12,342 → 12,34).
        assert_eq!(parsed[0].as_ref().unwrap().total_value, 14.52);
        assert_eq!(parsed[1].as_ref().unwrap().total_value, 14.96);
        // Sem a regra de IRRF do JCP, a linha é recusada em vez de assumir uma alíquota.
        let none = parse_b3_movimentacao_rows(&sheet, &RuleSet::new(vec![]));
        assert!(none[0].as_ref().unwrap_err().message.contains("jcp_irrf_rate"));
    }

    #[test]
    fn a_movement_sheet_without_the_expected_columns_is_rejected_whole() {
        let parsed = parse_b3_movimentacao_rows(&rows(&[&["Nome", "Idade"], &["a", "1"]]), &test_rule_set());
        assert_eq!(parsed.len(), 1);
        assert!(parsed[0].as_ref().unwrap_err().message.contains("Movimentação"));
    }

    #[test]
    fn product_ticker_reads_the_code_before_the_name() {
        assert_eq!(product_ticker("ABCD3 - EMPRESA A").as_deref(), Some("ABCD3"));
        assert_eq!(product_ticker("taee11 - TRANSMISSORA   ").as_deref(), Some("TAEE11"));
        assert_eq!(product_ticker("CDB - CDB926EMY03"), None);
        assert_eq!(product_ticker("Tesouro Selic 2029"), None);
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
            ..Default::default()
        }];
        let text = export_transactions_csv_text(&original).unwrap();
        let parsed = parse_anchor_csv(&text);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].as_ref().unwrap(), &original[0]);
    }

    #[test]
    fn the_fund_columns_round_trip_and_zero_credit_differs_from_blank() {
        let come_cotas = |credit: Option<f64>| ProposedTransaction {
            line: 1,
            transaction_date: "2025-05-30".to_string(),
            transaction_type: "come_cotas".to_string(),
            ticker: "FUNDO1".to_string(),
            asset_class: Some("fundo".to_string()),
            currency: Some("BRL".to_string()),
            quantity: Some(2.5),
            unit_price: Some(12.0),
            total_value: 30.0,
            loss_credit_used: credit,
            fund_term: Some("longo_prazo".to_string()),
            fund_structure: Some("fechado".to_string()),
            fund_estoque_option: Some("opcao_8".to_string()),
            ..Default::default()
        };
        for credit in [None, Some(0.0), Some(5.0)] {
            let original = vec![come_cotas(credit)];
            let parsed = parse_anchor_csv(&export_transactions_csv_text(&original).unwrap());
            assert_eq!(parsed[0].as_ref().unwrap(), &original[0], "crédito {credit:?}");
        }
    }

    /// Fixture com a saída de `transactionsCsv` do mobile (`mobile/lib/domain/transactions_csv.dart`): o
    /// importador do desktop tem de ler o que o celular exporta, sem erro, com data só em `AAAA-MM-DD` (a chave de
    /// duplicata compara o texto da data) e com o escape de aspas e quebra de linha preservado.
    #[test]
    fn a_csv_exported_by_the_mobile_app_parses_without_issues() {
        let text = "date,type,ticker,asset_class,currency,quantity,unit_price,total_value,fee,custody,payment_type,notes,loss_credit_used,fund_term,fund_structure,fund_estoque_option\n\
                    2026-03-10,compra,PETR4,acao_br,BRL,10,100,1000,1.5,\"Corretora \"\"A\"\", filial\",,\"linha 1\nlinha 2\",,,,\n\
                    2026-03-11,aporte,,,,,,5000,,,,,,,,\n\
                    2026-03-12,provento,HGLG11,fii,BRL,,,80,,,,,,,,\n\
                    2026-03-13,come_cotas,FUNDO1,fundo,BRL,0.5,120,60,,,,,,,,\n";
        let parsed = parse_anchor_csv(text);
        assert_eq!(parsed.len(), 4);
        assert!(parsed.iter().all(|row| row.is_ok()), "{parsed:?}");

        let buy = parsed[0].as_ref().unwrap();
        assert_eq!(buy.transaction_date, "2026-03-10");
        assert_eq!(buy.custody.as_deref(), Some("Corretora \"A\", filial"));
        assert_eq!(buy.notes.as_deref(), Some("linha 1\nlinha 2"));

        let deposit = parsed[1].as_ref().unwrap();
        assert_eq!((deposit.ticker.as_str(), deposit.quantity, deposit.total_value), ("", None, 5000.0));
    }

    #[test]
    fn a_csv_from_before_the_fund_columns_still_parses() {
        let old = "date,type,ticker,asset_class,currency,quantity,unit_price,total_value,fee,custody,payment_type,notes\n\
                   2026-01-05,compra,PETR4,,,100,30.5,3050,4.9,XP,,\n";
        let row = parse_anchor_csv(old).remove(0).unwrap();
        assert_eq!((row.loss_credit_used, row.fund_term, row.fund_structure, row.fund_estoque_option), (None, None, None, None));
    }

    #[test]
    fn a_non_numeric_loss_credit_reports_the_line() {
        let text = "date,type,total_value,loss_credit_used\n2026-01-05,come_cotas,30,abc\n";
        let issue = parse_anchor_csv(text).remove(0).unwrap_err();
        assert_eq!(issue.line, 1);
        assert!(issue.message.contains("abc"));
    }

    #[test]
    fn a_csv_missing_required_columns_or_with_bad_values_reports_the_line() {
        let parsed = parse_anchor_csv("foo,bar\n1,2\n");
        assert!(parsed[0].as_ref().unwrap_err().message.contains("date"));

        let parsed = parse_anchor_csv("date,type,ticker,total_value\n2026-01-05,compra,PETR4,100\n05-01-26,compra,PETR4,100\n");
        assert!(parsed[0].is_ok());
        assert_eq!(parsed[1].as_ref().unwrap_err().line, 2);
    }

    /// Roda o leitor contra uma planilha real da B3 cujo caminho vem de
    /// `ANCHOR_B3_SAMPLE`. Só imprime contagens e motivos, nunca valores, e fica
    /// `#[ignore]` porque a amostra é privada e não vai para o repositório.
    #[test]
    #[ignore]
    fn real_b3_sample_parses() {
        let path = std::env::var("ANCHOR_B3_SAMPLE").expect("defina ANCHOR_B3_SAMPLE");
        let parsed = parse_b3_rows(&read_xlsx_rows(&path).expect("planilha abre"));
        let ok = parsed.iter().filter(|r| r.is_ok()).count();
        println!("linhas lidas: {} ok, {} com erro", ok, parsed.len() - ok);
        for issue in parsed.iter().filter_map(|r| r.as_ref().err()) {
            println!("linha {}: {}", issue.line, issue.message);
        }
        for row in parsed.iter().filter_map(|r| r.as_ref().ok()) {
            println!("ticker {} classe inferida {:?}", row.ticker, infer_class(&row.ticker));
        }
    }

    /// Igual ao anterior para a planilha "Movimentação" (`ANCHOR_B3_MOVIMENTACAO`).
    #[test]
    #[ignore]
    fn real_b3_movimentacao_sample_parses() {
        let path = std::env::var("ANCHOR_B3_MOVIMENTACAO").expect("defina ANCHOR_B3_MOVIMENTACAO");
        let parsed = parse_b3_movimentacao_rows(&read_xlsx_rows(&path).expect("planilha abre"), &test_rule_set());
        let ok = parsed.iter().filter(|r| r.is_ok()).count();
        println!("linhas lidas: {} proventos, {} puladas", ok, parsed.len() - ok);
        let by_type = |kind: &str| {
            parsed.iter().filter_map(|r| r.as_ref().ok()).filter(|p| p.payment_type.as_deref() == Some(kind)).count()
        };
        println!("aluguel: {} linhas, reembolso: {} linhas", by_type("aluguel"), by_type("reembolso"));
        let fixed_income = |kind: &str| -> Vec<&ProposedTransaction> {
            parsed
                .iter()
                .filter_map(|r| r.as_ref().ok())
                .filter(|p| p.asset_class.as_deref() == Some("renda_fixa") && p.transaction_type == kind)
                .collect()
        };
        let sells = fixed_income("venda");
        println!(
            "renda fixa: {} compras, {} vendas ao custo ({} com a aplicação fora do arquivo)",
            fixed_income("compra").len(),
            sells.len(),
            sells.iter().filter(|p| p.notes.as_deref().is_some_and(|n| n.contains("não está toda"))).count()
        );
        // JCP: o bruto reconstituído tem de ficar perto de quantidade × preço unitário (o preço é bruto por ação).
        let deviations: Vec<f64> = parsed
            .iter()
            .filter_map(|r| r.as_ref().ok())
            .filter(|p| p.payment_type.as_deref() == Some("jscp"))
            .filter_map(|p| Some((p.total_value / (p.quantity? * p.unit_price?) - 1.0).abs()))
            .collect();
        if !deviations.is_empty() {
            println!(
                "JCP: {} linhas, desvio do bruto reconstituído contra quantidade × preço: máx {:.2}%",
                deviations.len(),
                deviations.iter().cloned().fold(0.0, f64::max) * 100.0
            );
        }
        let mut reasons: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        for issue in parsed.iter().filter_map(|r| r.as_ref().err()) {
            *reasons.entry(issue.message.clone()).or_insert(0) += 1;
        }
        for (reason, count) in reasons {
            println!("{count}x {reason}");
        }
        for row in parsed.iter().filter_map(|r| r.as_ref().ok()) {
            println!("provento de {} ({})", row.ticker, row.payment_type.as_deref().unwrap_or("-"));
        }
    }
}
