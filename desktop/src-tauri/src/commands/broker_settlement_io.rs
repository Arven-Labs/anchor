// Liquidação da B3 como caixa da conta Corretora. A planilha "Movimentação" traz uma linha
// `Transferência - Liquidação` por ativo e por dia: a quantidade e o valor bruto do que o ativo
// somou nos negócios daquele dia, liquidados dois dias úteis depois (na amostra de 2025, 53 das 64
// linhas com valor casam com a soma dos negócios da "Negociação" a até R$ 0,12, sem taxa nenhuma).
// A saída do banco não bate com a liquidação (o dinheiro vai para a corretora em aportes soltos),
// então o caixa da corretora vive numa conta própria: o aporte do banco entra como transferência
// (`statement_io`, `Transferência enviada/recebida`), e cada liquidação vira um lançamento dessa conta.
//
// `Credito` é a compra (o ativo entra na custódia, o dinheiro sai da corretora) e `Debito` é a venda.
// Os proventos em dinheiro da mesma planilha (dividendo, rendimento, JCP, pagamento do aluguel e reembolso)
// também creditam a Corretora, pelo valor creditado (o JCP e o aluguel já vêm líquidos de IR). A posição e o
// provento da carteira não mexem no caixa, então sem isto o saldo da Corretora ficaria abaixo do real.
// Os negócios continuam vindo da "Negociação"; aqui só o dinheiro. Cada linha vai para a mesma fila de
// revisão do extrato (`bank_transaction`), com a data da liquidação ou do pagamento, que é quando o caixa se move.
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
        let movement = normalize_header(get(columns.movement));
        let is_settlement = movement.contains("liquidacao");
        let is_proceed = movement == "dividendo"
            || movement == "rendimento"
            || movement.contains("juros sobre capital")
            || movement.contains("emprestimo")
            || movement.contains("reembolso");
        if !is_settlement && !is_proceed {
            continue;
        }
        let issue = |message: String| ParseIssue { line, message };
        let flow = normalize_header(get(columns.flow));
        // Liquidação: `Credito` é a compra (sai dinheiro) e `Debito` a venda. Provento: sempre `Credito`, entra dinheiro.
        let is_buy = match (is_settlement, flow.as_str()) {
            (true, "credito") => true,
            (true, "debito") => false,
            (false, "credito") => false,
            (_, other) => {
                out.push(Err(issue(format!("movimentação com sentido '{other}' (esperado crédito{})", if is_settlement { " ou débito" } else { "" }))));
                continue;
            }
        };
        let Some(date) = parse_date(get(columns.date)) else {
            out.push(Err(issue(format!("data inválida: '{}'", get(columns.date).trim()))));
            continue;
        };
        // Sem valor é o movimento do aluguel (só move a custódia): não há dinheiro a lançar.
        let value = match parse_number(get(columns.value)) {
            Ok(Some(v)) if v.is_finite() && v > 0.0 => v,
            Ok(_) => {
                out.push(Err(issue("movimento sem valor (custódia, como o aluguel) pulado".to_string())));
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
            label: match (is_settlement, is_buy) {
                (true, true) => "Compra",
                (true, false) => "Venda",
                (false, _) => "Provento",
            }
            .to_string(),
            description: format!("{ticker} · {quantity}"),
            reference: None,
            porquinho: false,
            broker: false,
        }));
    }
    if out.is_empty() {
        out.push(Err(ParseIssue { line: 0, message: "não achei nenhuma liquidação nem provento na planilha".to_string() }));
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
        assert_eq!(parsed.len(), 3, "liquidação e provento viram lançamento");
        let proceed = parsed[2].as_ref().unwrap();
        assert_eq!((proceed.transaction_date.as_str(), proceed.valor, proceed.label.as_str()), ("2025-12-19", 10.0, "Provento"));
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
    fn every_kind_of_cash_proceed_credits_the_broker_and_a_rental_custody_line_does_not() {
        let rows = sheet(&[
            ["Credito", "02/12/2025", "Dividendo", "ABCD3 - EMPRESA S.A.", "CORRETORA", "10", "1", "10"],
            ["Credito", "02/12/2025", "Rendimento", "FIIX11 - FUNDO", "CORRETORA", "10", "1", "10"],
            ["Credito", "02/12/2025", "Juros Sobre Capital Próprio", "ABCD3 - EMPRESA S.A.", "CORRETORA", "10", "1", "8.5"],
            ["Credito", "02/12/2025", "Empréstimo", "ABCD3 - EMPRESA S.A.", "CORRETORA", "10", "-", "0.5"],
            ["Credito", "02/12/2025", "Reembolso", "ABCD3 - EMPRESA S.A.", "CORRETORA", "10", "1", "3"],
            ["Credito", "02/12/2025", "Empréstimo", "ABCD3 - EMPRESA S.A.", "CORRETORA", "10", "-", "-"],
            ["Debito", "02/12/2025", "Dividendo", "ABCD3 - EMPRESA S.A.", "CORRETORA", "10", "1", "10"],
        ]);
        let parsed = parse_b3_settlements(&rows).rows;
        let good: Vec<f64> = parsed.iter().filter_map(|r| r.as_ref().ok()).map(|p| p.valor).collect();
        assert_eq!(good, vec![10.0, 10.0, 8.5, 0.5, 3.0], "o valor creditado, positivo, no mesmo dia");
        let errors: Vec<&str> = parsed.iter().filter_map(|r| r.as_ref().err()).map(|e| e.message.as_str()).collect();
        assert!(errors[0].contains("sem valor") && errors[1].contains("esperado crédito"), "{errors:?}");
    }

    #[test]
    fn a_sheet_without_settlements_or_with_other_columns_says_so() {
        let none = parse_b3_settlements(&sheet(&[["Credito", "19/12/2025", "Compra", "ABCD3 - X", "C", "1", "1", "1"]]));
        assert!(none.rows[0].as_ref().unwrap_err().message.contains("nenhuma liquidação nem provento"));
        let wrong = parse_b3_settlements(&[vec!["a".to_string(), "b".to_string()]]);
        assert!(wrong.rows[0].as_ref().unwrap_err().message.contains("Movimentação"));
    }
}
