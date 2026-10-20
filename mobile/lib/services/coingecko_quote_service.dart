import 'dart:convert';

import 'package:http/http.dart' as http;

import '../models/quote.dart';
import 'quote_exceptions.dart';
import 'quote_http.dart';

/// Cliente Dart dos mesmos endpoints públicos (sem chave) já confirmados
/// contra a API real por `data-collector/sources/cripto_coingecko.py`.
/// Diferente do Yahoo, cripto não é identificada pelo símbolo digitado
/// (`ETH`/`BTC`) e sim por um `coin_id` interno do CoinGecko (`ethereum`,
/// `bitcoin`) — só dá pra descobrir via busca por texto.
class CoinGeckoQuoteService {
  static const _baseUrl = 'https://api.coingecko.com/api/v3';

  final http.Client _client;

  CoinGeckoQuoteService({http.Client? client})
    : _client = client ?? http.Client();

  Future<http.Response> _get(Uri uri) =>
      quoteGet(_client, uri, source: 'coingecko');

  /// Resolve um símbolo livre (`ETH`) pro `coin_id` canônico do CoinGecko.
  ///
  /// Espelha `resolve_coin_id` do Python: `GET /search?query=X` devolve
  /// vários candidatos que podem compartilhar o mesmo símbolo (ex: "eth"
  /// também acha "ETHFI") — só aceita match *exato* (case-insensitive) de
  /// símbolo, nunca fuzzy; entre matches exatos, o menor `market_cap_rank`
  /// vence (o ativo mais conhecido daquele símbolo).
  Future<Map<String, dynamic>?> _resolveCoinId(String symbol) async {
    final uri = Uri.parse(
      '$_baseUrl/search',
    ).replace(queryParameters: {'query': symbol});
    final response = await _get(uri);
    if (response.statusCode != 200) return null;

    final List<Map<String, dynamic>> coins;
    try {
      coins =
          ((jsonDecode(response.body) as Map<String, dynamic>)['coins'] as List)
              .cast<Map<String, dynamic>>();
    } catch (_) {
      return null;
    }
    final exactMatches = coins
        .where(
          (c) =>
              (c['symbol'] as String? ?? '').toLowerCase() ==
              symbol.toLowerCase(),
        )
        .toList();
    if (exactMatches.isEmpty) return null;

    exactMatches.sort((a, b) {
      final rankA = (a['market_cap_rank'] as num?) ?? double.infinity;
      final rankB = (b['market_cap_rank'] as num?) ?? double.infinity;
      return rankA.compareTo(rankB);
    });
    return exactMatches.first;
  }

  /// Preço em USD de várias moedas numa só chamada (`ids=a,b,c`) — o
  /// Portfolio cota todas as criptos de uma vez em vez de um GET por ativo,
  /// que estoura o limite do plano gratuito. Moeda que o CoinGecko não
  /// devolve fica de fora do mapa (sem cotação), sem derrubar as outras.
  Future<Map<String, double>> fetchPrices(Iterable<String> coinIds) async {
    final ids = coinIds.toSet();
    if (ids.isEmpty) return {};

    final uri = Uri.parse(
      '$_baseUrl/simple/price',
    ).replace(queryParameters: {'ids': ids.join(','), 'vs_currencies': 'usd'});
    final response = await _get(uri);
    if (response.statusCode != 200) {
      throw QuoteNotFoundException(ids.join(','));
    }

    try {
      final body = jsonDecode(response.body) as Map<String, dynamic>;
      return {
        for (final id in ids)
          if ((body[id] as Map<String, dynamic>?)?['usd'] case final num usd)
            id: usd.toDouble(),
      };
    } catch (_) {
      throw QuoteNotFoundException(ids.join(','));
    }
  }

  Future<double> _fetchPriceById(String coinId) async {
    final price = (await fetchPrices([coinId]))[coinId];
    if (price == null) throw QuoteNotFoundException(coinId);
    return price;
  }

  /// `coinId` já resolvido (ativo já cadastrado) pula a busca por texto —
  /// mais rápido e não gasta rate limit do free tier a cada refresh do
  /// Portfolio. Sem `coinId`, resolve pelo `symbol` primeiro (busca livre /
  /// cadastro novo).
  Future<Quote> fetchQuote(String symbol, {String? coinId}) async {
    String resolvedId;
    String? name;

    if (coinId != null) {
      resolvedId = coinId;
    } else {
      final resolved = await _resolveCoinId(symbol);
      if (resolved == null) throw QuoteNotFoundException(symbol);
      resolvedId = resolved['id'] as String;
      name = resolved['name'] as String?;
    }

    final price = await _fetchPriceById(resolvedId);
    return Quote(
      ticker: symbol.toUpperCase(),
      price: price,
      name: name,
      currency: 'USD',
      externalId: resolvedId,
    );
  }
}
