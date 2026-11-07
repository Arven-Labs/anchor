import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/domain/stock_valuation.dart';
import 'package:anchor_mobile/domain/valuation_fields.dart';
import 'package:anchor_mobile/l10n/app_localizations.dart';
import 'package:anchor_mobile/l10n/registration_labels.dart';
import 'package:anchor_mobile/l10n/valuation_labels.dart';
import 'package:anchor_mobile/models/asset.dart';
import 'package:anchor_mobile/models/quote.dart';
import 'package:anchor_mobile/screens/quote_search_screen.dart';
import 'package:anchor_mobile/screens/stock_valuation_screen.dart';
import 'package:anchor_mobile/screens/stock_valuations_list_screen.dart';
import 'package:anchor_mobile/services/quote_dispatcher.dart';
import 'package:anchor_mobile/services/stock_valuation_repository.dart';

import 'support/localized_app.dart';

// Textos e telas do valuation de ações (ADR 0001). Dados fictícios.
Future<AppLocalizations> _load(String code) => AppLocalizations.delegate.load(Locale(code));

class _FakeDispatcher extends QuoteDispatcher {
  @override
  Future<Quote> fetchQuoteForTicker(String ticker, AssetClass assetClass) async =>
      Quote(ticker: ticker, price: 40, name: 'Petrobras', currency: 'BRL');
}

void main() {
  late AppLocalizations en;
  late AppLocalizations pt;

  setUpAll(() async {
    en = await _load('en');
    pt = await _load('pt');
  });

  group('textos', () {
    test('todo modelo e todo campo têm texto nos dois idiomas', () {
      for (final l in [pt, en]) {
        for (final model in ValuationModel.values) {
          expect(valuationModelName(l, model), isNotEmpty, reason: model.name);
          for (final field in valuationFields[model]!) {
            expect(valuationFieldLabel(l, field.key), isNotEmpty, reason: field.key);
          }
        }
      }
    });

    test('todo código de erro tem texto nos dois idiomas', () {
      for (final l in [pt, en]) {
        for (final code in ValuationErrorCode.values) {
          final text = valuationErrorText(
            l,
            ValuationException(code, const {'key': 'desired_yield'}),
          );
          expect(text, isNotEmpty, reason: code.name);
        }
      }
    });

    test('erros que citam o campo usam o rótulo dele, não a chave', () {
      final missing = ValuationException(ValuationErrorCode.missingInput, const {'key': 'desired_yield'});
      expect(valuationErrorText(pt, missing), 'Preencha o campo: Yield desejado (%)');
      expect(valuationErrorText(en, missing), 'Fill in the field: Desired yield (%)');
    });

    test('justo zero ou negativo explica que não há valuation', () {
      final error = ValuationException(ValuationErrorCode.fairPriceMustBePositive);
      expect(
        valuationErrorText(pt, error),
        'Essas premissas dão um preço justo zero ou negativo, então não há valuation. '
        'Revise os números.',
      );
    });
  });

  group('as telas', () {
    late Directory dir;
    final repository = StockValuationRepository();

    setUp(() async {
      dir = await Directory.systemTemp.createTemp('anchor-valuation-screen-');
      await databaseFactory.setDatabasesPath(dir.path);
      await AppDatabase.resetForTest();
    });

    tearDown(() async {
      await AppDatabase.resetForTest();
      await dir.delete(recursive: true);
    });

    Future<void> settle(WidgetTester tester) async {
      for (var i = 0; i < 40; i++) {
        await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
        await tester.pump();
      }
    }

    Future<void> pump(WidgetTester tester, Widget screen, String locale) async {
      tester.view.physicalSize = const Size(900, 3000);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      await tester.runAsync(AppDatabase.instance);
      await tester.pumpWidget(localizedApp(screen, locale: locale));
      await settle(tester);
    }

    Finder field(String label) => find.widgetWithText(TextField, label);

    Future<void> pickModel(WidgetTester tester, String name) async {
      await tester.tap(find.byType(DropdownButtonFormField<ValuationModel>));
      await tester.pumpAndSettle();
      await tester.tap(find.text(name).last);
      await tester.pumpAndSettle();
    }

    const calc = StockValuationScreen(ticker: 'PETR4', currentPrice: 40);

    testWidgets('pt: abre no Bazin, com o ticker, a cotação e os dois campos', (tester) async {
      await pump(tester, calc, 'pt');

      expect(find.text('Valuation — PETR4'), findsOneWidget);
      expect(find.text('Cotação atual: R\$ 40,00'), findsOneWidget);
      expect(find.text('Bazin (preço-teto por dividendos)'), findsOneWidget);
      expect(field('Dividendo médio anual por ação (R\$)'), findsOneWidget);
      expect(field('Yield desejado (%)'), findsOneWidget);
      expect(find.textContaining('Preço justo'), findsNothing);
    });

    testWidgets('pt: calcular mostra preço justo, margem e veredito (vírgula decimal)', (tester) async {
      await pump(tester, calc, 'pt');
      await tester.enterText(field('Dividendo médio anual por ação (R\$)'), '3,00');
      await tester.enterText(field('Yield desejado (%)'), '6');
      await tester.tap(find.text('Calcular'));
      await settle(tester);

      expect(find.text('Preço justo: R\$ 50,00'), findsOneWidget);
      expect(find.text('Margem de segurança: 20,0%'), findsOneWidget);
      expect(find.text('Barata: a cotação está abaixo do preço justo'), findsOneWidget);
    });

    testWidgets('pt: campo vazio e yield zero mostram o erro, sem resultado', (tester) async {
      await pump(tester, calc, 'pt');
      await tester.enterText(field('Dividendo médio anual por ação (R\$)'), '3');
      await tester.tap(find.text('Calcular'));
      await settle(tester);
      expect(find.text('Preencha o campo: Yield desejado (%)'), findsOneWidget);
      expect(find.textContaining('Preço justo'), findsNothing);

      await tester.enterText(field('Yield desejado (%)'), '0');
      await tester.tap(find.text('Calcular'));
      await settle(tester);
      expect(find.text('O yield desejado precisa ser maior que zero'), findsOneWidget);
      expect(find.text('Preencha o campo: Yield desejado (%)'), findsNothing);
    });

    testWidgets('trocar o modelo troca os campos; o DCF tem 13', (tester) async {
      await pump(tester, calc, 'pt');

      await pickModel(tester, 'Graham (valor intrínseco)');
      expect(field('Lucro por ação, LPA (R\$)'), findsOneWidget);
      expect(field('Yield desejado (%)'), findsNothing);

      await pickModel(tester, 'DCF (fluxo de caixa descontado)');
      expect(find.byType(TextField), findsNWidgets(13));
    });

    testWidgets('salvar grava o cálculo com a taxa em fração e avisa', (tester) async {
      await pump(tester, calc, 'pt');
      await tester.enterText(field('Dividendo médio anual por ação (R\$)'), '3');
      await tester.enterText(field('Yield desejado (%)'), '6');
      await tester.tap(find.text('Salvar'));
      await settle(tester);

      expect(find.text('Cálculo salvo'), findsOneWidget);
      final saved = await tester.runAsync(() => repository.list(ticker: 'PETR4'));
      expect(saved, hasLength(1));
      expect(saved!.single.model, ValuationModel.bazin);
      expect(saved.single.currentPrice, 40);
      expect(saved.single.fairPrice, closeTo(50, 1e-9));
      expect(saved.single.inputs['desired_yield'], closeTo(0.06, 1e-12));
    });

    testWidgets('salvar com premissa inválida mostra o erro e não grava', (tester) async {
      await pump(tester, calc, 'pt');
      await tester.enterText(field('Dividendo médio anual por ação (R\$)'), '3');
      await tester.enterText(field('Yield desejado (%)'), '0');
      await tester.tap(find.text('Salvar'));
      await settle(tester);

      expect(find.text('O yield desejado precisa ser maior que zero'), findsOneWidget);
      expect(find.text('Cálculo salvo'), findsNothing);
      expect(await tester.runAsync(repository.list), isEmpty);
    });

    testWidgets('en: os mesmos textos em inglês, sem sobra de português', (tester) async {
      await pump(tester, calc, 'en');
      await tester.enterText(field('Average annual dividend per share (R\$)'), '3.00');
      await tester.enterText(field('Desired yield (%)'), '6');
      await tester.tap(find.text('Calculate'));
      await settle(tester);

      expect(find.text('Current price: R\$ 40.00'), findsOneWidget);
      expect(find.text('Fair price: R\$ 50.00'), findsOneWidget);
      expect(find.text('Safety margin: 20.0%'), findsOneWidget);
      expect(find.text('Cheap: the price is below the fair price'), findsOneWidget);
      for (final word in ['Preço', 'Cotação', 'Calcular', 'Salvar', 'Barata']) {
        expect(find.textContaining(word), findsNothing, reason: word);
      }
    });

    Future<void> seed() async {
      await repository.save(
        ticker: 'PETR4',
        model: ValuationModel.bazin,
        currentPrice: 40,
        inputs: {'average_dividend': 3.0, 'desired_yield': 0.06},
      );
    }

    testWidgets('pt: lista vazia dos salvos', (tester) async {
      await pump(tester, const StockValuationsScreen(ticker: 'PETR4'), 'pt');

      expect(find.text('Valuations salvos'), findsOneWidget);
      expect(find.text('Nenhum valuation salvo para PETR4 ainda.'), findsOneWidget);
    });

    testWidgets('pt: lista os salvos, abre um com as premissas preenchidas e exclui', (tester) async {
      await tester.runAsync(seed);
      await pump(tester, const StockValuationsScreen(ticker: 'PETR4'), 'pt');

      expect(find.text('Bazin (preço-teto por dividendos)'), findsOneWidget);
      expect(
        find.textContaining('Cotação R\$ 40,00 · justo R\$ 50,00 · margem 20,0% · Barata'),
        findsOneWidget,
      );

      await tester.tap(find.text('Bazin (preço-teto por dividendos)'));
      await tester.pumpAndSettle();
      expect(find.text('Valuation — PETR4'), findsOneWidget);
      expect(tester.widget<TextField>(field('Yield desejado (%)')).controller!.text, '6');
      expect(
        tester.widget<TextField>(field('Dividendo médio anual por ação (R\$)')).controller!.text,
        '3',
      );
      await tester.pageBack();
      await tester.pumpAndSettle();

      await tester.tap(find.byTooltip('Excluir'));
      await settle(tester);
      expect(find.text('Nenhum valuation salvo para PETR4 ainda.'), findsOneWidget);
      expect(await tester.runAsync(repository.list), isEmpty);
    });

    testWidgets('en: lista dos salvos com data no formato en', (tester) async {
      await tester.runAsync(seed);
      await pump(tester, const StockValuationsScreen(ticker: 'PETR4'), 'en');

      expect(find.text('Saved valuations'), findsOneWidget);
      expect(find.text('Bazin (dividend price ceiling)'), findsOneWidget);
      expect(
        find.textContaining('Price R\$ 40.00 · fair R\$ 50.00 · margin 20.0% · Cheap'),
        findsOneWidget,
      );
    });

    Future<void> search(WidgetTester tester, {AssetClass? assetClass}) async {
      await pump(tester, QuoteSearchScreen(dispatcher: _FakeDispatcher()), 'pt');
      if (assetClass != null) {
        await tester.tap(find.byType(DropdownButtonFormField<AssetClass>));
        await tester.pumpAndSettle();
        await tester.tap(find.text(assetClassName(pt, assetClass)).last);
        await tester.pumpAndSettle();
      }
      await tester.enterText(find.byType(TextField), 'petr4');
      await tester.tap(find.text('Buscar'));
      await settle(tester);
    }

    testWidgets('busca de ação B3 mostra o botão e abre o valuation com a cotação', (tester) async {
      await search(tester);

      expect(find.text('Valuation'), findsOneWidget);
      await tester.tap(find.text('Valuation'));
      await tester.pumpAndSettle();
      expect(find.text('Valuation — PETR4'), findsOneWidget);
      expect(find.text('Cotação atual: R\$ 40,00'), findsOneWidget);
    });

    testWidgets('busca de outra classe (FII) não mostra o botão', (tester) async {
      await search(tester, assetClass: AssetClass.fii);

      expect(find.text('Petrobras'), findsOneWidget); // a cotação apareceu
      expect(find.text('Valuation'), findsNothing);
    });
  });
}
