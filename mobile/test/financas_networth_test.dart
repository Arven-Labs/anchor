import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/fx_history.dart';
import 'package:anchor_mobile/domain/general_finance.dart';
import 'package:anchor_mobile/models/asset.dart';
import 'package:anchor_mobile/models/position.dart';
import 'package:anchor_mobile/services/fx_rates_loader.dart';
import 'package:anchor_mobile/services/live_positions.dart';
import 'package:anchor_mobile/services/quote_exceptions.dart';

LivePositionRow row(
  AssetClass assetClass,
  String currency,
  double quantity,
  double? price,
) => LivePositionRow(
  Position(
    asset: Asset(
      id: 1,
      ticker: 'XXX',
      name: 'XXX',
      assetClass: assetClass,
      currency: currency,
      createdAt: DateTime(2026, 1, 1),
    ),
    netQuantity: quantity,
    averageBuyPrice: null,
  ),
  price,
);

FxHistory series(double rate) => FxHistory([(DateTime.utc(2026, 1, 2), rate)]);

void main() {
  group('portfolioValuedItems', () {
    test('quantidade × preço na moeda do ativo', () {
      final items = portfolioValuedItems([
        row(AssetClass.acaoInternacional, 'USD', 10, 12.5),
      ]);

      expect(items.single.currency, 'USD');
      expect(items.single.value, 125);
    });

    test('posição zerada e empresa não listada ficam de fora', () {
      final items = portfolioValuedItems([
        row(AssetClass.acaoBr, 'BRL', 0, 30), // já vendida
        row(AssetClass.empresaNaoListada, 'BRL', 1, null), // valor é a participação
        row(AssetClass.acaoBr, 'BRL', 2, 10),
      ]);

      expect(items, hasLength(1));
      expect(items.single.value, 20);
    });

    test('posição aberta sem cotação entra com valor nulo, para contar como fora', () {
      final items = portfolioValuedItems([row(AssetClass.acaoBr, 'BRL', 5, null)]);

      expect(items.single.value, isNull);
      final net = computeNetWorth(
        base: 'BRL',
        cashItems: const [],
        portfolioItems: items,
        ratesToBrl: const {},
      );
      expect(net.itemsMissingFx, 1);
      expect(net.portfolio, 0);
    });
  });

  group('loadRatesToBrl', () {
    test('busca cada moeda uma vez, pula o real e lê a taxa de hoje', () async {
      final calls = <String>[];
      final result = await loadRatesToBrl(
        ['USD', 'BRL', 'USD', 'EUR'],
        fetch: (currency) async {
          calls.add(currency);
          return series(currency == 'USD' ? 5.0 : 6.0);
        },
        today: DateTime.utc(2026, 1, 3),
      );

      expect(calls..sort(), ['EUR', 'USD']);
      expect(result.ratesToBrl, {'USD': 5.0, 'EUR': 6.0});
      expect(result.failed, isEmpty);
    });

    test('moeda sem dados, com fonte fora ou com série inútil vai para as falhas', () async {
      final result = await loadRatesToBrl(
        ['USD', 'PYG', 'ARS', 'CLP'],
        fetch: (currency) async {
          switch (currency) {
            case 'USD':
              return series(5.0);
            case 'PYG':
              throw QuoteNotFoundException('PYGBRL=X');
            case 'ARS':
              throw QuoteUnavailableException('yahoo');
            default:
              return FxHistory(const []);
          }
        },
      );

      expect(result.ratesToBrl, {'USD': 5.0});
      expect(result.failed, {'PYG', 'ARS', 'CLP'});
    });
  });
}
