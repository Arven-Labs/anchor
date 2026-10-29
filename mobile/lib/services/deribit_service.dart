import 'package:http/http.dart' as http;

import 'crypto_source_http.dart';

const _dvolSource = 'Deribit DVOL';
const _optionsSource = 'Deribit options';

/// Fechamento mais recente do DVOL. A Deribit manda linhas
/// `[timestamp, open, high, low, close]`, do mais antigo ao mais novo.
double parseDvolLatest(String body) {
  final json = decodeSourceJson(_dvolSource, body);
  final data = json is Map && json['result'] is Map
      ? (json['result'] as Map)['data']
      : null;
  if (data is! List) {
    throw const CryptoSourceException(_dvolSource, 'unexpected response');
  }
  if (data.isEmpty) {
    throw const CryptoSourceException(
      _dvolSource,
      'empty volatility index series',
    );
  }
  final row = data.last;
  final close = row is List && row.length >= 5 ? row[4] : null;
  if (close is! num || !close.isFinite) {
    throw const CryptoSourceException(_dvolSource, 'unexpected response');
  }
  return close.toDouble();
}

/// `(é put, open interest)` por instrumento; o que não termina em `-P`/`-C`
/// (o perpétuo, por exemplo) é ignorado.
List<(bool, double)> parseOptionOpenInterest(String body) {
  final json = decodeSourceJson(_optionsSource, body);
  final result = json is Map ? json['result'] : null;
  if (result is! List) {
    throw const CryptoSourceException(_optionsSource, 'unexpected response');
  }
  final options = <(bool, double)>[];
  for (final entry in result) {
    final name = entry is Map ? entry['instrument_name'] : null;
    if (name is! String) {
      throw const CryptoSourceException(_optionsSource, 'unexpected response');
    }
    final isPut = name.endsWith('-P');
    if (!isPut && !name.endsWith('-C')) continue;
    final openInterest = (entry as Map)['open_interest'];
    if (openInterest is! num || !openInterest.isFinite || openInterest < 0) {
      throw CryptoSourceException(
        _optionsSource,
        "invalid open_interest for '$name'",
      );
    }
    options.add((isPut, openInterest.toDouble()));
  }
  return options;
}

double? _putsOverCalls(List<(bool, double)> openInterest) {
  var puts = 0.0;
  var calls = 0.0;
  for (final (isPut, oi) in openInterest) {
    if (isPut) {
      puts += oi;
    } else {
      calls += oi;
    }
  }
  return calls <= 0 ? null : puts / calls;
}

/// Razão entre o open interest de puts e o de calls. Nulo sem calls.
double? putCallRatio(List<(bool, double)> openInterest) =>
    _putsOverCalls(openInterest);

/// DVOL e put/call das opções da Deribit. Só ETH e BTC têm; para as outras
/// moedas a Deribit responde erro (quem decide não pedir é o coletor).
class DeribitService {
  static const _host = 'www.deribit.com';

  final http.Client _client;
  final DateTime Function() _now;

  DeribitService({http.Client? client, DateTime Function()? now})
    : _client = client ?? http.Client(),
      _now = now ?? DateTime.now;

  /// Último fechamento do DVOL (volatilidade implícita), em candles diários.
  Future<double> dvol(String coin) async {
    final end = _now().millisecondsSinceEpoch;
    final start = end - 3 * Duration.millisecondsPerDay;
    final body = await cryptoGet(
      _client,
      Uri.https(_host, '/api/v2/public/get_volatility_index_data', {
        'currency': coin.toUpperCase(),
        'start_timestamp': '$start',
        'end_timestamp': '$end',
        'resolution': '86400',
      }),
      source: _dvolSource,
    );
    return parseDvolLatest(body);
  }

  /// Put/call por open interest de todas as opções da moeda.
  Future<double> putCallRatio(String coin) async {
    final body = await cryptoGet(
      _client,
      Uri.https(_host, '/api/v2/public/get_book_summary_by_currency', {
        'currency': coin.toUpperCase(),
        'kind': 'option',
      }),
      source: _optionsSource,
    );
    return _putsOverCalls(parseOptionOpenInterest(body)) ??
        (throw const CryptoSourceException(
          _optionsSource,
          'no call open interest',
        ));
  }
}
