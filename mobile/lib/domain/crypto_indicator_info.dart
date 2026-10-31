import 'crypto_score.dart';

/// Nome de cada indicador como a tela do score o mostra. As chaves são as do
/// banco (`crypto_indicators.indicator`) e nunca aparecem na tela.
const _labels = {
  'mvrv_z_score': 'MVRV Z-Score',
  'nvt_ratio': 'NVT (vs. média de 90 dias)',
  'puell_multiple': 'Múltiplo de Puell',
  'net_issuance': 'Emissão líquida (emissão − queima)',
  'staking_yield': 'Staking yield real',
  'tvl_trend': 'Tendência do TVL (30d)',
  'active_addresses_trend': 'Endereços ativos (30d)',
  'exchange_netflow': 'Netflow das exchanges',
  'fees_vs_emission': 'Taxas vs. emissão',
  'funding_rate_7d': 'Funding dos perpétuos (7d, anualizado)',
  'open_interest_change_30d': 'Open interest (variação 30d)',
  'long_short_ratio': 'Razão long/short (7d)',
  'dvol': 'DVOL (volatilidade implícita)',
  'put_call_ratio': 'Put/call de opções',
  'stablecoin_supply_change_30d': 'Oferta de stablecoins (30d)',
  'exchange_supply_change_30d': 'Oferta nas exchanges (30d)',
  'eth_btc_trend_30d': 'ETH/BTC (30d)',
  'fear_greed_index': 'Fear & Greed (0–100)',
  'mayer_multiple': 'Múltiplo de Mayer',
  'btc_dominance': 'Dominância do BTC',
  'sopr': 'SOPR',
  'nupl': 'NUPL',
};

/// Indicador desconhecido mostra a chave como veio, em vez de quebrar a tela.
String indicatorLabel(String indicator) => _labels[indicator] ?? indicator;

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

/// Valor para a tela: vírgula decimal, `%` nos percentuais, sem `-0,00` e `—` para
/// o que não é número finito.
String formatIndicatorValue(String indicator, double value) {
  if (!value.isFinite) return '—';
  var text = value.toStringAsFixed(_decimals(indicator));
  // Zero negativo ("-0.00", de -0.001 ou de -0.0) mostra como zero.
  if (double.parse(text) == 0) text = text.replaceFirst('-', '');
  text = text.replaceAll('.', ',');
  return _percent.contains(indicator) ? '$text %' : text;
}

final _errorLine = RegExp(r'^([A-Z]+) ([a-z0-9_]+): (.*)$', dotAll: true);

/// Linha de erro do coletor (`ETH mvrv_z_score: CoinMetrics: HTTP 404`) com o nome
/// do indicador no lugar da chave do banco (`ETH MVRV Z-Score: …`). O detalhe da
/// fonte segue como veio; linha de outro formato ou indicador desconhecido não muda.
String describeCollectionError(String line) {
  final match = _errorLine.firstMatch(line);
  if (match == null || !_labels.containsKey(match.group(2))) return line;
  return '${match.group(1)} ${_labels[match.group(2)]}: ${match.group(3)}';
}

String groupLabel(IndicatorGroup group) => switch (group) {
  IndicatorGroup.onChain => 'On-chain',
  IndicatorGroup.fundamentals => 'Fundamentos da rede',
  IndicatorGroup.derivatives => 'Derivativos e opções',
  IndicatorGroup.liquidity => 'Liquidez de mercado',
  IndicatorGroup.sentiment => 'Sentimento de mercado',
};

String verdictLabel(ScoreVerdict verdict) => switch (verdict) {
  ScoreVerdict.intact => 'Tese intacta — manter / seguir aportando',
  ScoreVerdict.neutral => 'Neutro — acompanhar de perto',
  ScoreVerdict.reduceRisk => 'Reduzir risco',
};

/// Texto do sinal gravado (`GREEN`, `NEUTRAL`, `RED`); a cor nunca é a única pista.
String signalLabel(String wire) => switch (wire) {
  'GREEN' => 'Verde',
  'NEUTRAL' => 'Neutro',
  'RED' => 'Vermelho',
  _ => '—',
};
