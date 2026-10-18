// Fase 13.4 — agregações puras pra tela "Proventos": IR retido na fonte do
// JSCP (era 15% fixo; desde a Fase 24 vem da regra `jcp_irrf_rate` vigente
// na data do pagamento — 17,5% a partir de 2026, LC 224/2025) e o bucketing mensal
// recebido/a-receber que alimenta o gráfico empilhado e a tabela "Monthly
// history" (o front pivota Ano×Mês e agrega Mensal→Anual em cima disto,
// mesmo padrão 100% client-side de LedgerSection.tsx/DividendHistoryChart.tsx).

use std::collections::HashMap;

pub const PAYMENT_TYPE_DIVIDENDO: &str = "dividendo";
pub const PAYMENT_TYPE_JSCP: &str = "jscp";
/// Renda do aluguel de ações (linhas "Empréstimo" da Movimentação). Sem retenção no app: vale o que a B3 pagou.
pub const PAYMENT_TYPE_ALUGUEL: &str = "aluguel";
/// Compensação de provento pago enquanto as ações estavam emprestadas (linha "Reembolso"). Idem, sem retenção.
pub const PAYMENT_TYPE_REEMBOLSO: &str = "reembolso";

// `None`/qualquer valor != "jscp" cai no ramo sem retenção — cobre tanto
// "dividendo" explícito quanto `transactions.payment_type = NULL`
// (lançamento legado, ver migration). `jscp_rate` = regra vigente na data
// do pagamento, resolvida pelo caller.
pub fn net_total(total: f64, payment_type: Option<&str>, jscp_rate: f64) -> f64 {
    match payment_type {
        Some(PAYMENT_TYPE_JSCP) => total * (1.0 - jscp_rate),
        _ => total,
    }
}

/// Inverso de `net_total` para JCP: o bruto, em centavos, cujo líquido (bruto × (1 − taxa),
/// arredondado) é exatamente o `net` recebido. A B3 informa só o líquido; o app guarda o bruto.
/// Vale a estimativa arredondada; se ela não fecha no centavo, tenta os vizinhos.
pub fn jscp_gross_from_net(net: f64, rate: f64) -> f64 {
    let net_cents = (net * 100.0).round();
    let guess = (net_cents / (1.0 - rate)).round();
    for delta in [0.0, 1.0, -1.0, 2.0, -2.0] {
        let gross_cents = guess + delta;
        if (gross_cents * (1.0 - rate)).round() == net_cents {
            return gross_cents / 100.0;
        }
    }
    guess / 100.0
}

pub struct ProventoEntry {
    pub asset_id: i32,
    pub payment_date: String, // "YYYY-MM-DD"
    pub net_total: f64,
    pub received: bool, // true = pago (transactions), false = a receber (suggested_dividends pendente/matched/divergent)
}

pub struct MonthlyBucket {
    pub year_month: String, // "YYYY-MM"
    pub received: f64,
    pub expected: f64,
}

pub fn bucket_monthly(entries: &[ProventoEntry]) -> Vec<MonthlyBucket> {
    let mut map: HashMap<String, (f64, f64)> = HashMap::new();
    for entry in entries {
        let ym = entry.payment_date.get(0..7).unwrap_or(&entry.payment_date).to_string();
        let bucket = map.entry(ym).or_insert((0.0, 0.0));
        if entry.received {
            bucket.0 += entry.net_total;
        } else {
            bucket.1 += entry.net_total;
        }
    }
    let mut result: Vec<MonthlyBucket> = map
        .into_iter()
        .map(|(year_month, (received, expected))| MonthlyBucket { year_month, received, expected })
        .collect();
    result.sort_by(|a, b| a.year_month.cmp(&b.year_month));
    result
}

// Soma `net_total` por ativo sobre o que o caller já filtrou (recebidos,
// janela de 12 meses) — a filtragem por data/hoje fica no comando, não aqui
// (mesmo padrão de `commands/portfolio_summary.rs`, que calcula `since` com
// `chrono::Utc::now()` fora da função pura).
pub fn by_asset(entries: &[ProventoEntry]) -> HashMap<i32, f64> {
    let mut map: HashMap<i32, f64> = HashMap::new();
    for entry in entries {
        *map.entry(entry.asset_id).or_insert(0.0) += entry.net_total;
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_jscp_gross_rebuilt_from_the_net_nets_back_to_the_same_cents() {
        // Para qualquer líquido em centavos, o bruto reconstituído, descontado a taxa, devolve o líquido.
        for rate in [0.15, 0.175] {
            for net_cents in 1..=5000 {
                let net = net_cents as f64 / 100.0;
                let gross = jscp_gross_from_net(net, rate);
                assert_eq!((gross * (1.0 - rate) * 100.0).round() as i64, net_cents, "líquido {net} à taxa {rate}");
            }
        }
        assert_eq!(jscp_gross_from_net(8.5, 0.15), 10.0);
        assert_eq!(jscp_gross_from_net(12.34, 0.15), 14.52);
    }

    #[test]
    fn net_total_dividendo_has_no_withholding() {
        assert_eq!(net_total(100.0, Some("dividendo"), 0.175), 100.0);
    }

    #[test]
    fn net_total_jscp_withholds_the_given_rate() {
        assert_eq!(net_total(100.0, Some("jscp"), 0.15), 85.0);
        assert_eq!(net_total(100.0, Some("jscp"), 0.175), 82.5);
    }

    #[test]
    fn net_total_aluguel_and_reembolso_have_no_withholding() {
        assert_eq!(net_total(100.0, Some(PAYMENT_TYPE_ALUGUEL), 0.175), 100.0);
        assert_eq!(net_total(100.0, Some(PAYMENT_TYPE_REEMBOLSO), 0.175), 100.0);
    }

    #[test]
    fn net_total_none_defaults_to_dividendo() {
        assert_eq!(net_total(100.0, None, 0.175), 100.0);
    }

    fn entry(asset_id: i32, date: &str, net_total: f64, received: bool) -> ProventoEntry {
        ProventoEntry { asset_id, payment_date: date.to_string(), net_total, received }
    }

    #[test]
    fn bucket_monthly_separates_received_and_expected() {
        let entries = [
            entry(1, "2026-01-10", 100.0, true),
            entry(1, "2026-01-20", 50.0, false),
        ];
        let result = bucket_monthly(&entries);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].year_month, "2026-01");
        assert_eq!(result[0].received, 100.0);
        assert_eq!(result[0].expected, 50.0);
    }

    #[test]
    fn bucket_monthly_sums_same_month_across_assets() {
        let entries = [entry(1, "2026-02-05", 10.0, true), entry(2, "2026-02-15", 20.0, true)];
        let result = bucket_monthly(&entries);
        assert_eq!(result[0].received, 30.0);
    }

    #[test]
    fn bucket_monthly_sorted_ascending() {
        let entries = [entry(1, "2026-03-01", 1.0, true), entry(1, "2026-01-01", 1.0, true)];
        let result = bucket_monthly(&entries);
        assert_eq!(result[0].year_month, "2026-01");
        assert_eq!(result[1].year_month, "2026-03");
    }

    #[test]
    fn by_asset_sums_only_passed_entries() {
        let entries = [
            entry(1, "2026-01-01", 10.0, true),
            entry(1, "2026-02-01", 5.0, true),
            entry(2, "2026-01-01", 7.0, true),
        ];
        let result = by_asset(&entries);
        assert_eq!(result[&1], 15.0);
        assert_eq!(result[&2], 7.0);
    }
}
