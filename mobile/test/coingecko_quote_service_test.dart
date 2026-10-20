import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:anchor_mobile/services/coingecko_quote_service.dart';
import 'package:anchor_mobile/services/quote_exceptions.dart';

void main() {
  const searchEth = {
    'coins': [
      {'id': 'ethfi', 'symbol': 'ETHFI', 'name': 'Ether.fi', 'market_cap_rank': 300},
      {'id': 'ethereum-classic', 'symbol': 'ETH', 'name': 'Fake ETH', 'market_cap_rank': 900},
      {'id': 'ethereum', 'symbol': 'ETH', 'name': 'Ethereum', 'market_cap_rank': 2},
    ],
  };

  /// Cliente que responde `/search` e `/simple/price` e anota os caminhos.
  (CoinGeckoQuoteService, List<String>) service({
    Object search = searchEth,
    Object price = const {'ethereum': {'usd': 700.1, 'brl': 3500.5}},
    int searchStatus = 200,
    int priceStatus = 200,
  }) {
    final paths = <String>[];
    final client = MockClient((request) async {
      paths.add(request.url.path);
      if (request.url.path.endsWith('/search')) {
        return http.Response(jsonEncode(search), searchStatus);
      }
      return http.Response(jsonEncode(price), priceStatus);
    });
    return (CoinGeckoQuoteService(client: client), paths);
  }

  test('resolve o símbolo exato e desempata pelo menor market_cap_rank', () async {
    final (coingecko, _) = service();

    final quote = await coingecko.fetchQuote('eth');

    expect(quote.externalId, 'ethereum');
    expect(quote.ticker, 'ETH');
    expect(quote.name, 'Ethereum');
    expect(quote.currency, 'BRL');
    expect(quote.price, 3500.5);
  });

  test('cripto antiga em USD é cotada pelo par usd', () async {
    final (coingecko, _) = service();

    final quote = await coingecko.fetchQuote('ETH', coinId: 'ethereum', currency: 'USD');

    expect(quote.currency, 'USD');
    expect(quote.price, 700.1);
  });

  test('moeda pedida sem preço no corpo é não encontrado', () async {
    final (onlyUsd, _) = service(price: const {'ethereum': {'usd': 700.1}});

    expect(onlyUsd.fetchQuote('ETH', coinId: 'ethereum'), throwsA(isA<QuoteNotFoundException>()));
  });

  test('símbolo sem match exato é não encontrado (ETH não pega só ETHFI)', () async {
    final (coingecko, _) = service(search: {
      'coins': [
        {'id': 'ethfi', 'symbol': 'ETHFI', 'name': 'Ether.fi', 'market_cap_rank': 300},
      ],
    });

    expect(coingecko.fetchQuote('ETH'), throwsA(isA<QuoteNotFoundException>()));
  });

  test('coin_id já salvo pula a busca por texto', () async {
    final (coingecko, paths) = service();

    final quote = await coingecko.fetchQuote('ETH', coinId: 'ethereum');

    expect(quote.price, 3500.5);
    expect(paths, hasLength(1));
    expect(paths.single, endsWith('/simple/price'));
  });

  test('429 e 5xx são fonte indisponível, não ativo inexistente', () async {
    for (final status in [429, 500, 503]) {
      final (onSearch, _) = service(searchStatus: status);
      expect(onSearch.fetchQuote('ETH'), throwsA(isA<QuoteUnavailableException>()), reason: 'busca $status');

      final (onPrice, _) = service(priceStatus: status);
      expect(
        onPrice.fetchQuote('ETH', coinId: 'ethereum'),
        throwsA(isA<QuoteUnavailableException>()),
        reason: 'preço $status',
      );
    }
  });

  test('timeout e rede fora são fonte indisponível', () async {
    for (final error in <Object>[TimeoutException('lento'), const SocketException('sem rede'), http.ClientException('caiu')]) {
      final coingecko = CoinGeckoQuoteService(client: MockClient((_) async => throw error));
      expect(coingecko.fetchQuote('ETH', coinId: 'ethereum'), throwsA(isA<QuoteUnavailableException>()), reason: '$error');
    }
  });

  test('404 e corpo sem o preço são não encontrado', () async {
    final (notFound, _) = service(priceStatus: 404);
    expect(notFound.fetchQuote('X', coinId: 'x'), throwsA(isA<QuoteNotFoundException>()));

    final (empty, _) = service(price: const <String, Object>{});
    expect(empty.fetchQuote('X', coinId: 'x'), throwsA(isA<QuoteNotFoundException>()));

    final (noUsd, _) = service(price: const {'x': {'brl': 10}});
    expect(noUsd.fetchQuote('X', coinId: 'x'), throwsA(isA<QuoteNotFoundException>()));
  });

  test('várias moedas saem numa só chamada e a que falta fica de fora', () async {
    final urls = <Uri>[];
    final client = MockClient((request) async {
      urls.add(request.url);
      return http.Response(
        jsonEncode({
          'bitcoin': {'usd': 70000.0, 'brl': 350000.0},
          'ethereum': {'usd': 3500},
        }),
        200,
      );
    });
    final coingecko = CoinGeckoQuoteService(client: client);

    final prices = await coingecko.fetchPrices(['bitcoin', 'ethereum', 'moeda-sumida', 'bitcoin']);

    expect(prices, {
      'bitcoin': {'usd': 70000.0, 'brl': 350000.0},
      'ethereum': {'usd': 3500.0},
    });
    expect(urls, hasLength(1));
    expect(urls.single.queryParameters['vs_currencies'], 'usd,brl');
    expect(urls.single.queryParameters['ids'], 'bitcoin,ethereum,moeda-sumida');
  });

  test('lista vazia não chama a rede e 429 na chamada em lote é indisponível', () async {
    var calls = 0;
    final client = MockClient((_) async {
      calls++;
      return http.Response('{}', 429);
    });
    final coingecko = CoinGeckoQuoteService(client: client);

    expect(await coingecko.fetchPrices(const []), isEmpty);
    expect(calls, 0);
    expect(coingecko.fetchPrices(['bitcoin']), throwsA(isA<QuoteUnavailableException>()));
  });

  test('busca com status de erro comum ou corpo malformado é não encontrado', () async {
    final (badStatus, _) = service(searchStatus: 404);
    expect(badStatus.fetchQuote('ETH'), throwsA(isA<QuoteNotFoundException>()));

    final (malformed, _) = service(search: const {'oops': true});
    expect(malformed.fetchQuote('ETH'), throwsA(isA<QuoteNotFoundException>()));
  });
}
