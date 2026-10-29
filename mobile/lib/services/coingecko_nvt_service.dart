import 'package:http/http.dart' as http;

import 'crypto_source_http.dart';

const _source = 'CoinGecko NVT';

/// Dias anteriores a hoje que a média precisa ter. A janela pedida é de 90; o
/// piso de 60 (dois terços) impede que uma resposta degradada vire uma "média de
/// 90 dias" calculada sobre poucos dias, que mudaria o significado do número.
const _minPreviousDays = 60;

/// Id da moeda na CoinGecko; só as que o score cobre.
const _coinIds = {
  'ETH': 'ethereum',
  'BTC': 'bitcoin',
  'ADA': 'cardano',
  'XRP': 'ripple',
  'BNB': 'binancecoin',
};

CryptoSourceException _unexpected() =>
    const CryptoSourceException(_source, 'unexpected response');

/// Valores de `[[timestamp, valor], ...]`, que precisam ser números finitos.
List<double> _column(Object? json, String key) {
  final raw = json is Map ? json[key] : null;
  if (raw is! List) throw _unexpected();
  return [
    for (final pair in raw)
      if (pair is List &&
          pair.length >= 2 &&
          pair[1] is num &&
          (pair[1] as num).isFinite)
        (pair[1] as num).toDouble()
      else
        throw _unexpected(),
  ];
}

/// NVT de hoje dividido pela média dos dias anteriores. NVT diário = market cap
/// ÷ volume; "volume" aqui é o negociado em corretoras (proxy), não o liquidado
/// on-chain. O último ponto é o de hoje (no meio do dia).
double parseNvtVsMa90(String body) {
  final json = decodeSourceJson(_source, body);
  final caps = _column(json, 'market_caps');
  final volumes = _column(json, 'total_volumes');
  if (caps.length != volumes.length) {
    throw const CryptoSourceException(
      _source,
      'market_caps and total_volumes differ in length',
    );
  }
  final daily = <double>[];
  for (var i = 0; i < caps.length; i++) {
    if (caps[i] < 0) {
      throw const CryptoSourceException(_source, 'negative market cap');
    }
    if (volumes[i] <= 0) {
      throw const CryptoSourceException(_source, 'non-positive volume');
    }
    daily.add(caps[i] / volumes[i]);
  }
  if (daily.length <= _minPreviousDays) {
    throw const CryptoSourceException(_source, 'not enough NVT history');
  }
  final previous = daily.sublist(0, daily.length - 1);
  final average = previous.reduce((a, b) => a + b) / previous.length;
  if (average == 0) {
    throw const CryptoSourceException(_source, 'zero NVT moving average');
  }
  return daily.last / average;
}

/// NVT (market cap ÷ volume) contra a própria média de 90 dias, na CoinGecko
/// (plano grátis, sem chave). Uma chamada por moeda; o limite do plano é baixo
/// (medido: a 6ª chamada seguida deu 429 com `Retry-After` de 60 s).
class CoinGeckoNvtService {
  final http.Client _client;

  CoinGeckoNvtService({http.Client? client})
    : _client = client ?? http.Client();

  /// NVT de hoje ÷ média dos dias anteriores. Moeda fora do score é erro de uso
  /// ([ArgumentError]), antes de qualquer pedido.
  Future<double> nvtVsMa90(String coin) async {
    final id = _coinIds[coin.toUpperCase()];
    if (id == null) {
      throw ArgumentError('Sem id da CoinGecko para $coin');
    }
    final body = await cryptoGet(
      _client,
      Uri.https('api.coingecko.com', '/api/v3/coins/$id/market_chart', {
        'vs_currency': 'usd',
        'days': '90',
        'interval': 'daily',
      }),
      source: _source,
    );
    return parseNvtVsMa90(body);
  }
}
