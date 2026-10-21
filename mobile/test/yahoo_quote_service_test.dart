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

  group('fetchFxHistory (USDBRL=X)', () {
    // 02/01/2026 e 05/01/2026 00:00 UTC, e um dia sem fechamento no meio.
    final jan2 = DateTime.utc(2026, 1, 2).millisecondsSinceEpoch ~/ 1000;
    final jan3 = DateTime.utc(2026, 1, 3).millisecondsSinceEpoch ~/ 1000;
    final jan5 = DateTime.utc(2026, 1, 5).millisecondsSinceEpoch ~/ 1000;

    Map<String, Object?> fxChart(List<int> timestamps, List<Object?> closes) => {
      'chart': {
        'result': [
          {
            'timestamp': timestamps,
            'indicators': {
              'quote': [
                {'close': closes},
              ],
            },
          },
        ],
      },
    };

    test('lê datas e fechamentos, descarta o fechamento nulo e pede o par certo', () async {
      late Uri url;
      final yahoo = YahooQuoteService(
        client: MockClient((request) async {
          url = request.url;
          return http.Response(jsonEncode(fxChart([jan2, jan3, jan5], [5.0, null, 5.4])), 200);
        }),
      );

      final fx = await yahoo.fetchFxHistory(from: DateTime(2026, 1, 2));

      expect(url.path, endsWith('/USDBRL=X'));
      expect(url.queryParameters['interval'], '1d');
      // period1 = 02/01 menos uma semana de folga.
      expect(
        int.parse(url.queryParameters['period1']!),
        DateTime.utc(2025, 12, 26).millisecondsSinceEpoch ~/ 1000,
      );
      expect(fx.rateOn(DateTime(2026, 1, 2)), 5.0);
      expect(fx.rateOn(DateTime(2026, 1, 5)), 5.4);
      expect(fx.rateOn(DateTime(2026, 1, 3)), 5.0, reason: 'o dia 03 não tem fechamento, vale o 02');
    });

    test('429 é indisponível; 404, série vazia e corpo estranho são não encontrado', () async {
      Future<void> fetch(YahooQuoteService yahoo) => yahoo.fetchFxHistory(from: DateTime(2026, 1, 2));

      expect(fetch(serviceReturning(429)), throwsA(isA<QuoteUnavailableException>()));
      expect(fetch(serviceReturning(404)), throwsA(isA<QuoteNotFoundException>()));
      expect(fetch(serviceReturning(200, fxChart([], []))), throwsA(isA<QuoteNotFoundException>()));
      expect(fetch(serviceReturning(200, fxChart([jan2], [null]))), throwsA(isA<QuoteNotFoundException>()));
      expect(fetch(serviceReturning(200, {'chart': {'result': <Object>[]}})), throwsA(isA<QuoteNotFoundException>()));
    });
  });

  test('metal indisponível propaga a falha da fonte; metal desconhecido é não encontrado', () async {
    expect(serviceReturning(429).fetchMetalQuote('XAU'), throwsA(isA<QuoteUnavailableException>()));
    expect(serviceReturning(200).fetchMetalQuote('XYZ'), throwsA(isA<QuoteNotFoundException>()));
  });
}
