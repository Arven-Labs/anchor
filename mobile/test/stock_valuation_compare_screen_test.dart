import 'dart:io';

import 'package:material_ui/material_ui.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/domain/stock_valuation.dart';
import 'package:anchor_mobile/models/stock_valuation.dart';
import 'package:anchor_mobile/screens/stock_valuation_compare_screen.dart';
import 'package:anchor_mobile/screens/stock_valuations_list_screen.dart';
import 'package:anchor_mobile/services/stock_valuation_repository.dart';

import 'support/localized_app.dart';

// A comparação de valuations salvos montada de verdade: a tela de comparação com cálculos
// prontos e a seleção na lista (SQLite ffi). Dados fictícios.
void main() {
  StockValuation valuation(
    int id,
    ValuationModel model, {
    double fair = 50,
    double margin = 0.2,
    ValuationVerdict verdict = ValuationVerdict.cheap,
    Map<String, double> inputs = const {},
  }) => StockValuation(
    id: id,
    ticker: 'PETR4',
    model: model,
    referenceYear: 2026,
    currentPrice: 40,
    fairPrice: fair,
    safetyMargin: margin,
    verdict: verdict,
    inputs: inputs,
    createdAt: DateTime(2026, 11, 7),
  );

  final bazin = valuation(1, ValuationModel.bazin, inputs: {'average_dividend': 3.0, 'desired_yield': 0.06});
  final graham = valuation(
    2,
    ValuationModel.graham,
    fair: 38.4,
    margin: -0.0416667,
    verdict: ValuationVerdict.expensive,
    inputs: {'eps': 2.0, 'book_value_per_share': 20.0},
  );

  group('tela de comparação', () {
    testWidgets('uma coluna por cálculo, os preços e o veredito de cada um', (tester) async {
      await tester.pumpWidget(localizedApp(StockValuationCompareScreen(valuations: [bazin, graham])));

      expect(find.text('Cálculo 1'), findsOneWidget);
      expect(find.text('Cálculo 2'), findsOneWidget);
      expect(find.text('R\$ 50,00'), findsOneWidget);
      expect(find.text('R\$ 38,40'), findsOneWidget);
      expect(find.text('Barata'), findsOneWidget);
      expect(find.text('Cara'), findsOneWidget);
    });

    testWidgets('a linha que difere ganha o aviso; a que é igual não', (tester) async {
      await tester.pumpWidget(localizedApp(StockValuationCompareScreen(valuations: [bazin, graham])));

      expect(find.byKey(const Key('compare-differs-fair-price')), findsOneWidget);
      expect(find.byKey(const Key('compare-differs-current-price')), findsNothing);
      expect(find.byKey(const Key('compare-row-current-price')), findsOneWidget);
    });

    testWidgets('premissas de modelos diferentes aparecem com o rótulo do campo e o marcador de ausente', (tester) async {
      await tester.pumpWidget(localizedApp(StockValuationCompareScreen(valuations: [bazin, graham])));

      expect(find.byKey(const Key('compare-row-desired-yield')), findsOneWidget);
      expect(find.text('Yield desejado (%)'), findsOneWidget);
      expect(find.text('6,0%'), findsOneWidget);
      expect(find.text('—'), findsWidgets);
    });

    testWidgets('em inglês os rótulos e os números mudam de idioma', (tester) async {
      await tester.pumpWidget(
        localizedApp(StockValuationCompareScreen(valuations: [bazin, graham]), locale: 'en'),
      );

      expect(find.text('Calculation 1'), findsOneWidget);
      expect(find.text('R\$ 38.40'), findsOneWidget);
    });
  });

  group('seleção na lista de valuations salvos', () {
    late Directory dir;
    var clock = DateTime(2026, 11, 7, 10);
    final repository = StockValuationRepository(now: () => clock);

    setUp(() async {
      clock = DateTime(2026, 11, 7, 10);
      dir = await Directory.systemTemp.createTemp('anchor-compare-test-');
      await databaseFactory.setDatabasesPath(dir.path);
      await AppDatabase.resetForTest();
    });

    tearDown(() async {
      await AppDatabase.resetForTest();
      await dir.delete(recursive: true);
    });

    /// Grava [count] cálculos de PETR4, um minuto depois do outro (o mais novo aparece primeiro).
    Future<List<StockValuation>> seed(int count) async {
      final saved = <StockValuation>[];
      for (var i = 0; i < count; i++) {
        clock = clock.add(const Duration(minutes: 1));
        saved.add(
          await repository.save(
            ticker: 'PETR4',
            model: i.isEven ? ValuationModel.bazin : ValuationModel.graham,
            currentPrice: 40,
            inputs: i.isEven
                ? {'average_dividend': 3.0 + i, 'desired_yield': 0.06}
                : {'eps': 2.0 + i, 'book_value_per_share': 20.0},
          ),
        );
      }
      return saved;
    }

    Future<void> pumpList(WidgetTester tester) async {
      await tester.pumpWidget(localizedApp(const StockValuationsScreen(ticker: 'PETR4')));
      for (var i = 0; i < 200; i++) {
        await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
        await tester.pump();
        if (find.byType(CircularProgressIndicator).evaluate().isEmpty) break;
      }
      await tester.pumpAndSettle();
    }

    Finder check(int id) => find.byKey(Key('compare-check-$id'));

    bool openEnabled(WidgetTester tester) =>
        tester.widget<FilledButton>(find.byKey(const Key('compare-open'))).onPressed != null;

    testWidgets('o botão Comparar só liga com 2 cálculos marcados', (tester) async {
      final saved = (await tester.runAsync(() => seed(3)))!;
      await pumpList(tester);

      expect(openEnabled(tester), isFalse);
      await tester.tap(check(saved[0].id));
      await tester.pump();
      expect(openEnabled(tester), isFalse);
      await tester.tap(check(saved[1].id));
      await tester.pump();
      expect(openEnabled(tester), isTrue);
    });

    testWidgets('abre a comparação dos marcados, na ordem da lista (o mais novo primeiro)', (tester) async {
      final saved = (await tester.runAsync(() => seed(3)))!;
      await pumpList(tester);

      // Marca o mais antigo antes do mais novo: a ordem das colunas não segue a ordem dos toques.
      await tester.tap(check(saved[0].id));
      await tester.tap(check(saved[2].id));
      await tester.pump();
      await tester.tap(find.byKey(const Key('compare-open')));
      await tester.pumpAndSettle();

      expect(find.text('Cálculo 1'), findsOneWidget);
      expect(find.text('Cálculo 2'), findsOneWidget);
      expect(find.text('Cálculo 3'), findsNothing);
      // Dividendo 5,00 é do mais novo (3º gravado) e 3,00 do mais antigo: o mais novo vem à esquerda.
      expect(
        tester.getTopLeft(find.text('R\$ 5,00')).dx,
        lessThan(tester.getTopLeft(find.text('R\$ 3,00')).dx),
      );
    });

    testWidgets('com 4 marcados, as demais caixas ficam desligadas', (tester) async {
      final saved = (await tester.runAsync(() => seed(5)))!;
      await pumpList(tester);

      for (final item in saved.take(4)) {
        await tester.tap(check(item.id));
      }
      await tester.pump();

      expect(tester.widget<Checkbox>(check(saved[4].id)).onChanged, isNull);
      expect(tester.widget<Checkbox>(check(saved[0].id)).onChanged, isNotNull);
    });

    testWidgets('desmarcar um dos quatro libera de novo a quinta caixa', (tester) async {
      final saved = (await tester.runAsync(() => seed(5)))!;
      await pumpList(tester);
      for (final item in saved.take(4)) {
        await tester.tap(check(item.id));
      }
      await tester.pump();

      await tester.tap(check(saved[0].id));
      await tester.pump();

      expect(tester.widget<Checkbox>(check(saved[4].id)).onChanged, isNotNull);
    });
  });
}
