// Fase 13.2 — quantidade acumulada por lançamento, pra alimentar a coluna
// "Running qty" da tela Ledger. Função pura, mesmo espírito de
// `domain/dividend_suggestion.rs::quantity_held_by`: as mesmas regras de
// `commands/transaction.rs::compute_positions` (`compra` soma, `venda`
// subtrai, `transferencia`/`aporte`/`retirada`/`provento` não alteram o
// total líquido do ativo), só que aqui devolvendo o valor após CADA
// lançamento, não só o total final.
use std::collections::HashMap;

pub struct LedgerEntry {
    pub id: i32,
    pub asset_id: i32,
    pub transaction_type: String,
    pub quantity: f64,
    pub transaction_date: String,
}

// Empates na mesma data são resolvidos por `id` crescente (ordem de
// inserção) — o banco não guarda timestamp mais fino que a data escolhida
// pelo usuário no formulário.
pub fn running_quantities(entries: &[LedgerEntry]) -> HashMap<i32, f64> {
    let mut sorted: Vec<&LedgerEntry> = entries.iter().collect();
    sorted.sort_by(|a, b| {
        (a.transaction_date.as_str(), a.id).cmp(&(b.transaction_date.as_str(), b.id))
    });

    let mut running_by_asset: HashMap<i32, f64> = HashMap::new();
    let mut result: HashMap<i32, f64> = HashMap::new();
    for entry in sorted {
        let qty = running_by_asset.entry(entry.asset_id).or_insert(0.0);
        match entry.transaction_type.as_str() {
            "compra" => *qty += entry.quantity,
            // O come-cotas tira cotas como uma venda (sem caixa).
            "venda" | "come_cotas" => *qty -= entry.quantity,
            _ => {}
        }
        result.insert(entry.id, *qty);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: i32, asset_id: i32, transaction_type: &str, quantity: f64, date: &str) -> LedgerEntry {
        LedgerEntry {
            id,
            asset_id,
            transaction_type: transaction_type.to_string(),
            quantity,
            transaction_date: date.to_string(),
        }
    }

    #[test]
    fn buy_accumulates() {
        let entries = [entry(1, 1, "compra", 10.0, "2026-01-10")];

        let result = running_quantities(&entries);

        assert_eq!(result[&1], 10.0);
    }

    #[test]
    fn buy_then_sell_reduces_running_total() {
        let entries = [
            entry(1, 1, "compra", 100.0, "2026-01-10"),
            entry(2, 1, "venda", 40.0, "2026-02-01"),
        ];

        let result = running_quantities(&entries);

        assert_eq!(result[&1], 100.0);
        assert_eq!(result[&2], 60.0);
    }

    #[test]
    fn come_cotas_takes_quotas_like_a_sale() {
        let entries = [
            entry(1, 1, "compra", 100.0, "2026-01-10"),
            entry(2, 1, "come_cotas", 2.5, "2026-05-29"),
        ];

        let result = running_quantities(&entries);

        assert_eq!(result[&2], 97.5);
    }

    #[test]
    fn transfer_and_dividend_leave_running_total_unchanged() {
        let entries = [
            entry(1, 1, "compra", 50.0, "2026-01-10"),
            entry(2, 1, "transferencia", 20.0, "2026-01-15"),
            entry(3, 1, "provento", 0.0, "2026-01-20"),
        ];

        let result = running_quantities(&entries);

        assert_eq!(result[&1], 50.0);
        assert_eq!(result[&2], 50.0);
        assert_eq!(result[&3], 50.0);
    }

    #[test]
    fn same_date_ties_broken_by_ascending_id() {
        // Se a ordem fosse a de inserção do array (id 2 antes do id 1), o
        // resultado do id 1 mudaria — confirma que o sort ignora a ordem de
        // entrada e usa (data, id).
        let entries = [
            entry(2, 1, "venda", 10.0, "2026-01-10"),
            entry(1, 1, "compra", 30.0, "2026-01-10"),
        ];

        let result = running_quantities(&entries);

        assert_eq!(result[&1], 30.0);
        assert_eq!(result[&2], 20.0);
    }

    #[test]
    fn unrelated_assets_do_not_interfere() {
        let entries = [
            entry(1, 1, "compra", 10.0, "2026-01-10"),
            entry(2, 2, "compra", 999.0, "2026-01-11"),
        ];

        let result = running_quantities(&entries);

        assert_eq!(result[&1], 10.0);
        assert_eq!(result[&2], 999.0);
    }
}
