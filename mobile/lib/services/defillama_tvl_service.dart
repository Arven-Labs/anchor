import 'package:http/http.dart' as http;

import 'crypto_source_http.dart';
import 'perp_derivatives_service.dart' show pctChange;

const _source = 'DefiLlama TVL';

/// Séries diárias: "30 dias atrás" é contar 30 posições a partir do fim. A série
/// real (conferida em 2026-10-29) tem datas contíguas, sem buracos.
const _lookbackDays = 30;

/// Cadeia da DefiLlama cujo TVL a moeda usa. Só as moedas com cadeia própria e
/// TVL relevante (ETH e BNB, via BSC).
const _chains = {'ETH': 'Ethereum', 'BNB': 'BSC'};

CryptoSourceException _unexpected() =>
    const CryptoSourceException(_source, 'unexpected response');

/// Variação % do TVL entre hoje e 30 posições atrás. Cada ponto é
/// `{"date":..,"tvl":..}`; só o `tvl` é lido, e precisa ser número finito e não
/// negativo.
double parseTvlTrend(String body) {
  final json = decodeSourceJson(_source, body);
  if (json is! List) throw _unexpected();
  final values = <double>[];
  for (final point in json) {
    final tvl = point is Map ? point['tvl'] : null;
    if (tvl is! num || !tvl.isFinite || tvl < 0) throw _unexpected();
    values.add(tvl.toDouble());
  }
  if (values.length <= _lookbackDays) {
    throw const CryptoSourceException(_source, 'not enough history');
  }
  final previous = values[values.length - 1 - _lookbackDays];
  return pctChange(previous, values.last) ??
      (throw const CryptoSourceException(_source, 'zero baseline'));
}

/// TVL por cadeia na DefiLlama (público, sem chave). O TVL de ETH e BNB é de
/// cadeias diferentes, então a leitura é por moeda.
class DefiLlamaTvlService {
  final http.Client _client;

  DefiLlamaTvlService({http.Client? client})
    : _client = client ?? http.Client();

  /// Variação % do TVL da cadeia da [coin] em 30 dias. Moeda sem cadeia própria
  /// é erro de uso ([ArgumentError]), antes de qualquer pedido.
  Future<double> tvlTrend(String coin) async {
    final chain = _chains[coin.toUpperCase()];
    if (chain == null) {
      throw ArgumentError('Sem TVL por cadeia para $coin (só ETH e BNB)');
    }
    final body = await cryptoGet(
      _client,
      Uri.https('api.llama.fi', '/v2/historicalChainTvl/$chain'),
      source: _source,
    );
    return parseTvlTrend(body);
  }
}
