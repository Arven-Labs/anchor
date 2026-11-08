import 'profitability_error.dart';

// Porta de `desktop/src-tauri/src/domain/twr.rs` (Fase 10.3): retorno por Dietz Modificado e
// encadeamento geométrico. Sem o custo de imposto (`FlowKind::Cost`): a TWR do mobile é bruta.

/// Um fluxo externo do mês. [weight] é a fração do mês em que o dinheiro ficou investido
/// (dias restantes ÷ dias do mês) e já vem calculado por quem chama.
class CashFlow {
  final double amount;
  final double weight;

  const CashFlow({required this.amount, required this.weight});
}

/// Natureza do fluxo, que decide o sinal no Dietz (CF > 0 = dinheiro que ENTRA na carteira
/// medida): provento é caixa que SAI para o investidor, então é negativo (o bug do desktop
/// da Sessão 101 era tratá-lo como positivo e derrubar o retorno).
enum FlowKind {
  /// Compra: o ativo "aparece" na carteira medida, equivale a um aporte.
  contribution,

  /// Venda: o valor sai da carteira medida.
  withdrawal,

  /// Provento recebido.
  income,
}

double flowAmount(FlowKind kind, double value) => switch (kind) {
  FlowKind.contribution => value,
  FlowKind.withdrawal || FlowKind.income => -value,
};

/// `R = (EMV − BMV − ΣCF) ÷ (BMV + Σ CF × peso)`. Denominador zero é recusado.
double modifiedDietz(double bmv, double emv, List<CashFlow> cashFlows) {
  var cfTotal = 0.0;
  var weightedCf = 0.0;
  for (final cf in cashFlows) {
    cfTotal += cf.amount;
    weightedCf += cf.amount * cf.weight;
  }
  final denominator = bmv + weightedCf;
  if (denominator == 0.0) {
    throw ProfitabilityError(ProfitabilityErrorCode.zeroDenominator);
  }
  return (emv - bmv - cfTotal) / denominator;
}

/// `R_total = Π(1 + R_mês) − 1`: geometricamente componível, não é a soma dos meses.
double chain(List<double> monthlyReturns) =>
    monthlyReturns.fold(1.0, (acc, r) => acc * (1.0 + r)) - 1.0;

/// Rebaseia o retorno acumulado (em pontos percentuais) para os últimos 12 meses. Com menos
/// de 13 pontos (menos de 12 transições mês a mês) devolve `null`, em vez de um número
/// rotulado "12M" que não é.
double? trailingTwelveMonthPct(List<double> cumulativePct) {
  if (cumulativePct.length < 13) return null;
  final end = 1.0 + cumulativePct[cumulativePct.length - 1] / 100.0;
  final base = 1.0 + cumulativePct[cumulativePct.length - 13] / 100.0;
  return (end / base - 1.0) * 100.0;
}
