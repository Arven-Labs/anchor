import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:anchor_mobile/domain/yahoo_monthly_history.dart';
import 'package:anchor_mobile/models/asset.dart';
import 'package:anchor_mobile/services/quote_exceptions.dart';
import 'package:anchor_mobile/services/yahoo_quote_service.dart';

// Histórico mensal do Yahoo (`interval=1mo&events=split`): leitura da resposta e pedido ao
// serviço. A forma da resposta foi conferida com uma chamada real (AAPL), tabela B1 a B6.
int _epoch(int y, int m, int d, [int h = 13, int min = 0]) =>
    DateTime.utc(y, m, d, h, min).millisecondsSinceEpoch ~/ 1000;

String _body({
  required List<int> timestamps,
  required List<double?> closes,
  Map<String, Object?>? splits,
}) => jsonEncode({
  'chart': {
    'result': [
      {
        'meta': {'regularMarketPrice': 32.0},
        'timestamp': timestamps,
        if (splits != null) 'events': {'splits': splits},
        'indicators': {
          'quote': [
            {'close': closes},
          ],
        },
      },
    ],
  },
});

Asset _asset(String ticker, AssetClass assetClass) => Asset(
  id: 1,
  ticker: ticker,
  name: ticker,
  assetClass: assetClass,
  currency: 'BRL',
  createdAt: DateTime(2026, 1, 1),
);

void main() {
  group('leitura da resposta', () {
    test('B1: fechamento vazio é descartado e o mês vem da data', () {
      final history = parseYahooMonthlyHistory(
        _body(
          timestamps: [_epoch(2026, 1, 2), _epoch(2026, 2, 2), _epoch(2026, 3, 2)],
          closes: [30.5, null, 32.0],
        ),
        'PETR4.SA',
      );
      expect(history.closes, {'2026-01': 30.5, '2026-03': 32.0});
      expect(history.splits, isEmpty);
    });

    test('B2: devolve a data de cada desdobramento', () {
      final history = parseYahooMonthlyHistory(
        _body(
          timestamps: [_epoch(2026, 1, 2)],
          closes: [30.5],
          splits: {
            '${_epoch(2026, 2, 1)}': {'date': _epoch(2026, 2, 15), 'numerator': 4.0, 'denominator': 1.0},
          },
        ),
        'AAPL',
      );
      expect(history.splits, [DateTime.utc(2026, 2, 15)]);
    });

    test('B3: corpo quebrado ou sem resultado é "ativo não encontrado"', () {
      expect(() => parseYahooMonthlyHistory('isto não é json', 'XPTO3.SA'), throwsA(isA<QuoteNotFoundException>()));
      expect(
        () => parseYahooMonthlyHistory(jsonEncode({'chart': {'result': null}}), 'XPTO3.SA'),
        throwsA(isA<QuoteNotFoundException>()),
      );
      expect(
        () => parseYahooMonthlyHistory(_body(timestamps: [_epoch(2026, 1, 2)], closes: [null]), 'XPTO3.SA'),
        throwsA(isA<QuoteNotFoundException>()),
      );
    });

    test('B4: o mês é o da data em UTC (01/jan 00:30 UTC é janeiro)', () {
      final history = parseYahooMonthlyHistory(
        _body(timestamps: [_epoch(2026, 1, 1, 0, 30), _epoch(2025, 12, 31, 23, 30)], closes: [30.0, 29.0]),
        'AAPL',
      );
      expect(history.closes, {'2026-01': 30.0, '2025-12': 29.0});
    });
  });

  group('serviço', () {
    test('B5: pede o símbolo com range=10y, interval=1mo e events=split', () async {
      late Uri url;
      final yahoo = YahooQuoteService(
        client: MockClient((request) async {
          url = request.url;
          return http.Response(_body(timestamps: [_epoch(2026, 1, 2)], closes: [30.5]), 200);
        }),
      );

      final history = await yahoo.fetchMonthlyHistory('PETR4.SA');

      expect(history.closes, {'2026-01': 30.5});
      expect(url.path, endsWith('/PETR4.SA'));
      expect(url.queryParameters, {'range': '10y', 'interval': '1mo', 'events': 'split'});
    });

    test('B5: 429 e 5xx são fonte indisponível; 404 é ativo não encontrado', () async {
      for (final status in [429, 500, 503]) {
        final yahoo = YahooQuoteService(client: MockClient((_) async => http.Response('{}', status)));
        await expectLater(yahoo.fetchMonthlyHistory('PETR4.SA'), throwsA(isA<QuoteUnavailableException>()), reason: '$status');
      }
      final notFound = YahooQuoteService(client: MockClient((_) async => http.Response('{}', 404)));
      await expectLater(notFound.fetchMonthlyHistory('XPTO3.SA'), throwsA(isA<QuoteNotFoundException>()));
    });

    test('B6: símbolo do Yahoo por classe; cripto e fundo não têm', () {
      expect(YahooQuoteService.symbolFor(_asset('PETR4', AssetClass.acaoBr)), 'PETR4.SA');
      expect(YahooQuoteService.symbolFor(_asset('HGLG11', AssetClass.fii)), 'HGLG11.SA');
      expect(YahooQuoteService.symbolFor(_asset('AAPL', AssetClass.acaoInternacional)), 'AAPL');
      expect(YahooQuoteService.symbolFor(_asset('XAU', AssetClass.metal)), 'GC=F');
      expect(YahooQuoteService.symbolFor(_asset('BTC', AssetClass.cripto)), isNull);
      expect(YahooQuoteService.symbolFor(_asset('FIDX', AssetClass.fundo)), isNull);
    });
  });
}
