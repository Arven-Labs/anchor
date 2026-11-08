// Erros da rentabilidade (TWR) com código e argumentos, no padrão de `finance_error.dart`
// (ADR 0001): o texto que o usuário lê vem dos `.arb` pelo código (na tela); a mensagem da
// exceção é inglês para quem depura.

enum ProfitabilityErrorCode {
  /// BMV e fluxos ponderados se anulam: o Dietz Modificado não tem denominador.
  zeroDenominator,

  /// Falta o fechamento mensal de um ativo que está em carteira no fim do mês.
  missingMonthPrice,

  /// Falta a série de câmbio de uma moeda usada por ativo ou fluxo.
  missingFx,
}

String _developerMessage(ProfitabilityErrorCode code, Map<String, String> args) => switch (code) {
  ProfitabilityErrorCode.zeroDenominator =>
    'Modified Dietz denominator is zero (BMV and weighted cash flows cancel out)',
  ProfitabilityErrorCode.missingMonthPrice =>
    'No monthly close for ${args['ticker']} in ${args['month']}: update the price history',
  ProfitabilityErrorCode.missingFx => 'No exchange-rate series for ${args['currency']}',
};

class ProfitabilityError extends StateError {
  final ProfitabilityErrorCode code;
  final Map<String, String> args;

  ProfitabilityError._(this.code, this.args, String message) : super(message);

  ProfitabilityError(ProfitabilityErrorCode code, [Map<String, String> args = const {}])
    : this._(code, args, _developerMessage(code, args));
}
