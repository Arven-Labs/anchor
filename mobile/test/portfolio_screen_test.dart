import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/l10n/app_localizations.dart';
import 'package:anchor_mobile/models/asset.dart';
import 'package:anchor_mobile/models/position.dart';
import 'package:anchor_mobile/screens/portfolio_screen.dart';
import 'package:anchor_mobile/screens/watchlist_screen.dart';
import 'package:anchor_mobile/services/live_positions.dart';
import 'package:anchor_mobile/services/portfolio_repository.dart';
import 'package:anchor_mobile/services/watchlist_repository.dart';

import 'support/localized_app.dart';

Future<AppLocalizations> _load(String code) => AppLocalizations.delegate.load(Locale(code));

LivePositionRow _row({
  String ticker = 'PETR4',
  double quantity = 10,
  double? avg = 20,
  double? price = 25.5,
  bool unavailable = false,
  AssetClass assetClass = AssetClass.acaoBr,
}) => LivePositionRow(
  Position(
    asset: Asset(
      id: 1,
      ticker: ticker,
      name: 'Petrobras',
      assetClass: assetClass,
      currency: 'BRL',
      createdAt: DateTime(2026, 1, 1),
    ),
    netQuantity: quantity,
    averageBuyPrice: avg,
  ),
  price,
  priceUnavailable: unavailable,
);

void main() {
  late AppLocalizations en;
  late AppLocalizations pt;

  setUpAll(() async {
    en = await _load('en');
    pt = await _load('pt');
  });

  group('livePriceLabel', () {
    test('pt: número com vírgula decimal, sem milhar', () {
      expect(livePriceLabel(pt, 3500.5), '3500,50');
      expect(livePriceLabel(pt, 10, unavailable: true), '10,00');
      expect(livePriceLabel(pt, 350000, currency: 'BRL'), 'BRL 350000,00');
      expect(livePriceLabel(pt, 70000.5, currency: 'USD'), 'USD 70000,50');
    });

    test('en: número com ponto decimal, sem milhar', () {
      expect(livePriceLabel(en, 3500.5), '3500.50');
      expect(livePriceLabel(en, 350000, currency: 'BRL'), 'BRL 350000.00');
      expect(livePriceLabel(en, 70000.5, currency: 'USD'), 'USD 70000.50');
    });

    test('sem preço: traço, ou "indisponível agora" quando a fonte falhou', () {
      expect(livePriceLabel(pt, null), '—');
      expect(livePriceLabel(pt, null, currency: 'BRL'), '—');
      expect(livePriceLabel(pt, null, unavailable: true), 'indisponível agora');
      expect(livePriceLabel(pt, null, unavailable: true, currency: 'BRL'), 'indisponível agora');
      expect(livePriceLabel(en, null), '—');
      expect(livePriceLabel(en, null, unavailable: true), 'unavailable right now');
    });
  });

  // A tela montada de verdade (SQLite ffi, posições injetadas, sem rede).
  group('a tela', () {
    late Directory dir;

    setUp(() async {
      dir = await Directory.systemTemp.createTemp('anchor-portfolio-screen-');
      await databaseFactory.setDatabasesPath(dir.path);
      await AppDatabase.resetForTest();
    });

    tearDown(() async {
      await AppDatabase.resetForTest();
      await dir.delete(recursive: true);
    });

    Future<void> pumpPortfolio(
      WidgetTester tester,
      List<LivePositionRow> rows, {
      String locale = 'pt',
    }) async {
      tester.view.physicalSize = const Size(800, 3000);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      await tester.runAsync(AppDatabase.instance);
      await tester.pumpWidget(
        localizedApp(PortfolioScreen(loadPositions: () async => rows), locale: locale),
      );
      for (var i = 0; i < 100; i++) {
        await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
        await tester.pump();
        if (find.byType(CircularProgressIndicator).evaluate().isEmpty) break;
      }
    }

    testWidgets('pt: preços e resultado com vírgula decimal', (tester) async {
      await pumpPortfolio(tester, [_row(quantity: 10, avg: 20, price: 25.5)]);

      expect(find.text('Portfolio'), findsOneWidget);
      expect(find.text('PETR4'), findsOneWidget);
      expect(find.text('10 un.'), findsOneWidget);
      expect(find.text('Preço médio: BRL 20,00'), findsOneWidget);
      expect(find.text('Preço atual: BRL 25,50'), findsOneWidget);
      expect(find.text('Valor: BRL 255,00'), findsOneWidget);
      // (25,5 − 20) × 10 = 55,00 sobre o custo de 200: +27,5%.
      expect(find.text('+55,00 (+27,5%)'), findsOneWidget);
    });

    testWidgets('en: o mesmo cartão em inglês, com ponto decimal', (tester) async {
      await pumpPortfolio(tester, [_row(quantity: 10, avg: 20, price: 25.5)], locale: 'en');

      expect(find.text('10 units'), findsOneWidget);
      expect(find.text('Average price: BRL 20.00'), findsOneWidget);
      expect(find.text('Current price: BRL 25.50'), findsOneWidget);
      expect(find.text('Value: BRL 255.00'), findsOneWidget);
      expect(find.text('+55.00 (+27.5%)'), findsOneWidget);
      for (final pt in ['un.', 'Preço', 'Valor:']) {
        expect(find.textContaining(pt), findsNothing, reason: pt);
      }
    });

    testWidgets('quantidade fracionária tem duas casas, com o separador do idioma', (tester) async {
      await pumpPortfolio(tester, [_row(quantity: 0.5, avg: null, price: null)]);
      expect(find.text('0,50 un.'), findsOneWidget);

      await pumpPortfolio(tester, [_row(quantity: 0.5, avg: null, price: null)], locale: 'en');
      expect(find.text('0.50 units'), findsOneWidget);
    });

    testWidgets('sem preço médio nem cotação: traços; fonte fora do ar: texto traduzido', (
      tester,
    ) async {
      await pumpPortfolio(tester, [_row(avg: null, price: null, unavailable: true)], locale: 'en');

      expect(find.text('Average price: —'), findsOneWidget);
      expect(find.text('Current price: unavailable right now'), findsOneWidget);
      expect(find.text('Value: —'), findsOneWidget);
    });

    testWidgets('sem ativos: aviso e botão traduzidos; tooltips da barra', (tester) async {
      await pumpPortfolio(tester, const [], locale: 'en');

      expect(find.text('No assets yet.'), findsOneWidget);
      expect(find.widgetWithText(FilledButton, 'Add asset'), findsOneWidget);
      expect(find.byTooltip('Custodians'), findsOneWidget);
      expect(find.byTooltip('History'), findsOneWidget);
      expect(find.byTooltip('Add asset'), findsOneWidget);
    });

    testWidgets('pt: aviso, botão e tooltips de antes', (tester) async {
      await pumpPortfolio(tester, const []);

      expect(find.text('Nenhum ativo cadastrado ainda.'), findsOneWidget);
      expect(find.widgetWithText(FilledButton, 'Adicionar ativo'), findsOneWidget);
      expect(find.byTooltip('Custódias'), findsOneWidget);
      expect(find.byTooltip('Histórico'), findsOneWidget);
    });

    testWidgets('tooltips de imposto aparecem só com a classe de ativo certa', (tester) async {
      await pumpPortfolio(
        tester,
        [_row(ticker: 'BTC', assetClass: AssetClass.cripto)],
        locale: 'en',
      );

      expect(find.byTooltip('Crypto tax'), findsOneWidget);
      expect(find.byTooltip('Swap crypto'), findsNothing); // só com duas criptos
      expect(find.byTooltip('Foreign tax'), findsNothing);
      expect(find.byTooltip('Fund tax'), findsNothing);
      expect(find.byTooltip('B3 tax'), findsNothing);
    });

    // Um teste por classe: um 2º `pumpWidget` no mesmo teste reaproveita o estado da tela
    // (mesmo tipo de widget, sem chave) e manteria as linhas da primeira rodada.
    for (final assetClass in [
      AssetClass.acaoBr,
      AssetClass.etfBr,
      AssetClass.bdr,
      AssetClass.fii,
    ]) {
      testWidgets('IR de B3 aparece com ativo da classe ${assetClass.name}', (tester) async {
        await pumpPortfolio(tester, [_row(ticker: 'XPTO3', assetClass: assetClass)], locale: 'en');
        expect(find.byTooltip('B3 tax'), findsOneWidget);
        expect(find.byTooltip('Crypto tax'), findsNothing);
      });
    }

    testWidgets('IR de B3 some com ativo do exterior, que tem o IR de exterior', (tester) async {
      await pumpPortfolio(
        tester,
        [_row(ticker: 'AAPL', assetClass: AssetClass.acaoInternacional)],
        locale: 'en',
      );
      expect(find.byTooltip('B3 tax'), findsNothing);
      expect(find.byTooltip('Foreign tax'), findsOneWidget);
    });

    // Tabela de testes 28: a estrela do card liga e desliga o favorito, e o ícone da barra
    // abre a tela de watchlists.
    testWidgets('28: a estrela marca e desmarca o favorito', (tester) async {
      final assetId = await tester.runAsync(
        () => PortfolioRepository().insertAsset(
          Asset(
            ticker: 'PETR4',
            name: 'Petrobras',
            assetClass: AssetClass.acaoBr,
            currency: 'BRL',
            createdAt: DateTime(2026, 1, 1),
          ),
        ),
      );
      await pumpPortfolio(tester, [_row(ticker: 'PETR4')]);
      final repository = WatchlistRepository();

      expect(find.byIcon(Icons.star_border), findsOneWidget);

      await tester.tap(find.byKey(Key('favorite-star-$assetId')));
      for (var i = 0; i < 10; i++) {
        await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
        await tester.pump();
      }
      expect(find.byIcon(Icons.star), findsOneWidget);
      expect((await tester.runAsync(repository.listFavorites))!.map((a) => a.ticker), ['PETR4']);

      await tester.tap(find.byKey(Key('favorite-star-$assetId')));
      for (var i = 0; i < 10; i++) {
        await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
        await tester.pump();
      }
      expect(find.byIcon(Icons.star_border), findsOneWidget);
      expect(await tester.runAsync(repository.listFavorites), isEmpty);
    });

    testWidgets('28: o ícone da barra abre a tela de watchlists', (tester) async {
      await pumpPortfolio(tester, [_row()]);

      await tester.tap(find.byTooltip('Watchlists'));
      for (var i = 0; i < 10; i++) {
        await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
        await tester.pump();
      }

      expect(find.byType(WatchlistScreen), findsOneWidget);
    });
  });
}
