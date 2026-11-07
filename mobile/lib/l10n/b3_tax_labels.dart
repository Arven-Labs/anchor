import '../domain/b3_tax.dart';
import '../domain/crypto_tax.dart' show darfMinimum;
import '../domain/tax_format.dart';
import 'app_localizations.dart';

// Textos do IR de B3 (ADR 0001). Um `switch` sem `default` nos enums faz o
// analisador acusar um valor novo sem texto.

/// Situação do mês numa frase: DARF, abaixo do mínimo, isento ou sem imposto.
String b3MonthStatus(AppLocalizations l, B3Month month) {
  final locale = l.localeName;
  if (month.due > 0) {
    return l.b3TaxStatusDarf(
      month.darfCode,
      formatBrl(month.due, locale: locale),
      formatDay(month.dueDate, locale: locale),
    );
  }
  if (month.carriedOut > 0) {
    return l.b3TaxStatusBelowMinimum(
      formatBrl(month.carriedOut, locale: locale),
      formatBrl(darfMinimum, locale: locale),
    );
  }
  if (month.stockExempt && month.exemptGain > 0) {
    return l.b3TaxStatusExempt(
      formatBrl(b3StockExemptionMonthlySales, locale: locale),
    );
  }
  return l.b3TaxStatusNoTax;
}

String b3BucketName(AppLocalizations l, B3Bucket bucket) => switch (bucket) {
  B3Bucket.common => l.b3TaxBucketCommon,
  B3Bucket.dayTrade => l.b3TaxBucketDayTrade,
  B3Bucket.fii => l.b3TaxBucketFii,
};

String b3WarningText(AppLocalizations l, B3TaxWarning warning) =>
    switch (warning.kind) {
      B3TaxWarningKind.costUnknown => l.b3TaxWarnCostUnknown(
        warning.ticker,
        formatDay(warning.date, locale: l.localeName),
      ),
    };
