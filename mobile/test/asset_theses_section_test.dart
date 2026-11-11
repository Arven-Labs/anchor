import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/l10n/app_localizations.dart';
import 'package:anchor_mobile/l10n/home_labels.dart';
import 'package:anchor_mobile/models/asset.dart';
import 'package:anchor_mobile/models/quote.dart';
import 'package:anchor_mobile/screens/asset_theses_section.dart';
import 'package:anchor_mobile/screens/quote_search_screen.dart';
import 'package:anchor_mobile/services/portfolio_repository.dart';
import 'package:anchor_mobile/services/quote_dispatcher.dart';
import 'package:anchor_mobile/services/thesis_repository.dart';

import 'support/localized_app.dart';

// O bloco de teses do ticker, dentro da busca de cotação (SQLite ffi, sem rede). Tabela de
// testes TS1 a TS8 (o bloco) e TQ1 a TQ4 (a busca). Dados fictícios.
class _FakeDispatcher extends QuoteDispatcher {
  @override
  Future<Quote> fetchQuoteForTicker(String ticker, AssetClass assetClass) async =>
      Quote(ticker: ticker, price: 40, name: 'Petrobras', currency: 'BRL');
}

void main() {
  var clock = DateTime(2026, 11, 8, 10);
  final repository = ThesisRepository(now: () => clock);
  final portfolio = PortfolioRepository();
  late Directory dir;
  late AppLocalizations pt;

  setUpAll(() async {
    pt = await AppLocalizations.delegate.load(const Locale('pt'));
  });

  setUp(() async {
    clock = DateTime(2026, 11, 8, 10);
    dir = await Directory.systemTemp.createTemp('anchor-asset-theses-');
    await databaseFactory.setDatabasesPath(dir.path);
    await AppDatabase.resetForTest();
  });

  tearDown(() async {
    await AppDatabase.resetForTest();
    await dir.delete(recursive: true);
  });

  Future<int> addAsset(String ticker, {AssetClass assetClass = AssetClass.acaoBr}) => portfolio.insertAsset(
    Asset(
      ticker: ticker,
      name: 'Empresa $ticker',
      assetClass: assetClass,
      currency: 'BRL',
      createdAt: DateTime(2026, 1, 1),
    ),
  );

  // Avança também o relógio falso: sem isso a transição de tela nunca termina.
  Future<void> settle(WidgetTester tester) async {
    for (var i = 0; i < 20; i++) {
      await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
      await tester.pump(const Duration(milliseconds: 50));
    }
  }

  Future<void> bigScreen(WidgetTester tester, {Size size = const Size(800, 3000)}) async {
    tester.view.physicalSize = size;
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    await tester.runAsync(AppDatabase.instance);
  }

  Future<void> pumpSection(WidgetTester tester, {String ticker = 'PETR4'}) async {
    await bigScreen(tester);
    await tester.pumpWidget(
      localizedApp(
        Scaffold(
          body: SingleChildScrollView(
            child: AssetThesesSection(ticker: ticker, assetClass: AssetClass.acaoBr, repository: repository),
          ),
        ),
      ),
    );
    await settle(tester);
  }

  Future<void> pumpSearch(WidgetTester tester, {AssetClass? assetClass, Size? size}) async {
    await bigScreen(tester, size: size ?? const Size(800, 3000));
    await tester.pumpWidget(
      localizedApp(QuoteSearchScreen(dispatcher: _FakeDispatcher(), thesisRepository: repository)),
    );
    await settle(tester);
    if (assetClass != null) {
      await tester.tap(find.byType(DropdownButtonFormField<AssetClass>));
      await settle(tester);
      await tester.tap(find.text(assetClassName(pt, assetClass)).last);
      await settle(tester);
    }
    await tester.enterText(find.byType(TextField), 'petr4');
    await tester.tap(find.text('Buscar'));
    await settle(tester);
  }

  group('o bloco', () {
    testWidgets('TS1: ticker na carteira e sem tese mostra o vazio e o botão de nova tese', (tester) async {
      await tester.runAsync(() => addAsset('PETR4'));
      await pumpSection(tester);

      expect(find.text('Nenhuma tese para PETR4.'), findsOneWidget);
      expect(find.byKey(const Key('asset-theses-new-button')), findsOneWidget);
    });

    testWidgets('TS2: lista só as teses do ticker, a mais recente primeiro', (tester) async {
      await tester.runAsync(() async {
        final petr = await addAsset('PETR4');
        final vale = await addAsset('VALE3');
        await repository.create(title: 'Petro antiga', assetId: petr);
        clock = DateTime(2026, 11, 8, 11);
        await repository.create(title: 'Vale', assetId: vale);
        clock = DateTime(2026, 11, 8, 12);
        await repository.create(title: 'Petro nova', assetId: petr);
      });
      await pumpSection(tester);

      expect(find.text('Petro nova'), findsOneWidget);
      expect(find.text('Petro antiga'), findsOneWidget);
      expect(find.text('Vale'), findsNothing);
      expect(
        tester.getTopLeft(find.text('Petro nova')).dy < tester.getTopLeft(find.text('Petro antiga')).dy,
        isTrue,
      );
    });

    testWidgets('TS3: com o ativo na carteira, a tese nova fica ligada a ele e entra na lista', (tester) async {
      final petr = (await tester.runAsync(() => addAsset('PETR4')))!;
      await pumpSection(tester);

      await tester.tap(find.byKey(const Key('asset-theses-new-button')));
      await settle(tester);
      expect(find.text('Ativo: PETR4'), findsOneWidget);
      expect(find.byKey(const Key('thesis-asset-field')), findsNothing);
      await tester.enterText(find.byKey(const Key('thesis-title-field')), 'Dividendos');
      await tester.tap(find.byKey(const Key('thesis-save-button')));
      await settle(tester);

      expect(find.text('Dividendos'), findsOneWidget);
      final saved = (await tester.runAsync(repository.list))!.single;
      expect(saved.thesis.assetId, petr);
    });

    testWidgets('TS4: ticker fora da carteira: o editor avisa, a tese fica Global e nenhum ativo é criado', (
      tester,
    ) async {
      await pumpSection(tester);

      await tester.tap(find.byKey(const Key('asset-theses-new-button')));
      await settle(tester);
      expect(find.text('Global — PETR4 não está na carteira'), findsOneWidget);
      await tester.enterText(find.byKey(const Key('thesis-title-field')), 'Petrobras');
      await tester.tap(find.byKey(const Key('thesis-save-button')));
      await settle(tester);

      final saved = (await tester.runAsync(repository.list))!.single;
      expect(saved.thesis.title, 'Petrobras');
      expect(saved.thesis.assetId, isNull);
      expect(await tester.runAsync(portfolio.listAssets), isEmpty);
    });

    testWidgets('TS5: ativo do mesmo ticker mas de outra classe não serve: a tese fica Global', (tester) async {
      await tester.runAsync(() => addAsset('PETR4', assetClass: AssetClass.fii));
      await pumpSection(tester);

      await tester.tap(find.byKey(const Key('asset-theses-new-button')));
      await settle(tester);
      expect(find.text('Global — PETR4 não está na carteira'), findsOneWidget);
      await tester.enterText(find.byKey(const Key('thesis-title-field')), 'Petrobras');
      await tester.tap(find.byKey(const Key('thesis-save-button')));
      await settle(tester);

      expect((await tester.runAsync(repository.list))!.single.thesis.assetId, isNull);
    });

    testWidgets('TS6: título em branco mostra o erro, não grava e fica na tela', (tester) async {
      await tester.runAsync(() => addAsset('PETR4'));
      await pumpSection(tester);

      await tester.tap(find.byKey(const Key('asset-theses-new-button')));
      await settle(tester);
      await tester.enterText(find.byKey(const Key('thesis-title-field')), '   ');
      await tester.tap(find.byKey(const Key('thesis-save-button')));
      await settle(tester);

      expect(find.text('Informe o título da tese'), findsOneWidget);
      expect(await tester.runAsync(repository.list), isEmpty);
    });

    testWidgets('TS7: tocar numa tese abre preenchida; editar o título atualiza o bloco', (tester) async {
      await tester.runAsync(() async {
        final petr = await addAsset('PETR4');
        await repository.create(title: 'Juros', content: 'texto', assetId: petr);
      });
      await pumpSection(tester);

      await tester.tap(find.text('Juros'));
      await settle(tester);
      expect(find.widgetWithText(TextField, 'texto'), findsOneWidget);
      await tester.enterText(find.byKey(const Key('thesis-title-field')), 'Juros 2028');
      await tester.tap(find.byKey(const Key('thesis-save-button')));
      await settle(tester);

      expect(find.text('Juros 2028'), findsOneWidget);
      expect(find.text('Juros'), findsNothing);
    });

    testWidgets('TS8: abrir "Nova tese" e voltar sem salvar não grava tese nem ativo', (tester) async {
      await pumpSection(tester);

      await tester.tap(find.byKey(const Key('asset-theses-new-button')));
      await settle(tester);
      // O "voltar" do sistema (gesto ou botão do Android), sem depender do botão do tema.
      tester.state<NavigatorState>(find.byType(Navigator)).pop();
      await settle(tester);

      expect(await tester.runAsync(repository.list), isEmpty);
      expect(await tester.runAsync(portfolio.listAssets), isEmpty);
    });
  });

  group('na busca de cotação', () {
    testWidgets('TQ1: Ação BR mostra o bloco de teses do ticker buscado', (tester) async {
      await tester.runAsync(() async {
        final petr = await addAsset('PETR4');
        await repository.create(title: 'Petrobras', assetId: petr);
      });
      await pumpSearch(tester);

      expect(find.byKey(const Key('asset-theses-section')), findsOneWidget);
      expect(find.text('Petrobras'), findsWidgets);
    });

    testWidgets('TQ2: Ação internacional também mostra o bloco', (tester) async {
      await pumpSearch(tester, assetClass: AssetClass.acaoInternacional);

      expect(find.byKey(const Key('asset-theses-section')), findsOneWidget);
    });

    testWidgets('TQ3: FII, ETF BR, BDR e cripto não mostram o bloco', (tester) async {
      for (final assetClass in [AssetClass.fii, AssetClass.etfBr, AssetClass.bdr, AssetClass.cripto]) {
        await pumpSearch(tester, assetClass: assetClass);
        expect(find.byKey(const Key('asset-theses-section')), findsNothing, reason: assetClass.value);
        expect(find.text('Petrobras'), findsOneWidget, reason: 'a busca em si funcionou');
      }
    });

    testWidgets('TQ4: em 390 px, com teses e cartão de cotação, nada estoura', (tester) async {
      await tester.runAsync(() async {
        final petr = await addAsset('PETR4');
        await repository.create(title: 'Petrobras 1', assetId: petr);
        await repository.create(title: 'Petrobras 2', assetId: petr);
      });
      await pumpSearch(tester, size: const Size(390, 844));

      expect(find.byKey(const Key('asset-theses-section')), findsOneWidget);
      expect(tester.takeException(), isNull);
    });
  });
}
