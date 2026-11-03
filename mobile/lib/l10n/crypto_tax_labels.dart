import '../domain/crypto_tax.dart';
import '../domain/tax_format.dart';
import '../models/crypto_swap.dart';
import 'app_localizations.dart';

// Textos do IR de cripto e da troca de cripto (ADR 0001). Um `switch` sem `default` nos
// enums faz o analisador acusar um valor novo sem texto.

/// Situação do mês numa frase: isento, sem imposto, abaixo do mínimo ou DARF.
String cryptoMonthStatus(AppLocalizations l, CryptoMonth month) {
  final locale = l.localeName;
  if (month.exempt) {
    return l.cryptoTaxStatusExempt(formatBrl(cryptoMonthlyExemption, locale: locale));
  }
  if (month.due > 0) {
    return l.cryptoTaxStatusDarf(
      month.darfCode,
      formatBrl(month.due, locale: locale),
      formatDay(month.dueDate, locale: locale),
    );
  }
  if (month.carriedOut > 0) {
    return l.cryptoTaxStatusBelowMinimum(
      formatBrl(month.carriedOut, locale: locale),
      formatBrl(darfMinimum, locale: locale),
    );
  }
  return l.cryptoTaxStatusNoTax;
}

String cryptoWarningText(AppLocalizations l, CryptoTaxWarning warning) => switch (warning.kind) {
  CryptoTaxWarningKind.costUnknown => l.cryptoTaxWarnCostUnknown(
    warning.ticker!,
    formatDay(warning.date!, locale: l.localeName),
  ),
  CryptoTaxWarningKind.salesNonBrl => l.cryptoTaxWarnSalesNonBrl(formatPeriod(warning.period!)),
  CryptoTaxWarningKind.fxMissing => l.cryptoTaxWarnFxMissing(formatPeriod(warning.period!)),
};

String cryptoSwapErrorText(AppLocalizations l, CryptoSwapException error) {
  String arg(String key) => error.args[key] ?? '';
  return switch (error.code) {
    CryptoSwapErrorCode.needTwoDifferentCoins => l.swapErrNeedTwoDifferentCoins,
    CryptoSwapErrorCode.notCrypto => l.swapErrNotCrypto(arg('ticker')),
    CryptoSwapErrorCode.notBrl => l.swapErrNotBrl(arg('ticker'), arg('currency')),
    CryptoSwapErrorCode.quantityOutMustBePositive => l.swapErrQuantityOutMustBePositive,
    CryptoSwapErrorCode.quantityInMustBePositive => l.swapErrQuantityInMustBePositive,
    CryptoSwapErrorCode.valueMustBePositive => l.swapErrValueMustBePositive,
    CryptoSwapErrorCode.negativeFee => l.swapErrNegativeFee,
    CryptoSwapErrorCode.insufficientBalance => l.swapErrInsufficientBalance(
      arg('ticker'),
      arg('available'),
      arg('requested'),
    ),
  };
}
