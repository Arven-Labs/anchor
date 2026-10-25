import 'dart:convert';

import 'package:http/http.dart' as http;

import '../domain/fx_history.dart';
import '../models/quote.dart';
import 'quote_exceptions.dart';
import 'quote_http.dart';

/// Cliente Dart do mesmo endpoint não-oficial do Yahoo Finance já usado (e
/// confirmado contra a API real) pelo `data-collector` em
/// `sources/acoes_yahoo.py::fetch_quotes` — público, sem chave. Ver a nota
/// nesse arquivo Python sobre a rota não ser documentada oficialmente pelo
/// Yahoo (mesma usada por baixo dos panos pela lib `yfinance`).
class YahooQuoteService {
  static const _chartUrl = 'https://query1.finance.yahoo.com/v8/finance/chart';

  final http.Client _client;

  YahooQuoteService({http.Client? client}) : _client = client ?? http.Client();

  /// Mapeamento ticker→símbolo do contrato futuro COMEX/NYMEX, espelhando
  /// `data-collector/sources/metais_yahoo.py::TICKER_TO_YAHOO_SYMBOL`.
  static const _metalYahooSymbol = {
    'XAU': 'GC=F',
    'XAG': 'SI=F',
    'XPT': 'PL=F',
    'XPD': 'PA=F',
  };

  static const _metalName = {
    'XAU': 'Gold',
    'XAG': 'Silver',
    'XPT': 'Platinum',
    'XPD': 'Palladium',
  };

  /// `suffix` é `.SA` (B3) por padrão — ação internacional chama com
  /// `suffix: ''`, mesmo parâmetro que
  /// `data-collector/sources/acoes_yahoo.py::fetch_quotes` já usa.
  Future<Quote> fetchQuote(String ticker, {String suffix = '.SA'}) async {
    final uri = Uri.parse(
      '$_chartUrl/$ticker$suffix',
    ).replace(queryParameters: {'range': '5d', 'interval': '1d'});

    final response = await quoteGet(
      _client,
      uri,
      source: 'yahoo',
      headers: {'User-Agent': 'Mozilla/5.0'},
    );

    if (response.statusCode != 200) {
      throw QuoteNotFoundException(ticker);
    }

    try {
      final body = jsonDecode(response.body) as Map<String, dynamic>;
      final result =
          (body['chart']['result'] as List).first as Map<String, dynamic>;
      final meta = result['meta'] as Map<String, dynamic>;
      return Quote.fromYahooMeta(ticker, meta);
    } catch (_) {
      throw QuoteNotFoundException(ticker);
    }
  }

  /// Série diária de `currency` em reais (`USDBRL=X`, `EURBRL=X`…) desde
  /// `from` (menos uma semana de folga pro dia mais próximo achar vizinho)
  /// até hoje — o IR de cripto em USD e o de exterior convertem cada operação
  /// pelo câmbio da data dela. Datas em UTC; fechamento nulo (dia sem
  /// pregão) é descartado. Moeda que o Yahoo não conhece é não encontrada.
  Future<FxHistory> fetchFxHistory({
    required DateTime from,
    String currency = 'USD',
  }) => _fetchPair(from, currency, 'BRL');

  /// Como `fetchFxHistory`, mas quando o Yahoo não tem o par `MOEDABRL=X` cai
  /// para o inverso (`BRLMOEDA=X`, invertido) e, se também não existir, para
  /// as pernas via dólar (`MOEDAUSD` × `USDBRL`, cada uma com o seu inverso).
  /// Espelha `ingestion::collect_with_fallback` do desktop: só a resposta
  /// "sem dados" (`QuoteNotFoundException`) abre o próximo caminho; limite de
  /// requisições, timeout ou rede fora (`QuoteUnavailableException`) sobem na
  /// hora. A segunda perna só é pedida se a primeira existir.
  Future<FxHistory> fetchFxHistoryWithFallback({
    required DateTime from,
    String currency = 'USD',
  }) async {
    final code = currency.trim().toUpperCase();
    const base = 'BRL';
    const pivot = 'USD';

    final direct = await _route(from, code, base);
    if (direct != null) return direct;

    if (code != pivot) {
      final first = await _route(from, code, pivot);
      if (first != null) {
        final second = await _route(from, pivot, base);
        if (second != null) return first.times(second);
      }
    }
    throw QuoteNotFoundException('$code$base=X');
  }

  /// `a`→`b` pelo par direto, ou pelo inverso invertido; `null` se o Yahoo
  /// não tem nenhum dos dois.
  Future<FxHistory?> _route(DateTime from, String a, String b) async {
    try {
      return await _fetchPair(from, a, b);
    } on QuoteNotFoundException {
      try {
        return (await _fetchPair(from, b, a)).inverted();
      } on QuoteNotFoundException {
        return null;
      }
    }
  }

  Future<FxHistory> _fetchPair(DateTime from, String base, String quote) async {
    final symbol = '${base.trim().toUpperCase()}$quote=X';
    final start = DateTime.utc(
      from.year,
      from.month,
      from.day,
    ).subtract(const Duration(days: 7));
    final end = DateTime.now().toUtc().add(const Duration(days: 1));
    final uri = Uri.parse('$_chartUrl/$symbol').replace(
      queryParameters: {
        'period1': '${start.millisecondsSinceEpoch ~/ 1000}',
        'period2': '${end.millisecondsSinceEpoch ~/ 1000}',
        'interval': '1d',
      },
    );

    final response = await quoteGet(
      _client,
      uri,
      source: 'yahoo',
      headers: {'User-Agent': 'Mozilla/5.0'},
    );
    if (response.statusCode != 200) throw QuoteNotFoundException(symbol);

    try {
      final body = jsonDecode(response.body) as Map<String, dynamic>;
      final result =
          (body['chart']['result'] as List).first as Map<String, dynamic>;
      final timestamps = (result['timestamp'] as List).cast<num>();
      final closes =
          ((result['indicators']['quote'] as List).first
                  as Map<String, dynamic>)['close']
              as List;
      final series = <(DateTime, double)>[];
      for (var i = 0; i < timestamps.length && i < closes.length; i++) {
        final close = closes[i];
        if (close is! num) continue;
        series.add((
          DateTime.fromMillisecondsSinceEpoch(
            timestamps[i].toInt() * 1000,
            isUtc: true,
          ),
          close.toDouble(),
        ));
      }
      if (series.isEmpty) throw QuoteNotFoundException(symbol);
      return FxHistory(series);
    } on QuoteNotFoundException {
      rethrow;
    } catch (_) {
      throw QuoteNotFoundException(symbol);
    }
  }

  /// Metal precioso — mesmo endpoint Yahoo, mas por trás do ticker (`XAU`)
  /// tem o símbolo de um contrato futuro (`GC=F`), sem sufixo `.SA`. Preço
  /// sempre USD/onça troy, sem conversão (mesma decisão do `data-collector`).
  Future<Quote> fetchMetalQuote(String metalTicker) async {
    final ticker = metalTicker.toUpperCase();
    final yahooSymbol = _metalYahooSymbol[ticker];
    if (yahooSymbol == null) throw QuoteNotFoundException(metalTicker);

    final raw = await fetchQuote(yahooSymbol, suffix: '');
    return Quote(
      ticker: ticker,
      price: raw.price,
      name: _metalName[ticker],
      currency: 'USD',
    );
  }
}
