import 'package:http/http.dart' as http;

import 'crypto_source_http.dart';

/// Funding da Binance sai a cada 8h: 3 por dia, 7 dias.
const _fundingPrintsPerDay = 3;
const _fundingWindow = 21;

/// A Binance só guarda ~30 dias de histórico de OI e de long/short.
const _historyDays = 30;
const _longShortWindow = 7;

const _epsilon = 2.220446049250313e-16;

// ---------------------------------------------------------------------------
// Cálculos puros (espelham `crypto_sources.rs` do desktop)
// ---------------------------------------------------------------------------

double? _mean(List<double> values) =>
    values.isEmpty ? null : values.reduce((a, b) => a + b) / values.length;

/// Média das taxas de funding por período de 8h, anualizada em percentual
/// (ex.: 0,0001 por 8h = 10,95% ao ano).
double? fundingAnnualizedPct(List<double> rates) {
  final average = _mean(rates);
  return average == null ? null : average * _fundingPrintsPerDay * 365 * 100;
}

/// Variação percentual entre o ponto mais antigo e o mais recente.
double? pctChange(double oldest, double newest) {
  if (oldest.abs() < _epsilon) return null;
  return (newest / oldest - 1) * 100;
}

/// Símbolo do perpétuo em USDT (`ETHUSDT`, `BTCUSDT`), igual na Binance e na Bybit.
String perpSymbol(String coin) => '${coin.toUpperCase()}USDT';

// ---------------------------------------------------------------------------
// Parsers (JSON → números). Todos devolvem em ordem cronológica, do mais
// antigo ao mais novo: a Binance já manda assim, a Bybit manda do mais novo
// para o mais antigo e o parser inverte. Trocar isso não dá erro, só inverte o
// sinal da variação do open interest.
// ---------------------------------------------------------------------------

CryptoSourceException _unexpected(String source) =>
    CryptoSourceException(source, 'unexpected response');

/// Campo de texto numérico de uma linha da resposta.
double _textNumber(String source, Object? row, String field) {
  if (row is! Map || row[field] is! String) throw _unexpected(source);
  return parseSourceNumber(source, field, row[field] as String);
}

List<double> _binanceColumn(String source, String body, String field) {
  final json = decodeSourceJson(source, body);
  if (json is! List) throw _unexpected(source);
  return [for (final row in json) _textNumber(source, row, field)];
}

List<double> parseBinanceFunding(String body) =>
    _binanceColumn('Binance funding', body, 'fundingRate');

/// Open interest em unidades do ativo (ETH), do mais antigo ao mais recente.
List<double> parseBinanceOpenInterest(String body) =>
    _binanceColumn('Binance open interest', body, 'sumOpenInterest');

List<double> parseBinanceLongShort(String body) =>
    _binanceColumn('Binance long/short', body, 'longShortRatio');

/// Linhas da Bybit já em ordem cronológica. Em resposta de erro (`retCode`
/// diferente de 0) o `result` vem `{}`, sem `list`.
List<Map<Object?, Object?>> _bybitRows(String source, String body) {
  final json = decodeSourceJson(source, body);
  if (json is! Map || json['retCode'] is! int) throw _unexpected(source);
  if (json['retCode'] != 0) {
    throw CryptoSourceException(source, '${json['retMsg']}');
  }
  final result = json['result'];
  if (result == null) return [];
  if (result is! Map) throw _unexpected(source);
  final list = result['list'];
  if (list == null) return [];
  if (list is! List) throw _unexpected(source);
  return [
    for (final row in list.reversed)
      if (row is Map) row else throw _unexpected(source),
  ];
}

List<double> parseBybitFunding(String body) {
  const source = 'Bybit funding';
  return [
    for (final row in _bybitRows(source, body))
      _textNumber(source, row, 'fundingRate'),
  ];
}

List<double> parseBybitOpenInterest(String body) {
  const source = 'Bybit open interest';
  return [
    for (final row in _bybitRows(source, body))
      _textNumber(source, row, 'openInterest'),
  ];
}

/// Razão long/short = proporção de contas compradas ÷ vendidas.
List<double> parseBybitLongShort(String body) {
  const source = 'Bybit long/short';
  return [
    for (final row in _bybitRows(source, body))
      () {
        final buy = _textNumber(source, row, 'buyRatio');
        final sell = _textNumber(source, row, 'sellRatio');
        if (sell <= 0) {
          throw const CryptoSourceException(source, 'sellRatio is zero');
        }
        return buy / sell;
      }(),
  ];
}

// ---------------------------------------------------------------------------
// Fallback
// ---------------------------------------------------------------------------

/// Tenta a Binance e, se falhar (rede, HTTP, bloqueio de região ou resposta
/// inutilizável), a Bybit. Só dá erro se as duas falharem, e a mensagem traz os
/// dois motivos. Captura só [CryptoSourceException]: um bug (`StateError`,
/// `TypeError`) sobe em vez de virar "usei a Bybit".
Future<Sourced> binanceThenBybit(
  String metric,
  Future<double> Function() binance,
  Future<double> Function() bybit,
) async {
  try {
    return (value: await binance(), source: 'binance');
  } on CryptoSourceException catch (primary) {
    try {
      return (value: await bybit(), source: 'bybit');
    } on CryptoSourceException catch (fallback) {
      throw CryptoSourceException(
        metric,
        'Binance failed ($primary); Bybit failed ($fallback)',
      );
    }
  }
}

// ---------------------------------------------------------------------------
// Serviço
// ---------------------------------------------------------------------------

/// Derivativos do perpétuo `{moeda}USDT` (funding, open interest e long/short),
/// da Binance com fallback para a Bybit. Endpoints públicos, sem chave.
class PerpDerivativesService {
  static const _binanceHost = 'fapi.binance.com';
  static const _bybitHost = 'api.bybit.com';

  final http.Client _client;

  PerpDerivativesService({http.Client? client})
    : _client = client ?? http.Client();

  Future<String> _get(
    String host,
    String path,
    Map<String, String> query,
    String source,
  ) => cryptoGet(_client, Uri.https(host, path, query), source: source);

  double _funding(List<double> rates, String source) =>
      fundingAnnualizedPct(rates) ??
      (throw CryptoSourceException(source, 'no funding data'));

  double _openInterest(List<double> series, String source) {
    if (series.length < 2) {
      throw CryptoSourceException(source, 'not enough history');
    }
    return pctChange(series.first, series.last) ??
        (throw CryptoSourceException(
          source,
          'zero open interest at the start',
        ));
  }

  double _longShort(List<double> ratios, String source) =>
      _mean(ratios) ?? (throw CryptoSourceException(source, 'no data'));

  /// Funding médio de 7 dias, anualizado em %.
  Future<Sourced> fundingRate7d(String coin) {
    final symbol = perpSymbol(coin);
    return binanceThenBybit(
      'funding',
      () async => _funding(
        parseBinanceFunding(
          await _get(_binanceHost, '/fapi/v1/fundingRate', {
            'symbol': symbol,
            'limit': '$_fundingWindow',
          }, 'Binance funding'),
        ),
        'Binance funding',
      ),
      () async => _funding(
        parseBybitFunding(
          await _get(_bybitHost, '/v5/market/funding/history', {
            'category': 'linear',
            'symbol': symbol,
            'limit': '$_fundingWindow',
          }, 'Bybit funding'),
        ),
        'Bybit funding',
      ),
    );
  }

  /// Variação do open interest, em unidades da moeda, nos últimos ~30 dias, em %.
  Future<Sourced> openInterestChange30d(String coin) {
    final symbol = perpSymbol(coin);
    return binanceThenBybit(
      'open interest',
      () async => _openInterest(
        parseBinanceOpenInterest(
          await _get(_binanceHost, '/futures/data/openInterestHist', {
            'symbol': symbol,
            'period': '1d',
            'limit': '$_historyDays',
          }, 'Binance open interest'),
        ),
        'Binance open interest',
      ),
      () async => _openInterest(
        parseBybitOpenInterest(
          await _get(_bybitHost, '/v5/market/open-interest', {
            'category': 'linear',
            'symbol': symbol,
            'intervalTime': '1d',
            'limit': '$_historyDays',
          }, 'Bybit open interest'),
        ),
        'Bybit open interest',
      ),
    );
  }

  /// Média de 7 dias da razão long/short de contas.
  Future<Sourced> longShortRatio(String coin) {
    final symbol = perpSymbol(coin);
    return binanceThenBybit(
      'long/short',
      () async => _longShort(
        parseBinanceLongShort(
          await _get(
            _binanceHost,
            '/futures/data/globalLongShortAccountRatio',
            {'symbol': symbol, 'period': '1d', 'limit': '$_longShortWindow'},
            'Binance long/short',
          ),
        ),
        'Binance long/short',
      ),
      () async => _longShort(
        parseBybitLongShort(
          await _get(_bybitHost, '/v5/market/account-ratio', {
            'category': 'linear',
            'symbol': symbol,
            'period': '1d',
            'limit': '$_longShortWindow',
          }, 'Bybit long/short'),
        ),
        'Bybit long/short',
      ),
    );
  }
}
