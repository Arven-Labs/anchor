import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:anchor_mobile/services/quote_exceptions.dart';
import 'package:anchor_mobile/services/yahoo_quote_service.dart';

void main() {
  final chart = {
    'chart': {
      'result': [
        {
          'meta': {
            'regularMarketPrice': 38.5,
            'longName': 'Petroleo Brasileiro',
            'currency': 'BRL',
          },
        },
      ],
    },
  };

  YahooQuoteService serviceReturning(int status, [Object? body]) => YahooQuoteService(
    client: MockClient((_) async => http.Response(jsonEncode(body ?? chart), status)),
  );

  test('lê o preço do meta do gráfico e usa o sufixo .SA por padrão', () async {
    late Uri url;
    final yahoo = YahooQuoteService(
      client: MockClient((request) async {
        url = request.url;
        return http.Response(jsonEncode(chart), 200);
      }),
    );

    final quote = await yahoo.fetchQuote('PETR4');

    expect(quote.price, 38.5);
    expect(quote.currency, 'BRL');
    expect(url.path, endsWith('/PETR4.SA'));
  });

  test('429 e 5xx são fonte indisponível, não ticker inexistente', () async {
    for (final status in [429, 500, 503]) {
      expect(serviceReturning(status).fetchQuote('PETR4'), throwsA(isA<QuoteUnavailableException>()), reason: '$status');
    }
  });

  test('timeout e rede fora são fonte indisponível', () async {
    for (final error in <Object>[TimeoutException('lento'), const SocketException('sem rede'), http.ClientException('caiu')]) {
      final yahoo = YahooQuoteService(client: MockClient((_) async => throw error));
      expect(yahoo.fetchQuote('PETR4'), throwsA(isA<QuoteUnavailableException>()), reason: '$error');
    }
  });

  test('404 e corpo sem resultado são não encontrado', () async {
    expect(serviceReturning(404).fetchQuote('XXXX'), throwsA(isA<QuoteNotFoundException>()));
    expect(
      serviceReturning(200, {'chart': {'result': <Object>[]}}).fetchQuote('XXXX'),
      throwsA(isA<QuoteNotFoundException>()),
    );
  });

  test('metal indisponível propaga a falha da fonte; metal desconhecido é não encontrado', () async {
    expect(serviceReturning(429).fetchMetalQuote('XAU'), throwsA(isA<QuoteUnavailableException>()));
    expect(serviceReturning(200).fetchMetalQuote('XYZ'), throwsA(isA<QuoteNotFoundException>()));
  });
}
