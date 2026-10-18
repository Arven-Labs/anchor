// Liquidação da B3 como caixa da conta Corretora. A planilha "Movimentação" traz uma linha
// `Transferência - Liquidação` por ativo e por dia: a quantidade e o valor bruto do que o ativo
// somou nos negócios daquele dia, liquidados dois dias úteis depois (na amostra de 2025, 53 das 64
// linhas com valor casam com a soma dos negócios da "Negociação" a até R$ 0,12, sem taxa nenhuma).
// A saída do banco não bate com a liquidação (o dinheiro vai para a corretora em aportes soltos),
// então o caixa da corretora vive numa conta própria: o aporte do banco entra como transferência
// (`statement_io`, `Transferência enviada/recebida`), e cada liquidação vira um lançamento dessa conta.
//
// `Credito` é a compra (o ativo entra na custódia, o dinheiro sai da corretora) e `Debito` é a venda.
// Os negócios continuam vindo da "Negociação"; aqui só o dinheiro. Cada linha vai para a mesma fila de
// revisão do extrato (`bank_transaction`), com a data da liquidação, que é quando o caixa se move.
use sea_orm::DatabaseConnection;

use crate::commands::portfolio_io::{
    movimentacao_columns, normalize_header, parse_date, parse_number, read_xlsx_rows, ParseIssue, MOVIMENTACAO_COLUMNS_MESSAGE,
};
use crate::commands::statement_io::{stage_statement_parsed, ParsedStatement, ProposedBankTransaction, StatementStageResult};
use crate::error::AppError;

const SOURCE: &str = "b3_settlement";

/// Lê as linhas de liquidação da planilha "Movimentação". Compra sai do caixa (valor negativo) e venda entra.
pub fn parse_b3_settlements(rows: &[Vec<String>]) -> ParsedStatement {
    let Some(header) = rows.first() else {
        return statement(vec![Err(ParseIssue { line: 0, message: "planilha vazia".to_string() })]);
    };
    let Some(columns) = movimentacao_columns(header) else {
        return statement(vec![Err(ParseIssue { line: 0, message: MOVIMENTACAO_COLUMNS_MESSAGE.to_string() })]);
    };
    let mut out = Vec::new();
    for (line, row) in rows.iter().enumerate().skip(1) {
        let get = |i: usize| row.get(i).map(String::as_str).unwrap_or("");
        if !normalize_header(get(columns.movement)).contains("liquidacao") {
            continue;
        }
        let issue = |message: String| ParseIssue { line, message };
        let flow = normalize_header(get(columns.flow));
        let is_buy = match flow.as_str() {
            "credito" => true,
            "debito" => false,
            other => {
                out.push(Err(issue(format!("liquidação com sentido '{other}' (esperado crédito ou débito)"))));
                continue;
            }
        };
        let Some(date) = parse_date(get(columns.date)) else {
            out.push(Err(issue(format!("data inválida: '{}'", get(columns.date).trim()))));
            continue;
        };
        // Sem valor é a liquidação do aluguel (só move a custódia): não há dinheiro a lançar.
        let value = match parse_number(get(columns.value)) {
            Ok(Some(v)) if v.is_finite() && v > 0.0 => v,
            Ok(_) => {
                out.push(Err(issue("liquidação sem valor (movimento de custódia, como o aluguel) pulada".to_string())));
                continue;
            }
            Err(message) => {
                out.push(Err(issue(message)));
                continue;
            }
        };
        let ticker = get(columns.product).split(" - ").next().unwrap_or("").trim().to_string();
        let quantity = get(columns.quantity).trim();
        out.push(Ok(ProposedBankTransaction {
            line,
            transaction_date: date,
            valor: if is_buy { -value } else { value },
            label: if is_buy { "Compra" } else { "Venda" }.to_string(),
            description: format!("{ticker} · {quantity}"),
            reference: None,
            porquinho: false,
            broker: false,
        }));
    }
    if out.is_empty() {
        out.push(Err(ParseIssue { line: 0, message: "não achei nenhuma liquidação (Transferência - Liquidação) na planilha".to_string() }));
    }
    statement(out)
}

fn statement(rows: Vec<Result<ProposedBankTransaction, ParseIssue>>) -> ParsedStatement {
    ParsedStatement { rows, account_ref: None }
}

pub async fn stage_b3_settlements_in(
    db: &DatabaseConnection,
    workspace_id: i32,
    broker_account_id: i32,
    path: &str,
) -> Result<StatementStageResult, AppError> {
    let rows = read_xlsx_rows(path)?;
    stage_statement_parsed(db, workspace_id, broker_account_id, None, false, SOURCE, parse_b3_settlements(&rows)).await
}

#[tauri::command]
pub async fn stage_b3_settlements(
    db: tauri::State<'_, DatabaseConnection>,
    workspace_id: i32,
    broker_account_id: i32,
    path: String,
) -> Result<StatementStageResult, AppError> {
    stage_b3_settlements_in(db.inner(), workspace_id, broker_account_id, &path).await
}

#[cfg(test)]
mod tests {
    use super::*;

    // Dados inventados: nenhum código, valor ou data de planilha real.
    const HEADER: [&str; 8] =
        ["Entrada/Saída", "Data", "Movimentação", "Produto", "Instituição", "Quantidade", "Preço unitário", "Valor da Operação"];

    fn sheet(rows: &[[&str; 8]]) -> Vec<Vec<String>> {
        std::iter::once(HEADER.to_vec()).chain(rows.iter().map(|r| r.to_vec())).map(|r| r.into_iter().map(String::from).collect()).collect()
    }

    #[test]
    fn buys_leave_the_broker_cash_and_sells_enter_it() {
        let rows = sheet(&[
            ["Credito", "17/12/2025", "Transferência - Liquidação", "ABCD3 - EMPRESA S.A.", "CORRETORA", "100", "9", "900"],
            ["Debito", "18/12/2025", "Transferência - Liquidação", "ABCD3 - EMPRESA S.A.", "CORRETORA", "40", "10", "400"],
            ["Credito", "19/12/2025", "Dividendo", "ABCD3 - EMPRESA S.A.", "CORRETORA", "100", "0.1", "10"],
        ]);
        let parsed = parse_b3_settlements(&rows).rows;
        assert_eq!(parsed.len(), 2, "só liquidação vira lançamento");
        let buy = parsed[0].as_ref().unwrap();
        assert_eq!((buy.transaction_date.as_str(), buy.valor, buy.label.as_str()), ("2025-12-17", -900.0, "Compra"));
        assert_eq!(buy.description, "ABCD3 · 100");
        let sell = parsed[1].as_ref().unwrap();
        assert_eq!((sell.transaction_date.as_str(), sell.valor, sell.label.as_str()), ("2025-12-18", 400.0, "Venda"));
    }

    #[test]
    fn a_settlement_without_value_is_skipped_with_the_reason() {
        let rows = sheet(&[["Debito", "17/12/2025", "Transferência - Liquidação", "ABCD4 - EMPRESA S.A.", "CORRETORA", "167", "-", "-"]]);
        let parsed = parse_b3_settlements(&rows).rows;
        assert_eq!(parsed.len(), 1);
        assert!(parsed[0].as_ref().unwrap_err().message.contains("sem valor"));
    }

    #[test]
    fn a_sheet_without_settlements_or_with_other_columns_says_so() {
        let none = parse_b3_settlements(&sheet(&[["Credito", "19/12/2025", "Dividendo", "ABCD3 - X", "C", "1", "1", "1"]]));
        assert!(none.rows[0].as_ref().unwrap_err().message.contains("nenhuma liquidação"));
        let wrong = parse_b3_settlements(&[vec!["a".to_string(), "b".to_string()]]);
        assert!(wrong.rows[0].as_ref().unwrap_err().message.contains("Movimentação"));
    }
}
