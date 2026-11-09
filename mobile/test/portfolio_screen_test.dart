import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/l10n/app_localizations.dart';
import 'package:anchor_mobile/models/asset.dart';
import 'package:anchor_mobile/models/position.dart';
import 'package:anchor_mobile/screens/custodias_screen.dart';
import 'package:anchor_mobile/screens/portfolio_screen.dart';
import 'package:anchor_mobile/screens/theses_screen.dart';
import 'package:anchor_mobile/screens/transaction_history_screen.dart';
import 'package:anchor_mobile/screens/watchlist_screen.dart';
import 'package:anchor_mobile/services/live_positions.dart';
import 'package:anchor_mobile/services/portfolio_repository.dart';
import 'package:anchor_mobile/services/watchlist_repository.dart';

import 'support/localized_app.dart';

Future<AppLocalizations> _load(String code) => AppLocalizations.delegate.load(Locale(code));

LivePositionRow _row({
  int id = 1,
  String ticker = 'PETR4',
  double quantity = 10,
  double? avg = 20,
  double? price = 25.5,
  bool unavailable = false,
  AssetClass assetClass = AssetClass.acaoBr,
}) => LivePositionRow(
  Position(
    asset: Asset(
      id: id,
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

    // Os atalhos (Rentabilidade, Teses, Watchlists, IR, Permuta, Custódias, Histórico) moram
    // num menu "Mais" da barra: a barra não cabe um ícone por atalho em celular estreito (390 px).
    Future<void> openMoreMenu(WidgetTester tester, String tooltip) async {
      await tester.tap(find.byTooltip(tooltip));
      for (var i = 0; i < 10; i++) {
        await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
        await tester.pump(const Duration(milliseconds: 50));
      }
    }

    // Os 5 itens condicionais do menu (IR e Permuta), pelo rótulo em inglês.
    const conditionalItems = ['B3 tax', 'Crypto tax', 'Foreign tax', 'Fund tax', 'Swap crypto'];

    void expectConditionalItems(Set<String> shown) {
      for (final item in conditionalItems) {
        expect(
          find.text(item),
          shown.contains(item) ? findsOneWidget : findsNothing,
          reason: item,
        );
      }
    }

    testWidgets('sem ativos: aviso e botão traduzidos; na barra só "More" e "Add asset"', (
      tester,
    ) async {
      await pumpPortfolio(tester, const [], locale: 'en');

      expect(find.text('No assets yet.'), findsOneWidget);
      expect(find.widgetWithText(FilledButton, 'Add asset'), findsOneWidget);
      expect(find.byTooltip('More'), findsOneWidget);
      expect(find.byTooltip('Add asset'), findsOneWidget);
      for (final tooltip in ['Custodians', 'History', ...conditionalItems]) {
        expect(find.byTooltip(tooltip), findsNothing, reason: tooltip);
      }
    });

    testWidgets('sem ativos: o menu tem Theses, Watchlists, Custodians e History, sem itens de IR', (
      tester,
    ) async {
      await pumpPortfolio(tester, const [], locale: 'en');
      await openMoreMenu(tester, 'More');

      for (final item in ['Theses', 'Watchlists', 'Custodians', 'History']) {
        expect(find.text(item), findsOneWidget, reason: item);
      }
      expectConditionalItems({});
    });

    testWidgets('pt: aviso, botão e itens do menu em português', (tester) async {
      await pumpPortfolio(tester, const []);

      expect(find.text('Nenhum ativo cadastrado ainda.'), findsOneWidget);
      expect(find.widgetWithText(FilledButton, 'Adicionar ativo'), findsOneWidget);
      expect(find.byTooltip('Custódias'), findsNothing);
      expect(find.byTooltip('Histórico'), findsNothing);

      await openMoreMenu(tester, 'Mais');
      expect(find.text('Custódias'), findsOneWidget);
      expect(find.text('Histórico'), findsOneWidget);
    });

    testWidgets('com uma cripto: o menu tem IR de cripto e não tem Permuta', (tester) async {
      await pumpPortfolio(
        tester,
        [_row(ticker: 'BTC', assetClass: AssetClass.cripto)],
        locale: 'en',
      );
      await openMoreMenu(tester, 'More');

      expectConditionalItems({'Crypto tax'}); // Permuta só com duas criptos
    });

    testWidgets('com duas criptos: o menu tem IR de cripto e Permuta', (tester) async {
      await pumpPortfolio(
        tester,
        [
          _row(ticker: 'BTC', assetClass: AssetClass.cripto),
          _row(id: 2, ticker: 'ETH', assetClass: AssetClass.cripto),
        ],
        locale: 'en',
      );
      await openMoreMenu(tester, 'More');

      expectConditionalItems({'Crypto tax', 'Swap crypto'});
    });

    testWidgets('com um fundo: o menu tem só IR de fundos', (tester) async {
      await pumpPortfolio(
        tester,
        [_row(ticker: 'FUNDO1', assetClass: AssetClass.fundo)],
        locale: 'en',
      );
      await openMoreMenu(tester, 'More');

      expectConditionalItems({'Fund tax'});
    });

    // Um teste por classe: um 2º `pumpWidget` no mesmo teste reaproveita o estado da tela
    // (mesmo tipo de widget, sem chave) e manteria as linhas da primeira rodada.
    for (final assetClass in [
      AssetClass.acaoBr,
      AssetClass.etfBr,
      AssetClass.bdr,
      AssetClass.fii,
    ]) {
      testWidgets('IR de B3 aparece no menu com ativo da classe ${assetClass.name}', (
        tester,
      ) async {
        await pumpPortfolio(tester, [_row(ticker: 'XPTO3', assetClass: assetClass)], locale: 'en');
        await openMoreMenu(tester, 'More');
        expectConditionalItems({'B3 tax'});
      });
    }

    testWidgets('IR de B3 some do menu com ativo do exterior, que tem o IR de exterior', (
      tester,
    ) async {
      await pumpPortfolio(
        tester,
        [_row(ticker: 'AAPL', assetClass: AssetClass.acaoInternacional)],
        locale: 'en',
      );
      await openMoreMenu(tester, 'More');
      expectConditionalItems({'Foreign tax'});
    });

    // Tocar no item abre a tela e, ao voltar, a Carteira recarrega as posições.
    for (final entry in {
      'Custodians': CustodiasScreen,
      'History': TransactionHistoryScreen,
    }.entries) {
      testWidgets('o item ${entry.key} do menu abre a tela e recarrega ao voltar', (tester) async {
        var loads = 0;
        tester.view.physicalSize = const Size(800, 3000);
        tester.view.devicePixelRatio = 1;
        addTearDown(tester.view.reset);
        await tester.runAsync(AppDatabase.instance);
        await tester.pumpWidget(
          localizedApp(
            PortfolioScreen(
              loadPositions: () async {
                loads++;
                return [_row()];
              },
            ),
            locale: 'en',
          ),
        );
        for (var i = 0; i < 20; i++) {
          await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
          await tester.pump();
        }
        final loadsBefore = loads;

        await openMoreMenu(tester, 'More');
        await tester.tap(find.text(entry.key));
        for (var i = 0; i < 20; i++) {
          await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
          await tester.pump(const Duration(milliseconds: 50));
        }
        expect(find.byType(entry.value), findsOneWidget);

        Navigator.of(tester.element(find.byType(entry.value))).pop();
        for (var i = 0; i < 20; i++) {
          await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
          await tester.pump(const Duration(milliseconds: 50));
        }
        expect(loads, greaterThan(loadsBefore));
      });
    }

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

    // Tabela de testes B20: o item "Rentabilidade" só aparece com ativo de classe medida.
    testWidgets('B20: item Rentabilidade aparece no menu com ação', (tester) async {
      await pumpPortfolio(tester, [_row()], locale: 'en');
      await openMoreMenu(tester, 'More');
      expect(find.text('Profitability'), findsOneWidget);
    });

    testWidgets('B20: só com cripto o item Rentabilidade não aparece (Teses e Watchlists sim)', (tester) async {
      await pumpPortfolio(tester, [_row(ticker: 'BTC', assetClass: AssetClass.cripto)], locale: 'en');
      await openMoreMenu(tester, 'More');
      expect(find.text('Profitability'), findsNothing);
      expect(find.text('Theses'), findsOneWidget);
      expect(find.text('Watchlists'), findsOneWidget);
    });

    // Tabela de testes TC1: o item "Teses" abre a tela de teses.
    testWidgets('TC1: o item Teses do menu abre a tela de teses', (tester) async {
      await pumpPortfolio(tester, [_row()], locale: 'en');
      await openMoreMenu(tester, 'More');

      await tester.tap(find.text('Theses'));
      for (var i = 0; i < 10; i++) {
        await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
        await tester.pump(const Duration(milliseconds: 50));
      }

      expect(find.byType(ThesesScreen), findsOneWidget);
    });

    testWidgets('28: o item Watchlists do menu abre a tela de watchlists', (tester) async {
      await pumpPortfolio(tester, [_row()]);
      await openMoreMenu(tester, 'Mais');

      await tester.tap(find.text('Watchlists'));
      for (var i = 0; i < 10; i++) {
        await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
        await tester.pump(const Duration(milliseconds: 50));
      }

      expect(find.byType(WatchlistScreen), findsOneWidget);
    });

    // Regressão do CI das capturas: ação + exterior + cripto (três atalhos de IR) estouravam a
    // barra em 42 px numa tela de 390 px quando cada atalho novo era um ícone. Com o menu
    // "Mais" são 7 ícones (336 px). Carteira com fundo e duas criptos (9 ícones) ainda não cabe.
    testWidgets('a barra da Carteira cabe em 390 px com ação, exterior e cripto', (tester) async {
      tester.view.physicalSize = const Size(390, 3000);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      await tester.runAsync(AppDatabase.instance);
      await tester.pumpWidget(
        localizedApp(
          PortfolioScreen(
            loadPositions: () async => [
              // Sem preço nem custo: com a fonte Ahem do flutter_test (cada letra do tamanho do
              // corpo) a linha "Valor / resultado" do card estoura em 390 px e mascararia a barra.
              _row(ticker: 'PETR4', avg: null, price: null),
              _row(ticker: 'AAPL', assetClass: AssetClass.acaoInternacional, avg: null, price: null),
              _row(ticker: 'BTC', assetClass: AssetClass.cripto, avg: null, price: null),
            ],
          ),
          locale: 'en',
        ),
      );
      for (var i = 0; i < 20; i++) {
        await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
        await tester.pump(const Duration(milliseconds: 50));
      }

      expect(tester.takeException(), isNull);
    });
  });
}
