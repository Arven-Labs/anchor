import 'dart:io';

import 'package:material_ui/material_ui.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/models/asset.dart';
import 'package:anchor_mobile/models/position.dart';
import 'package:anchor_mobile/screens/watchlist_screen.dart';
import 'package:anchor_mobile/services/live_positions.dart';
import 'package:anchor_mobile/services/portfolio_repository.dart';
import 'package:anchor_mobile/services/watchlist_repository.dart';

import 'support/localized_app.dart';
import 'support/settle.dart';

// A tela de watchlists montada de verdade (SQLite ffi, cotação injetada, sem rede).
// Tabela de testes 21 a 27.
void main() {
  final repository = WatchlistRepository();
  final portfolio = PortfolioRepository();
  late Directory dir;

  setUp(() async {
    dir = await Directory.systemTemp.createTemp('anchor-watchlist-screen-');
    await databaseFactory.setDatabasesPath(dir.path);
    await AppDatabase.resetForTest();
  });

  tearDown(() async {
    await AppDatabase.resetForTest();
    await dir.delete(recursive: true);
  });

  Future<int> addAsset(String ticker) => portfolio.insertAsset(
    Asset(
      ticker: ticker,
      name: 'Empresa $ticker',
      assetClass: AssetClass.acaoBr,
      currency: 'BRL',
      createdAt: DateTime(2026, 1, 1),
    ),
  );

  /// Cotação injetada por ticker; ticker fora do mapa = sem cotação; `unavailable` = fonte fora do ar.
  Future<List<LivePositionRow>> Function(List<Asset>) prices(
    Map<String, double> byTicker, {
    Set<String> unavailable = const {},
  }) => (assets) async => [
    for (final asset in assets)
      LivePositionRow(
        Position(asset: asset, netQuantity: 0, averageBuyPrice: null),
        byTicker[asset.ticker],
        priceUnavailable: unavailable.contains(asset.ticker),
      ),
  ];

  Future<void> pumpScreen(
    WidgetTester tester, {
    Future<List<LivePositionRow>> Function(List<Asset>)? loadPrices,
  }) async {
    tester.view.physicalSize = const Size(800, 3000);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    await tester.runAsync(AppDatabase.instance);
    await tester.pumpWidget(localizedApp(WatchlistScreen(loadPrices: loadPrices ?? prices(const {}))));
    await settle(tester);
  }

  testWidgets('21: sem nenhuma lista, avisa que não há watchlist', (tester) async {
    await pumpScreen(tester);
    await waitFor(tester, find.text('Nenhum favorito ainda.'), 'o carregamento da tela');
    expect(find.text('Nenhuma watchlist ainda.'), findsOneWidget);
    expect(find.text('Nenhum favorito ainda.'), findsOneWidget);
  });

  testWidgets('22: criar "Dividendos" já seleciona a lista', (tester) async {
    await pumpScreen(tester);

    await tester.enterText(find.byKey(const Key('watchlist-name-field')), 'Dividendos');
    await tester.tap(find.byKey(const Key('watchlist-create-button')));
    await waitFor(tester, find.byKey(const Key('watchlist-add-item-button')), 'a lista criada e selecionada');

    expect(find.text('Dividendos'), findsWidgets);
    expect(find.text('Nenhuma watchlist ainda.'), findsNothing);
    expect(find.byKey(const Key('watchlist-add-item-button')), findsOneWidget);
    expect((await tester.runAsync(repository.listWatchlists))!.map((w) => w.name), ['Dividendos']);
  });

  testWidgets('23: nome em branco mostra o erro e não cria', (tester) async {
    await pumpScreen(tester);

    await tester.enterText(find.byKey(const Key('watchlist-name-field')), '   ');
    await tester.tap(find.byKey(const Key('watchlist-create-button')));
    await waitFor(tester, find.text('Informe o nome da watchlist'), 'o erro de nome em branco');

    expect(find.text('Informe o nome da watchlist'), findsOneWidget);
    expect(await tester.runAsync(repository.listWatchlists), isEmpty);
  });

  testWidgets('24: apagar pede confirmação; só o segundo toque apaga', (tester) async {
    await tester.runAsync(() => repository.createWatchlist('Dividendos'));
    await pumpScreen(tester);

    await tester.tap(find.byKey(const Key('watchlist-delete-button')));
    await waitFor(tester, find.text('Confirmar?'), 'o pedido de confirmação');
    expect(find.text('Confirmar?'), findsOneWidget);
    expect(await tester.runAsync(repository.listWatchlists), hasLength(1));

    await tester.tap(find.byKey(const Key('watchlist-delete-button')));
    await waitFor(tester, find.text('Nenhuma watchlist ainda.'), 'a tela sem watchlist');
    expect(await tester.runAsync(repository.listWatchlists), isEmpty);
    expect(find.text('Nenhuma watchlist ainda.'), findsOneWidget);
  });

  testWidgets('25: adicionar PETR4 com alvo 30 mostra ticker, preço atual 38,20 e alvo 30,00', (tester) async {
    await tester.runAsync(() async {
      await addAsset('PETR4');
      await repository.createWatchlist('Dividendos');
    });
    await pumpScreen(tester, loadPrices: prices({'PETR4': 38.2}));

    await tester.tap(find.byKey(const Key('watchlist-add-item-button')));
    await waitFor(tester, find.byKey(const Key('watchlist-asset-field')), 'o diálogo de adicionar');
    await tester.tap(find.byKey(const Key('watchlist-asset-field')));
    await waitFor(tester, find.text('PETR4 — Empresa PETR4'), 'a lista de ativos aberta');
    await tester.tap(find.text('PETR4 — Empresa PETR4').last);
    await settle(tester);
    await tester.enterText(find.byKey(const Key('watchlist-target-field')), '30,00');
    await tester.tap(find.byKey(const Key('watchlist-item-confirm-button')));
    await waitFor(tester, find.text('Alvo: 30,00'), 'o item na lista');

    expect(find.text('PETR4'), findsOneWidget);
    expect(find.text('Preço atual: BRL 38,20'), findsOneWidget);
    expect(find.text('Alvo: 30,00'), findsOneWidget);
  });

  testWidgets('26: fonte fora do ar mostra "indisponível agora"; sem cotação mostra traço', (tester) async {
    await tester.runAsync(() async {
      final petr = await addAsset('PETR4');
      final vale = await addAsset('VALE3');
      final list = await repository.createWatchlist('Dividendos');
      await repository.addItem(watchlistId: list, assetId: petr);
      await repository.addItem(watchlistId: list, assetId: vale);
    });
    await pumpScreen(tester, loadPrices: prices(const {}, unavailable: {'PETR4'}));
    await waitFor(tester, find.text('Preço atual: indisponível agora'), 'as cotações carregadas');

    expect(find.text('Preço atual: indisponível agora'), findsOneWidget);
    expect(find.text('Preço atual: —'), findsOneWidget);
  });

  testWidgets('27: o bloco Favoritos mostra ticker e preço; Remover tira o favorito', (tester) async {
    await tester.runAsync(() async {
      final petr = await addAsset('PETR4');
      await repository.toggleFavorite(petr);
    });
    await pumpScreen(tester, loadPrices: prices({'PETR4': 38.2}));
    await waitFor(tester, find.text('Preço atual: BRL 38,20'), 'o favorito com preço');

    expect(find.byKey(const Key('favorites-block')), findsOneWidget);
    expect(find.descendant(of: find.byKey(const Key('favorites-block')), matching: find.text('PETR4')), findsOneWidget);
    expect(find.text('Preço atual: BRL 38,20'), findsOneWidget);

    await tester.tap(find.byKey(const Key('favorite-remove-PETR4')));
    await waitFor(tester, find.text('Nenhum favorito ainda.'), 'o favorito removido da tela');

    expect(await tester.runAsync(repository.listFavorites), isEmpty);
    expect(find.text('Nenhum favorito ainda.'), findsOneWidget);
  });
}
