import 'dart:math' show pow;

/// Dívida com parcelas geradas — porta de `desktop/src-tauri/src/domain/liability.rs`.
enum AmortizationSystem {
  price('price', 'Price (parcela fixa)'),
  sac('sac', 'SAC (amortização fixa)');

  /// Texto gravado em `sistema_amortizacao`, o mesmo do desktop.
  final String value;
  final String label;

  const AmortizationSystem(this.value, this.label);

  static AmortizationSystem fromValue(String value) =>
      values.firstWhere((s) => s.value == value);
}

class Installment {
  final int numeroParcela;
  final double valorParcela;
  final double valorJuros;
  final double valorAmortizacao;
  final double saldoDevedorApos;

  const Installment({
    required this.numeroParcela,
    required this.valorParcela,
    required this.valorJuros,
    required this.valorAmortizacao,
    required this.saldoDevedorApos,
  });
}

/// Cronograma completo — Price (parcela fixa) ou SAC (amortização fixa,
/// parcela decrescente). `taxaMensal` é decimal (0.01 para 1% a.m.). Lança
/// `ArgumentError` para principal ou prazo não positivos e taxa negativa.
List<Installment> computeInstallments({
  required double principal,
  required double taxaMensal,
  required int prazoMeses,
  required AmortizationSystem sistema,
}) {
  if (!principal.isFinite || principal <= 0) {
    throw ArgumentError('O principal deve ser maior que zero');
  }
  if (!taxaMensal.isFinite || taxaMensal < 0) {
    throw ArgumentError('A taxa mensal não pode ser negativa');
  }
  if (prazoMeses <= 0) {
    throw ArgumentError('O prazo (em meses) deve ser maior que zero');
  }

  final n = prazoMeses;
  final i = taxaMensal;
  final result = <Installment>[];
  var saldo = principal;

  // Parcela fixa; sem juros a fórmula divide por zero, cai em principal / n.
  double pmt() {
    if (i == 0) return principal / n;
    final factor = pow(1 + i, n);
    return principal * (i * factor) / (factor - 1);
  }

  final fixedPayment = sistema == AmortizationSystem.price ? pmt() : 0.0;
  for (var numero = 1; numero <= n; numero++) {
    final juros = saldo * i;
    // A última parcela absorve o resíduo de arredondamento: saldo final zero.
    final amortizacao = numero == n
        ? saldo
        : sistema == AmortizationSystem.price
        ? fixedPayment - juros
        : principal / n;
    saldo -= amortizacao;
    result.add(
      Installment(
        numeroParcela: numero,
        valorParcela: amortizacao + juros,
        valorJuros: juros,
        valorAmortizacao: amortizacao,
        saldoDevedorApos: saldo,
      ),
    );
  }
  return result;
}

/// Saldo devedor de `hoje` (`YYYY-MM-DD`): o principal menos a amortização de
/// toda parcela já vencida (`dataVencimento <= hoje`, comparação de texto).
/// Parcela futura não conta: a dívida só diminui quando a parcela vence.
double outstandingBalance(
  double principal,
  Iterable<({String dataVencimento, double valorAmortizacao})> installments,
  String hoje,
) {
  var amortizado = 0.0;
  for (final i in installments) {
    if (i.dataVencimento.compareTo(hoje) <= 0) amortizado += i.valorAmortizacao;
  }
  return principal - amortizado;
}

/// `date` mais `months` meses, mantendo o dia e recuando para o fim do mês
/// quando ele não existe (31/01 + 1 = 28/02). Porta de `add_months` do desktop.
DateTime addMonths(DateTime date, int months) {
  final total = date.month - 1 + months;
  final year = date.year + (total / 12).floor();
  final month = total % 12 + 1;
  final lastDay = DateTime(year, month + 1, 0).day;
  return DateTime(year, month, date.day < lastDay ? date.day : lastDay);
}
