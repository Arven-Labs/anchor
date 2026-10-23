// Fase 13.6 — reconstrução de quantidade histórica pra gerar sugestões de
// provento a partir de `stock_dividend_payments` (só o "passado" no estado
// atual da fonte, ver PHASE.md). Função pura, mesmo espírito de `domain/twr.rs`:
// o comando separa os lançamentos por ativo e esta função só soma quanto o
// dono do projeto tinha do ativo numa data (data de pagamento do provento).
//
// Mesmas regras de `commands/transaction.rs::get_portfolio_positions`: só
// `compra` e `venda` movem a quantidade líquida; `transferencia` move entre
// custódias sem alterar o total do portfolio; `aporte`/`retirada`/`provento`
// não afetam posição (fluxo de caixa / caixa recebido).
pub fn quantity_held_by(txs: &[LedgerRow], cutoff_date: &str) -> f64 {
    txs.iter()
        .filter(|tx| tx.transaction_date.as_str() <= cutoff_date)
        .map(|tx| match tx.transaction_type.as_str() {
            "compra" => tx.quantity,
            // O come-cotas tira cotas como uma venda (sem caixa).
            "venda" | "come_cotas" => -tx.quantity,
            _ => 0.0,
        })
        .sum()
}

// Faixa mínima do lançamento que a reconstrução precisa, desacoplada do
// `Model` do SeaORM pra função continuar pura/testável sem banco.
pub struct LedgerRow {
    pub transaction_type: String,
    pub quantity: f64,
    pub transaction_date: String,
}

// Conciliação de provento esperado (registrado manualmente) com o valor que
// a fonte automática trouxe de verdade: considera que "bateu" se a diferença
// relativa entre os dois valores/cota ficou dentro da tolerância percentual
// (decisão da Sessão 77: ±10%). O valor de referência é o `expected` (o que
// o dono do projeto previu), não o da fonte.
pub fn within_tolerance(expected: f64, actual: f64, tolerance_pct: f64) -> bool {
    if expected == 0.0 {
        return false;
    }
    // Epsilon de ~0.0000001% absorve o ruído de ponto flutuante no limite
    // exato (ex.: 0.55 − 0.50 computa 0.10000000000000009 > 0.10).
    let relative = (actual - expected).abs() / expected.abs();
    relative <= tolerance_pct / 100.0 + 1e-9
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(transaction_type: &str, quantity: f64, transaction_date: &str) -> LedgerRow {
        LedgerRow {
            transaction_type: transaction_type.to_string(),
            quantity,
            transaction_date: transaction_date.to_string(),
        }
    }

    #[test]
    fn come_cotas_reduces_the_quantity_held() {
        let txs = [row("compra", 100.0, "2026-01-10"), row("come_cotas", 2.5, "2026-05-29")];

        assert_eq!(quantity_held_by(&txs, "2026-05-28"), 100.0);
        assert_eq!(quantity_held_by(&txs, "2026-06-01"), 97.5);
    }

    #[test]
    fn holds_nothing_before_any_transaction() {
        let txs = [row("compra", 10.0, "2026-01-10")];

        assert_eq!(quantity_held_by(&txs, "2026-01-01"), 0.0);
    }

    #[test]
    fn buy_counted_only_when_before_or_on_cutoff() {
        let txs = [row("compra", 10.0, "2026-01-10")];

        assert_eq!(quantity_held_by(&txs, "2026-01-09"), 0.0);
        assert_eq!(quantity_held_by(&txs, "2026-01-10"), 10.0);
        assert_eq!(quantity_held_by(&txs, "2026-01-11"), 10.0);
    }

    #[test]
    fn partial_sell_reduces_holding() {
        let txs = [
            row("compra", 100.0, "2026-01-10"),
            row("venda", 40.0, "2026-02-01"),
        ];

        assert_eq!(quantity_held_by(&txs, "2026-01-31"), 100.0);
        assert_eq!(quantity_held_by(&txs, "2026-02-01"), 60.0);
    }

    #[test]
    fn sell_all_leaves_zero() {
        let txs = [row("compra", 50.0, "2026-01-10"), row("venda", 50.0, "2026-03-01")];

        assert_eq!(quantity_held_by(&txs, "2026-03-01"), 0.0);
    }

    #[test]
    fn non_position_types_do_not_move_quantity() {
        let txs = [
            row("compra", 10.0, "2026-01-10"),
            row("aporte", 999.0, "2026-01-11"),
            row("retirada", 999.0, "2026-01-12"),
            row("provento", 999.0, "2026-01-13"),
            row("transferencia", 999.0, "2026-01-14"),
        ];

        assert_eq!(quantity_held_by(&txs, "2026-01-14"), 10.0);
    }

    #[test]
    fn transfer_does_not_change_total_but_other_rows_can_overlap() {
        let txs = [
            row("compra", 10.0, "2026-01-10"),
            row("transferencia", 4.0, "2026-02-01"),
            row("venda", 3.0, "2026-02-02"),
        ];

        assert_eq!(quantity_held_by(&txs, "2026-02-01"), 10.0);
        assert_eq!(quantity_held_by(&txs, "2026-02-02"), 7.0);
    }

    #[test]
    fn tolerance_matches_exact_value() {
        assert!(within_tolerance(0.50, 0.50, 10.0));
    }

    #[test]
    fn tolerance_accepts_relative_difference_inside_margin() {
        assert!(within_tolerance(0.50, 0.45, 10.0));
        assert!(within_tolerance(0.50, 0.55, 10.0));
        assert!(within_tolerance(0.50, 0.549, 10.0));
    }

    #[test]
    fn tolerance_rejects_difference_outside_margin() {
        assert!(!within_tolerance(0.50, 0.44, 10.0));
        assert!(!within_tolerance(0.50, 0.56, 10.0));
    }

    #[test]
    fn tolerance_handles_zero_expected_as_never_matching() {
        assert!(!within_tolerance(0.0, 0.0, 10.0));
        assert!(!within_tolerance(0.0, 0.5, 10.0));
    }
}