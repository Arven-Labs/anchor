import 'number_locale.dart';

/// Chaves dos indicadores como estão no banco (`crypto_indicators.indicator`); nunca
/// aparecem na tela. O nome de cada um vive nos `.arb` (`l10n/crypto_labels.dart`).
const knownIndicators = {
  'mvrv_z_score',
  'nvt_ratio',
  'puell_multiple',
  'net_issuance',
  'staking_yield',
  'tvl_trend',
  'active_addresses_trend',
  'exchange_netflow',
  'fees_vs_emission',
  'funding_rate_7d',
  'open_interest_change_30d',
  'long_short_ratio',
  'dvol',
  'put_call_ratio',
  'stablecoin_supply_change_30d',
  'exchange_supply_change_30d',
  'eth_btc_trend_30d',
  'fear_greed_index',
  'mayer_multiple',
  'btc_dominance',
  'sopr',
  'nupl',
};

/// Indicadores cujo valor é uma variação ou participação em %.
const _percent = {
  'net_issuance',
  'staking_yield',
  'tvl_trend',
  'active_addresses_trend',
  'funding_rate_7d',
  'open_interest_change_30d',
  'stablecoin_supply_change_30d',
  'exchange_supply_change_30d',
  'eth_btc_trend_30d',
  'btc_dominance',
};

int _decimals(String indicator) => switch (indicator) {
  'fear_greed_index' => 0,
  // O SOPR vive perto de 1: duas casas esconderiam a diferença que importa.
  'sopr' => 3,
  _ => 2,
};

/// Valor para a tela, no separador do [locale] (vírgula em `pt`, ponto no resto), com `%`
/// nos percentuais (` %` em `pt`, `%` colado em `en`), sem `-0,00` e `—` para o que não é
/// número finito. Não agrupa milhar.
String formatIndicatorValue(String indicator, double value, {required String locale}) {
  if (!value.isFinite) return '—';
  var text = value.toStringAsFixed(_decimals(indicator));
  // Zero negativo ("-0.00", de -0.001 ou de -0.0) mostra como zero.
  if (double.parse(text) == 0) text = text.replaceFirst('-', '');
  text = withDecimalSeparator(text, locale);
  if (!_percent.contains(indicator)) return text;
  return isPortuguese(locale) ? '$text %' : '$text%';
}

final _errorLine = RegExp(r'^([A-Z]+) ([a-z0-9_]+): (.*)$', dotAll: true);

/// Linha de erro do coletor (`ETH mvrv_z_score: CoinMetrics: HTTP 404`) com o nome do
/// indicador no lugar da chave do banco (`ETH MVRV Z-Score: …`). [nameOf] dá o nome no
/// idioma da tela, ou `null` para uma chave desconhecida. O detalhe da fonte segue como
/// veio; linha de outro formato ou indicador desconhecido não muda.
String describeCollectionError(String line, {required String? Function(String key) nameOf}) {
  final match = _errorLine.firstMatch(line);
  if (match == null) return line;
  final name = nameOf(match.group(2)!);
  if (name == null) return line;
  return '${match.group(1)} $name: ${match.group(3)}';
}
