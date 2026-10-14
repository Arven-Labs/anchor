use crate::error::AppError;

pub struct CashFlow {
    pub amount: f64,
    pub weight: f64,
}

// R_mes = (EMV - BMV - CF_total) / (BMV + Σ(CF_i × W_i))
// Método Dietz Modificado (Fase 10.3) — `weight` de cada fluxo já vem
// calculado pelo chamador (dias restantes no mês / dias no mês), esta
// função só soma e divide.
pub fn modified_dietz(bmv: f64, emv: f64, cash_flows: &[CashFlow]) -> Result<f64, AppError> {
    let cf_total: f64 = cash_flows.iter().map(|cf| cf.amount).sum();
    let weighted_cf: f64 = cash_flows.iter().map(|cf| cf.amount * cf.weight).sum();
    let denominator = bmv + weighted_cf;

    if denominator == 0.0 {
        return Err(AppError::InvalidGuard(
            "Modified Dietz denominator is zero (BMV and weighted cash flows cancel out)"
                .to_string(),
        ));
    }

    Ok((emv - bmv - cf_total) / denominator)
}

// Sinal de cada fluxo externo no Dietz acima, onde CF > 0 = dinheiro que
// ENTRA na carteira medida (reduz o retorno, já que o EMV cresce sem ganho).
// Fase 24 (Sessão 101): corrige o provento, que até aqui entrava como +CF
// e por isso DERRUBAVA o retorno (BMV 100, EMV 100, provento 5 → ≈ −5% em
// vez de +5%) — contrário à intenção da Sessão 35 ("provento soma ao
// retorno"). Provento é caixa que SAI da carteira medida pro investidor
// (o EMV não inclui caixa), então é −CF; imposto pago é custo sem ativo
// correspondente, então é +CF.
pub enum FlowKind {
    /// Compra — o ativo "aparece" na sub-carteira (equivale a um aporte).
    Contribution,
    /// Venda — o valor sai da sub-carteira.
    Withdrawal,
    /// Provento recebido.
    Income,
    /// Imposto pago sobre a carteira.
    Cost,
}

pub fn flow_amount(kind: FlowKind, value: f64) -> f64 {
    match kind {
        FlowKind::Contribution | FlowKind::Cost => value,
        FlowKind::Withdrawal | FlowKind::Income => -value,
    }
}

// R_total = Π(1 + R_mes_i) − 1 — geometricamente componível.
pub fn chain(monthly_returns: &[f64]) -> f64 {
    monthly_returns.iter().fold(1.0, |acc, r| acc * (1.0 + r)) - 1.0
}

/// Rebaseia uma série de retorno acumulado (`r_cumulative_pct`, em pontos
/// percentuais) pros últimos 12 meses — mesma lógica de
/// `SummarySection.tsx::trailingTwelveMonthPct` (Fase 13.1), portada pro
/// backend porque aqui o consumidor (Fase 12, spread por dívida vinculada)
/// precisa de um número por ativo, não de uma série pra gráfico. Diferença
/// deliberada do precedente client-side: lá, com ≤12 meses de série, o
/// retorno acumulado "do que existir" é devolvido; aqui, como a coluna é
/// rotulada explicitamente "12M", menos de 13 pontos mensais (= menos de 12
/// transições mês-a-mês) devolve `None` em vez de um número mal rotulado.
pub fn trailing_twelve_month_pct(cumulative_pct: &[f64]) -> Option<f64> {
    if cumulative_pct.len() < 13 {
        return None;
    }
    let end_index = 1.0 + cumulative_pct[cumulative_pct.len() - 1] / 100.0;
    let base_index = 1.0 + cumulative_pct[cumulative_pct.len() - 13] / 100.0;
    Some((end_index / base_index - 1.0) * 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_phase_md_worked_example_january() {
        let cash_flows = [CashFlow {
            amount: 2000.0,
            weight: 21.0 / 31.0,
        }];

        let r = modified_dietz(10000.0, 12500.0, &cash_flows).unwrap();

        assert!((r - 0.04403409090909091).abs() < 1e-9);
    }

    #[test]
    fn matches_phase_md_worked_example_february_no_flows() {
        let r = modified_dietz(12500.0, 12250.0, &[]).unwrap();

        assert!((r - (-0.02)).abs() < 1e-9);
    }

    #[test]
    fn chain_composes_january_and_february() {
        let total = chain(&[0.04403409090909091, -0.02]);

        assert!((total - 0.023153409090908905).abs() < 1e-9);
    }

    #[test]
    fn income_raises_return() {
        // Carteira parada (BMV = EMV = 100) que pagou 5 de provento no fim do
        // mês (peso 0): retorno tem que ser +5%, não −5%.
        let cash_flows = [CashFlow {
            amount: flow_amount(FlowKind::Income, 5.0),
            weight: 0.0,
        }];

        let r = modified_dietz(100.0, 100.0, &cash_flows).unwrap();

        assert!((r - 0.05).abs() < 1e-9);
    }

    #[test]
    fn cost_lowers_return() {
        // Mesma carteira parada, pagou 3 de imposto: retorno −3%.
        let cash_flows = [CashFlow {
            amount: flow_amount(FlowKind::Cost, 3.0),
            weight: 0.0,
        }];

        let r = modified_dietz(100.0, 100.0, &cash_flows).unwrap();

        assert!((r - (-0.03)).abs() < 1e-9);
    }

    #[test]
    fn rejects_degenerate_zero_denominator() {
        assert!(matches!(
            modified_dietz(0.0, 0.0, &[]),
            Err(AppError::InvalidGuard(_))
        ));
    }

    #[test]
    fn trailing_twelve_month_pct_needs_at_least_thirteen_points() {
        assert_eq!(trailing_twelve_month_pct(&[]), None);
        let twelve_points = vec![1.0; 12];
        assert_eq!(trailing_twelve_month_pct(&twelve_points), None);
    }

    #[test]
    fn trailing_twelve_month_pct_rebases_last_thirteen_points() {
        // 13 pontos de retorno acumulado: base (12 meses atrás) = 21.0%,
        // fim (agora) = 30.2% -> rebase = (1.302 / 1.21) - 1 ~= 7.6033%
        let mut series = vec![21.0];
        series.extend(vec![0.0; 11]); // pontos intermediários, irrelevantes pro cálculo
        series.push(30.2);
        assert_eq!(series.len(), 13);

        let result = trailing_twelve_month_pct(&series).unwrap();
        assert!((result - 7.603305785123968).abs() < 1e-9);
    }
}
