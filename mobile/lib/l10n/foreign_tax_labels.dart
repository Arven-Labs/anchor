import '../domain/foreign_tax.dart';
import '../domain/number_locale.dart';
import '../domain/ptax_series.dart';
import '../domain/tax_format.dart';
import 'app_localizations.dart';

// Textos do IR de exterior (ADR 0001). Um `switch` sem `default` nos enums faz o analisador
// acusar um valor novo sem texto.

String _brl(AppLocalizations l, double value) => formatBrl(value, locale: l.localeName);
String _day(AppLocalizations l, DateTime date) => formatDay(date, locale: l.localeName);

/// Câmbio de um fato e de onde veio: `Câmbio 6,0000 (PTAX)` ou `Rate 6.0000 (market)`.
String foreignRateLabel(AppLocalizations l, double rate, RateSource source) {
  final shown = withDecimalSeparator(rate.toStringAsFixed(4), l.localeName);
  return switch (source) {
    RateSource.ptax => l.foreignRatePtax(shown),
    RateSource.market => l.foreignRateMarket(shown),
  };
}

/// Situação do ano numa frase: imposto estimado, coberto pelo crédito, sem imposto por
/// prejuízo ou sem imposto por ficar abaixo do que já foi retido.
String foreignYearStatus(AppLocalizations l, ForeignYear year) {
  if (year.taxDue > 0) return l.foreignStatusTaxDue(_brl(l, year.taxDue));
  if (year.taxBeforeCredit > 0) return l.foreignStatusCovered(_brl(l, year.taxBeforeCredit));
  if (year.lossCarriedOut > year.lossCarriedIn) {
    return l.foreignStatusLoss(_brl(l, year.lossCarriedOut - year.lossCarriedIn));
  }
  return l.foreignStatusFitInLoss;
}

String foreignWarningText(AppLocalizations l, ForeignTaxWarning warning) => switch (warning.kind) {
  ForeignTaxWarningKind.costUnknown => l.foreignWarnCostUnknown(
    warning.ticker!,
    _day(l, warning.date!),
  ),
  ForeignTaxWarningKind.fxMissing => l.foreignWarnFxMissing(warning.year!),
  ForeignTaxWarningKind.fxApproximate => l.foreignWarnFxApproximate(warning.year!),
};

/// "Grupo 03 · Código 01 · País 249" com o que o usuário informou; vazio quando nenhum dos
/// três foi informado.
String dirpfFichaText(AppLocalizations l, {String? group, String? code, String? country}) => [
  if (group != null && group.isNotEmpty) l.foreignFichaGroup(group),
  if (code != null && code.isNotEmpty) l.foreignFichaCode(code),
  if (country != null && country.isNotEmpty) l.foreignFichaCountry(country),
].join(' · ');

/// Texto de discriminação de um ativo na declaração de `year` (posição em 31/12 do ano
/// anterior e do ano), para colar no programa da Receita. Segue o idioma da tela.
String foreignHoldingDescription(
  AppLocalizations l, {
  required int year,
  required String ticker,
  required String name,
  String? custodiaLabel,
  ForeignHolding? previous,
  ForeignHolding? current,
  String? group,
  String? code,
  String? country,
}) {
  String units(ForeignHolding? h) => h == null
      ? l.foreignDescNoPosition
      : l.foreignDescUnits(formatHoldingQuantity(h.quantity, locale: l.localeName));
  String cost(ForeignHolding? h) => _brl(l, h?.costBrl ?? 0);
  String position(int y, ForeignHolding? h) =>
      l.foreignDescPosition(_day(l, DateTime(y, 12, 31)), units(h), cost(h));
  final ficha = dirpfFichaText(l, group: group, code: code, country: country);
  return [
    l.foreignDescHead(ticker, name, custodiaLabel == null ? '' : l.foreignDescCustody(custodiaLabel)),
    if (ficha.isNotEmpty) l.foreignDescFicha(ficha),
    position(year - 1, previous),
    position(year, current),
    l.foreignDescBasis,
  ].join('\n');
}
