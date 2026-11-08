import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/domain/finance_error.dart';
import 'package:anchor_mobile/models/asset.dart';
import 'package:anchor_mobile/services/portfolio_repository.dart';
import 'package:anchor_mobile/services/watchlist_repository.dart';

// Persistência das watchlists e dos favoritos (tabelas do schema v23). Dados fictícios.
Matcher _argumentError(FinanceErrorCode code) =>
    throwsA(isA<FinanceArgumentError>().having((e) => e.code, 'code', code));

void main() {
  final repository = WatchlistRepository();
  final portfolio = PortfolioRepository();
  late Directory dir;

  setUp(() async {
    dir = await Directory.systemTemp.createTemp('anchor-watchlist-test-');
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

  Future<int> count(String table, {String? where}) async {
    final db = await AppDatabase.instance();
    final rows = await db.rawQuery('SELECT COUNT(*) AS n FROM $table${where == null ? '' : ' WHERE $where'}');
    return rows.single['n']! as int;
  }

  group('banco e migração', () {
    test('7: banco novo nasce com as três tabelas vazias', () async {
      expect(await count('watchlists'), 0);
      expect(await count('watchlist_items'), 0);
      expect(await count('asset_favorites'), 0);
    });

    test('8: banco v22 migra sem perder ativo nem transação e ganha as tabelas', () async {
      final old = await openDatabase(
        '${dir.path}/anchor_mobile.db',
        version: 22,
        onCreate: (db, _) async {
          await db.execute('''
            CREATE TABLE assets (
              id INTEGER PRIMARY KEY AUTOINCREMENT, ticker TEXT NOT NULL,
              name TEXT NOT NULL, asset_class TEXT NOT NULL,
              currency TEXT NOT NULL, created_at TEXT NOT NULL)''');
          await db.execute('''
            CREATE TABLE portfolio_transactions (
              id INTEGER PRIMARY KEY AUTOINCREMENT, asset_id INTEGER NOT NULL,
              type TEXT NOT NULL)''');
          await db.insert('assets', {
            'ticker': 'PETR4',
            'name': 'Petrobras',
            'asset_class': 'acao_br',
            'currency': 'BRL',
            'created_at': '2026-01-01T00:00:00.000',
          });
          await db.insert('portfolio_transactions', {'asset_id': 1, 'type': 'compra'});
        },
      );
      await old.close();

      expect(await repository.listWatchlists(), isEmpty);
      expect(await count('watchlists'), 0);
      expect(await count('asset_favorites'), 0);
      expect(await count('assets', where: "ticker = 'PETR4'"), 1);
      expect(await count('portfolio_transactions'), 1);
    });

    test('reabrir o banco não apaga as listas', () async {
      await repository.createWatchlist('Dividendos');
      await AppDatabase.resetForTest();
      expect((await repository.listWatchlists()).map((w) => w.name), ['Dividendos']);
    });
  });

  group('listas', () {
    test('9: cria duas listas; nome repetido é permitido; o nome vem sem espaços nas pontas', () async {
      await repository.createWatchlist(' Dividendos ');
      await repository.createWatchlist('Turnarounds');
      await repository.createWatchlist('Turnarounds');
      expect((await repository.listWatchlists()).map((w) => w.name), ['Dividendos', 'Turnarounds', 'Turnarounds']);
    });

    test('nome em branco é recusado e nada é gravado', () async {
      await expectLater(repository.createWatchlist('  '), _argumentError(FinanceErrorCode.watchlistNameRequired));
      expect(await count('watchlists'), 0);
    });

    test('10: apagar uma lista leva os itens dela e preserva a outra', () async {
      final petr = await addAsset('PETR4');
      final vale = await addAsset('VALE3');
      final dividendos = await repository.createWatchlist('Dividendos');
      final turnarounds = await repository.createWatchlist('Turnarounds');
      await repository.addItem(watchlistId: dividendos, assetId: petr);
      await repository.addItem(watchlistId: dividendos, assetId: vale);
      await repository.addItem(watchlistId: turnarounds, assetId: petr);

      await repository.deleteWatchlist(dividendos);

      expect((await repository.listWatchlists()).map((w) => w.id), [turnarounds]);
      expect(await count('watchlist_items', where: 'watchlist_id = $dividendos'), 0);
      expect((await repository.listItems(turnarounds)).map((e) => e.asset.ticker), ['PETR4']);
    });
  });

  group('itens', () {
    test('11: adicionar guarda ativo, alvo e anotação', () async {
      final petr = await addAsset('PETR4');
      final list = await repository.createWatchlist('Dividendos');

      await repository.addItem(watchlistId: list, assetId: petr, targetPrice: 30, notes: ' ver Selic ');

      final entry = (await repository.listItems(list)).single;
      expect(entry.asset.ticker, 'PETR4');
      expect(entry.item.targetPrice, 30);
      expect(entry.item.notes, 'ver Selic');
    });

    test('12: o mesmo ativo duas vezes na mesma lista é recusado e continua uma linha', () async {
      final petr = await addAsset('PETR4');
      final list = await repository.createWatchlist('Dividendos');
      await repository.addItem(watchlistId: list, assetId: petr);

      await expectLater(
        repository.addItem(watchlistId: list, assetId: petr),
        _argumentError(FinanceErrorCode.assetAlreadyInWatchlist),
      );
      expect(await count('watchlist_items'), 1);
    });

    test('13: o mesmo ativo em duas listas diferentes é aceito', () async {
      final petr = await addAsset('PETR4');
      final a = await repository.createWatchlist('A');
      final b = await repository.createWatchlist('B');
      await repository.addItem(watchlistId: a, assetId: petr);
      await repository.addItem(watchlistId: b, assetId: petr);
      expect(await count('watchlist_items'), 2);
    });

    test('14: editar muda alvo e anotação; com alvo em branco o alvo volta a null', () async {
      final petr = await addAsset('PETR4');
      final list = await repository.createWatchlist('Dividendos');
      final id = await repository.addItem(watchlistId: list, assetId: petr, targetPrice: 30, notes: 'a');

      await repository.updateItem(id, targetPrice: 35.5, notes: 'b');
      var entry = (await repository.listItems(list)).single;
      expect((entry.item.targetPrice, entry.item.notes), (35.5, 'b'));

      await repository.updateItem(id, targetPrice: null, notes: '  ');
      entry = (await repository.listItems(list)).single;
      expect((entry.item.targetPrice, entry.item.notes), (null, null));
    });

    test('15: remover o item não apaga o ativo nem o item de outra lista', () async {
      final petr = await addAsset('PETR4');
      final a = await repository.createWatchlist('A');
      final b = await repository.createWatchlist('B');
      final inA = await repository.addItem(watchlistId: a, assetId: petr);
      await repository.addItem(watchlistId: b, assetId: petr);

      await repository.removeItem(inA);

      expect(await repository.listItems(a), isEmpty);
      expect(await repository.listItems(b), hasLength(1));
      expect(await portfolio.listAssets(), hasLength(1));
    });

    test('16: alvo negativo ou zero é recusado ao adicionar e ao editar, sem gravar nada', () async {
      final petr = await addAsset('PETR4');
      final list = await repository.createWatchlist('Dividendos');

      await expectLater(
        repository.addItem(watchlistId: list, assetId: petr, targetPrice: -1),
        _argumentError(FinanceErrorCode.targetPriceMustBePositive),
      );
      expect(await count('watchlist_items'), 0);

      final id = await repository.addItem(watchlistId: list, assetId: petr, targetPrice: 30);
      await expectLater(
        repository.updateItem(id, targetPrice: 0, notes: 'x'),
        _argumentError(FinanceErrorCode.targetPriceMustBePositive),
      );
      final entry = (await repository.listItems(list)).single;
      expect((entry.item.targetPrice, entry.item.notes), (30, null));
    });

    test('17: ativo apagado do cadastro some da lista, sem erro', () async {
      final petr = await addAsset('PETR4');
      final vale = await addAsset('VALE3');
      final list = await repository.createWatchlist('Dividendos');
      await repository.addItem(watchlistId: list, assetId: petr);
      await repository.addItem(watchlistId: list, assetId: vale);

      await portfolio.deleteAsset(petr);

      expect((await repository.listItems(list)).map((e) => e.asset.ticker), ['VALE3']);
    });
  });

  group('favoritos', () {
    test('18: tocar duas vezes liga e desliga, e a lista acompanha', () async {
      final petr = await addAsset('PETR4');

      expect(await repository.toggleFavorite(petr), isTrue);
      expect((await repository.listFavorites()).map((a) => a.ticker), ['PETR4']);

      expect(await repository.toggleFavorite(petr), isFalse);
      expect(await repository.listFavorites(), isEmpty);
    });

    test('19: dois toques concorrentes deixam no máximo uma linha', () async {
      final petr = await addAsset('PETR4');
      await Future.wait([repository.toggleFavorite(petr), repository.toggleFavorite(petr)]);
      expect(await count('asset_favorites', where: 'asset_id = $petr'), lessThanOrEqualTo(1));
    });

    test('20: favorito e watchlist são independentes', () async {
      final petr = await addAsset('PETR4');
      final list = await repository.createWatchlist('Dividendos');
      await repository.toggleFavorite(petr);
      expect(await repository.listItems(list), isEmpty);

      await repository.toggleFavorite(petr);
      await repository.addItem(watchlistId: list, assetId: petr);
      expect(await repository.listFavorites(), isEmpty);
    });

    test('favorito de ativo apagado do cadastro some da lista', () async {
      final petr = await addAsset('PETR4');
      await repository.toggleFavorite(petr);
      await portfolio.deleteAsset(petr);
      expect(await repository.listFavorites(), isEmpty);
    });
  });
}
