import 'package:http/http.dart' as http;

import 'crypto_source_http.dart';

const _source = 'CoinGecko global';

/// Dominância do BTC (% do cap. total do mercado), do `/global`. Só o valor de
/// hoje: a fonte não tem série grátis, então não há tendência.
double parseBtcDominance(String body) {
  final json = decodeSourceJson(_source, body);
  final data = json is Map ? json['data'] : null;
  final shares = data is Map ? data['market_cap_percentage'] : null;
  if (shares is! Map) {
    throw const CryptoSourceException(_source, 'unexpected response');
  }
  final btc = shares['btc'];
  if (btc == null) {
    throw const CryptoSourceException(
      _source,
      'missing market_cap_percentage.btc',
    );
  }
  if (btc is! num || !btc.isFinite || btc < 0 || btc > 100) {
    throw CryptoSourceException(_source, 'implausible dominance $btc');
  }
  return btc.toDouble();
}

/// Dados globais do mercado na CoinGecko (sem chave, plano grátis).
class CoinGeckoGlobalService {
  final http.Client _client;

  CoinGeckoGlobalService({http.Client? client})
    : _client = client ?? http.Client();

  /// Dominância do BTC hoje, em % do cap. total.
  Future<double> btcDominance() async {
    final body = await cryptoGet(
      _client,
      Uri.https('api.coingecko.com', '/api/v3/global'),
      source: _source,
    );
    return parseBtcDominance(body);
  }
}
