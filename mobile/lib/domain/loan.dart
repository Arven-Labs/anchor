// Empréstimos concedidos a terceiros — porta de
// `desktop/src-tauri/src/domain/loan.rs`. Só funções puras: juros informativos,
// saldo a receber e "extra recebido". Nada disso é gravado, o saldo é sempre
// derivado.

/// Meses completos entre `inicio` e `hoje` (0 se `hoje` não passa de
/// `inicio`). Dia do mês ainda não atingido não conta o mês corrente.
int fullMonthsBetween(DateTime inicio, DateTime hoje) {
  if (!hoje.isAfter(inicio)) return 0;
  var months = (hoje.year - inicio.year) * 12 + hoje.month - inicio.month;
  if (hoje.day < inicio.day) months--;
  return months < 0 ? 0 : months;
}

/// Juros **simples** sobre o principal, por meses completos, à taxa mensal em
/// percentual cru (1.5 = 1,5% a.m.). Sem taxa ou com taxa ≤ 0 não há juros: o
/// empréstimo informal em família não tem taxa contratada. É um número
/// informativo; o usuário baixa o valor que quiser.
double accruedInterest({
  required double principal,
  required double? taxaMensalPercentual,
  required DateTime dataInicio,
  required DateTime hoje,
}) {
  final taxa = taxaMensalPercentual;
  if (taxa == null || taxa <= 0) return 0;
  return principal * (taxa / 100) * fullMonthsBetween(dataInicio, hoje);
}

/// Saldo a receber = principal + juros − o já baixado. Nunca negativo: baixar
/// mais do que o devido quita o empréstimo, não gera saldo a pagar.
double loanOutstandingBalance(
  double principal,
  double interest,
  Iterable<double> valoresBaixados,
) {
  final baixado = valoresBaixados.fold<double>(0, (sum, v) => sum + v);
  final saldo = principal + interest - baixado;
  return saldo < 0 ? 0 : saldo;
}

/// Quanto entrou a mais (ou a menos) do que foi baixado, somando todas as
/// baixas (pares recebido, baixado). Positivo = "um pouquinho a mais" que a
/// pessoa devolveu.
double extraReceived(
  Iterable<({double recebido, double baixado})> repayments,
) => repayments.fold<double>(0, (sum, r) => sum + r.recebido - r.baixado);
