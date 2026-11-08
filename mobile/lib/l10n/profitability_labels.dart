import '../domain/profitability_error.dart';
import 'app_localizations.dart';

/// Texto do erro da rentabilidade para o usuário, pelo código (ADR 0001). Erro que não é da
/// rentabilidade mostra a própria mensagem.
String profitabilityErrorText(AppLocalizations l, Object error) => switch (error) {
  ProfitabilityError(:final code, :final args) => switch (code) {
    ProfitabilityErrorCode.missingMonthPrice => l.profitErrMissingMonthPrice(
      args['ticker'] ?? '',
      args['month'] ?? '',
    ),
    ProfitabilityErrorCode.missingFx => l.profitErrMissingFx(args['currency'] ?? ''),
    ProfitabilityErrorCode.zeroDenominator => l.profitErrZeroDenominator,
  },
  _ => '$error',
};
