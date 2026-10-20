import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:http/http.dart' as http;

import '../models/quote.dart';
import 'quote_exceptions.dart';

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

  /// GET que separa "fonte indisponível" de "ativo não existe": 429 (limite
  /// do plano gratuito), 5xx, timeout e rede fora viram
  /// `QuoteUnavailableException`; os demais status voltam pro chamador, que
  /// decide (404 e afins = não encontrado).
  Future<http.Response> _get(Uri uri) async {
    final http.Response response;
    try {
      response = await _client.get(uri).timeout(const Duration(seconds: 15));
    } on TimeoutException {
      throw QuoteUnavailableException('coingecko');
    } on SocketException {
      throw QuoteUnavailableException('coingecko');
    } on http.ClientException {
      throw QuoteUnavailableException('coingecko');
    }
    if (response.statusCode == 429 || response.statusCode >= 500) {
      throw QuoteUnavailableException('coingecko');
    }
    return response;
  }

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

  Future<double> _fetchPriceById(String coinId) async {
    final uri = Uri.parse(
      '$_baseUrl/simple/price',
    ).replace(queryParameters: {'ids': coinId, 'vs_currencies': 'usd'});
    final response = await _get(uri);
    if (response.statusCode != 200) throw QuoteNotFoundException(coinId);

    try {
      final body = jsonDecode(response.body) as Map<String, dynamic>;
      return ((body[coinId] as Map<String, dynamic>)['usd'] as num).toDouble();
    } catch (_) {
      throw QuoteNotFoundException(coinId);
    }
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
