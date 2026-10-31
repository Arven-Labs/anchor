import 'dart:math' show sqrt;

import 'package:http/http.dart' as http;

import 'crypto_source_http.dart';
import 'perp_derivatives_service.dart' show pctChange;

const _source = 'CoinMetrics';

/// Séries diárias: "30 dias atrás" é contar 30 posições a partir do fim. Conferido
/// ao vivo em 2026-10-31: ETH, BTC, ADA e XRP têm datas contíguas, sem buracos.
const _lookbackDays = 30;
const _puellWindow = 365;
const _mayerWindow = 200;
const _netflowWindow = 30;

/// Teto da página do plano Community (acima disso, HTTP 400). O MVRV Z precisa da
/// série inteira: o desvio-padrão é da história toda.
const _fullHistoryPageSize = 10000;

/// Idade máxima da última linha. O normal é 1 a 2 dias; o Community parou de
/// atualizar as métricas do BNB em 2019 e o cálculo seguiria "funcionando".
const _maxAgeDays = 7;

CryptoSourceException _unexpected([String detail = 'unexpected response']) =>
    CryptoSourceException(_source, detail);

/// Linhas de `data`, do dia mais antigo ao mais novo, recusando a série cuja última
/// linha é velha demais (a API só manda dado velho, nunca erro, nesse caso).
List<Map<String, Object?>> _rows(String body, DateTime today) {
  final json = decodeSourceJson(_source, body);
  final data = json is Map ? json['data'] : null;
  if (data is! List || data.isEmpty) throw _unexpected('empty series');
  final rows = <Map<String, Object?>>[];
  for (final row in data) {
    if (row is! Map<String, dynamic>) throw _unexpected();
    rows.add(row);
  }
  // `time` vem como `2026-10-26T00:00:00.000000000Z`; só a data importa.
  final time = rows.last['time'];
  final day = time is String && time.length >= 10
      ? time.substring(0, 10)
      : null;
  final date = day == null ? null : DateTime.tryParse('${day}T00:00:00Z');
  if (day == null || date == null) {
    throw _unexpected("last row has no valid 'time'");
  }
  final utc = today.toUtc();
  final age = DateTime.utc(utc.year, utc.month, utc.day).difference(date).inDays;
  if (age > _maxAgeDays) {
    throw _unexpected('stale data, latest row is $day ($age days old)');
  }
  return rows;
}

/// Coluna [metric] de todas as linhas. A API manda todo número como texto; `null`
/// (métrica que a moeda não tem), texto, `NaN`, infinito e negativo são erro.
List<double> _column(List<Map<String, Object?>> rows, String metric) {
  final values = <double>[];
  for (final row in rows) {
    final raw = row[metric];
    final double value;
    if (raw is String) {
      value = parseSourceNumber(_source, metric, raw);
    } else if (raw is num && raw.isFinite) {
      value = raw.toDouble();
    } else {
      throw _unexpected("invalid '$metric'");
    }
    if (value < 0) throw _unexpected("negative '$metric'");
    values.add(value);
  }
  return values;
}

double _mean(Iterable<double> values) =>
    values.reduce((a, b) => a + b) / values.length;

/// Variação % do último contra o ponto de 30 posições atrás.
double _trend(List<double> values) {
  if (values.length <= _lookbackDays) throw _unexpected('not enough history');
  final previous = values[values.length - 1 - _lookbackDays];
  return pctChange(previous, values.last) ?? (throw _unexpected('zero baseline'));
}

/// MVRV Z-Score: `(MarketCap − RealizedCap) ÷ desvio-padrão(MarketCap)`, com o
/// desvio **populacional** (divide por n), como o desktop. O `CapRealUSD` é
/// bloqueado no plano grátis, então o RealizedCap de hoje sai de
/// `MarketCap ÷ CapMVRVCur`.
double parseMvrvZScore(String body, {required DateTime today}) {
  final rows = _rows(body, today);
  final caps = _column(rows, 'CapMrktCurUSD');
  final mvrv = _column(rows, 'CapMVRVCur');
  if (mvrv.any((m) => m == 0)) throw _unexpected('zero MVRV');
  final mean = _mean(caps);
  var squares = 0.0;
  for (final cap in caps) {
    squares += (cap - mean) * (cap - mean);
  }
  final stddev = sqrt(squares / caps.length);
  if (stddev == 0) throw _unexpected('zero market cap stddev');
  final realized = caps.last / mvrv.last;
  return (caps.last - realized) / stddev;
}

/// Puell Multiple: emissão bruta de hoje (USD) ÷ média das últimas 365 linhas.
double parsePuellMultiple(String body, {required DateTime today}) {
  final issuance = _column(_rows(body, today), 'IssTotUSD');
  if (issuance.length < _puellWindow) {
    throw _unexpected('not enough issuance history');
  }
  final average = _mean(issuance.sublist(issuance.length - _puellWindow));
  if (average == 0) throw _unexpected('zero issuance moving average');
  return issuance.last / average;
}

/// Múltiplo de Mayer: preço de hoje (USD) ÷ média simples das últimas 200 linhas.
double parseMayerMultiple(String body, {required DateTime today}) {
  final prices = _column(_rows(body, today), 'PriceUSD');
  if (prices.length < _mayerWindow) {
    throw _unexpected('not enough price history');
  }
  final average = _mean(prices.sublist(prices.length - _mayerWindow));
  if (average <= 0) throw _unexpected('non-positive price moving average');
  return prices.last / average;
}

/// Variação % de endereços ativos entre hoje e 30 posições atrás.
double parseActiveAddressesTrend(String body, {required DateTime today}) =>
    _trend(_column(_rows(body, today), 'AdrActCnt'));

/// Variação % do que está guardado em exchanges (`SplyExNtv`): negativo = saída
/// das exchanges (acumulação). A última linha vem como dado provisório (`flash`).
double parseExchangeSupplyChange30d(String body, {required DateTime today}) =>
    _trend(_column(_rows(body, today), 'SplyExNtv'));

/// Variação % do preço do ETH em BTC (`PriceBTC`): positivo = o ETH ganha do BTC.
double parseEthBtcTrend30d(String body, {required DateTime today}) =>
    _trend(_column(_rows(body, today), 'PriceBTC'));

/// `Σ(entrada − saída) ÷ Σ(entrada + saída)` nas últimas 30 linhas, em [−1, 1]:
/// negativo = saída líquida (acumulação). Uma razão suaviza o ruído dia a dia
/// desses números `flash`. Menos de 30 linhas é erro (o desktop aceita menos).
double parseExchangeNetflow(String body, {required DateTime today}) {
  final rows = _rows(body, today);
  final inflow = _column(rows, 'FlowInExUSD');
  final outflow = _column(rows, 'FlowOutExUSD');
  if (inflow.length < _netflowWindow) throw _unexpected('not enough history');
  double tail(List<double> v) =>
      v.sublist(v.length - _netflowWindow).reduce((a, b) => a + b);
  final (inSum, outSum) = (tail(inflow), tail(outflow));
  final total = inSum + outSum;
  if (total == 0) throw _unexpected('zero total flow');
  return (inSum - outSum) / total;
}

/// Inflação bruta anual da última linha, em %: `IssTotNtv × 365 ÷ SplyCur × 100`
/// (emissão diária em unidades da moeda, antes de qualquer queima). Emissão zero
/// é uma inflação válida de 0 %; oferta zero é erro.
double parseGrossInflationPct(String body, {required DateTime today}) {
  final last = [_rows(body, today).last];
  final issuance = _column(last, 'IssTotNtv').single;
  final supply = _column(last, 'SplyCur').single;
  if (supply <= 0) throw _unexpected('circulating supply is zero');
  return issuance * 365 / supply * 100;
}

/// Id do ativo no CoinMetrics; só as moedas que o score cobre.
const _assets = {
  'ETH': 'eth',
  'BTC': 'btc',
  'ADA': 'ada',
  'XRP': 'xrp',
  'BNB': 'bnb',
};

/// Indicadores on-chain do CoinMetrics Community (público, sem chave; o servidor
/// declara 6.000 pedidos por janela de 20 s, medido em 2026-10-31).
class CoinMetricsService {
  final http.Client _client;
  final DateTime Function() _now;

  /// [now] é o relógio (injetável nos testes) da checagem de dado velho.
  CoinMetricsService({http.Client? client, DateTime Function()? now})
    : _client = client ?? http.Client(),
      _now = now ?? DateTime.now;

  /// Corpo das [metrics] da [coin], as [rows] linhas mais recentes. Moeda fora do
  /// score é erro de uso ([ArgumentError]), antes de qualquer pedido.
  Future<String> _fetch(String coin, String metrics, int rows) {
    final asset = _assets[coin.toUpperCase()];
    if (asset == null) {
      throw ArgumentError('Sem CoinMetrics para $coin (só ETH, BTC, ADA, XRP e BNB)');
    }
    return cryptoGet(
      _client,
      Uri.https('community-api.coinmetrics.io', '/v4/timeseries/asset-metrics', {
        'assets': asset,
        'metrics': metrics,
        'frequency': '1d',
        'page_size': '$rows',
      }),
      source: _source,
    );
  }

  Future<double> mvrvZScore(String coin) async => parseMvrvZScore(
    await _fetch(coin, 'CapMrktCurUSD,CapMVRVCur', _fullHistoryPageSize),
    today: _now(),
  );

  Future<double> puellMultiple(String coin) async => parsePuellMultiple(
    await _fetch(coin, 'IssTotUSD', _puellWindow),
    today: _now(),
  );

  Future<double> mayerMultiple(String coin) async => parseMayerMultiple(
    await _fetch(coin, 'PriceUSD', _mayerWindow),
    today: _now(),
  );

  Future<double> activeAddressesTrend(String coin) async =>
      parseActiveAddressesTrend(
        await _fetch(coin, 'AdrActCnt', _lookbackDays + 1),
        today: _now(),
      );

  Future<double> exchangeSupplyChange30d(String coin) async =>
      parseExchangeSupplyChange30d(
        await _fetch(coin, 'SplyExNtv', _lookbackDays + 1),
        today: _now(),
      );

  Future<double> ethBtcTrend30d(String coin) async => parseEthBtcTrend30d(
    await _fetch(coin, 'PriceBTC', _lookbackDays + 1),
    today: _now(),
  );

  Future<double> exchangeNetflow(String coin) async => parseExchangeNetflow(
    await _fetch(coin, 'FlowInExUSD,FlowOutExUSD', _netflowWindow),
    today: _now(),
  );

  /// Só a linha mais recente: o staking real desconta a inflação de hoje.
  Future<double> grossInflationPct(String coin) async => parseGrossInflationPct(
    await _fetch(coin, 'IssTotNtv,SplyCur', 1),
    today: _now(),
  );
}
