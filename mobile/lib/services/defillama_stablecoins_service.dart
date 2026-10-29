import 'package:http/http.dart' as http;

import 'crypto_source_http.dart';
import 'perp_derivatives_service.dart' show pctChange;

const _source = 'DefiLlama stablecoins';
const _windowSeconds = 30 * 86400;

CryptoSourceException _unexpected() =>
    const CryptoSourceException(_source, 'unexpected response');

/// Variação % do total circulante de stablecoins (USD) nos últimos 30 dias.
/// A base é o último ponto **até** 30 dias antes do mais recente: a série tem
/// falhas, então não dá para contar 30 posições. Ponto com data ilegível é
/// ignorado; ponto com total ausente, não numérico ou negativo é erro de formato.
double parseStablecoinChange30d(String body) {
  final json = decodeSourceJson(_source, body);
  if (json is! List) throw _unexpected();

  final points = <(int, double)>[];
  for (final point in json) {
    if (point is! Map) throw _unexpected();
    final date = point['date'];
    final usd = point['totalCirculatingUSD'];
    final total = usd is Map ? usd['peggedUSD'] : null;
    if (date is! String || total is! num || !total.isFinite || total < 0) {
      throw _unexpected();
    }
    final seconds = int.tryParse(date);
    if (seconds != null) points.add((seconds, total.toDouble()));
  }
  if (points.isEmpty) {
    throw const CryptoSourceException(_source, 'empty series');
  }

  final (lastDate, lastTotal) = points.last;
  final baseline = points.reversed
      .where((p) => p.$1 <= lastDate - _windowSeconds)
      .firstOrNull;
  if (baseline == null) {
    throw const CryptoSourceException(_source, 'less than 30 days of history');
  }
  return pctChange(baseline.$2, lastTotal) ??
      (throw const CryptoSourceException(_source, 'zero supply at the start'));
}

/// Oferta total de stablecoins na DefiLlama. O valor é do mercado todo, o mesmo
/// para qualquer moeda do score.
class DefiLlamaStablecoinsService {
  final http.Client _client;

  DefiLlamaStablecoinsService({http.Client? client})
    : _client = client ?? http.Client();

  /// Variação % da oferta total de stablecoins em 30 dias.
  Future<double> supplyChange30d() async {
    final body = await cryptoGet(
      _client,
      Uri.https('stablecoins.llama.fi', '/stablecoincharts/all'),
      source: _source,
    );
    return parseStablecoinChange30d(body);
  }
}
