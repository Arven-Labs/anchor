import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:anchor_mobile/models/asset.dart';
import 'package:anchor_mobile/services/coingecko_quote_service.dart';
import 'package:anchor_mobile/services/quote_dispatcher.dart';
import 'package:anchor_mobile/services/quote_exceptions.dart';

void main() {
  late List<Uri> requests;
  late QuoteDispatcher dispatcher;

  setUp(() {
    requests = [];
    final client = MockClient((request) async {
      requests.add(request.url);
      if (request.url.path.endsWith('/search')) {
        return http.Response(
          jsonEncode({
            'coins': [
              {'id': 'bitcoin', 'symbol': 'BTC', 'name': 'Bitcoin', 'market_cap_rank': 1},
            ],
          }),
          200,
        );
      }
      return http.Response(
        jsonEncode({
          'bitcoin': {'usd': 70000.0},
        }),
        200,
      );
    });
    dispatcher = QuoteDispatcher(coingecko: CoinGeckoQuoteService(client: client));
  });

  Asset asset(AssetClass assetClass, {String ticker = 'BTC', String? externalId}) => Asset(
    ticker: ticker,
    name: ticker,
    assetClass: assetClass,
    currency: 'USD',
    createdAt: DateTime(2026, 1, 1),
    externalId: externalId,
  );

  test('cripto digitada ainda sem cadastro é resolvida por texto no CoinGecko', () async {
    final quote = await dispatcher.fetchQuoteForTicker('BTC', AssetClass.cripto);

    expect(quote.externalId, 'bitcoin');
    expect(quote.price, 70000.0);
    expect(requests.map((r) => r.path.split('/').last), ['search', 'price']);
  });

  test('cripto cadastrada usa o coin_id salvo e não busca por texto', () async {
    final quote = await dispatcher.fetchQuoteForAsset(asset(AssetClass.cripto, externalId: 'bitcoin'));

    expect(quote.price, 70000.0);
    expect(requests, hasLength(1));
    expect(requests.single.queryParameters['ids'], 'bitcoin');
  });

  test('cripto cadastrada sem coin_id cai na busca por texto', () async {
    final quote = await dispatcher.fetchQuoteForAsset(asset(AssetClass.cripto));

    expect(quote.externalId, 'bitcoin');
    expect(requests, hasLength(2));
  });

  test('classes sem fonte automática seguem não encontradas, sem chamar rede', () async {
    for (final assetClass in [
      AssetClass.tesouroDireto,
      AssetClass.rendaFixa,
      AssetClass.imovel,
      AssetClass.empresaNaoListada,
    ]) {
      expect(
        // Closure: nessas classes o dispatcher lança na hora, sem devolver um Future.
        () => dispatcher.fetchQuoteForTicker('X', assetClass),
        throwsA(isA<QuoteNotFoundException>()),
        reason: assetClass.name,
      );
    }
    expect(requests, isEmpty);
  });
}
