import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/models/asset.dart';
import 'package:anchor_mobile/models/portfolio_transaction.dart';
import 'package:anchor_mobile/services/live_positions.dart';
import 'package:anchor_mobile/services/portfolio_repository.dart';
import 'package:anchor_mobile/services/quote_dispatcher.dart';

// A empresa não listada no patrimônio, lendo a avaliação manual do banco de
// verdade (SQLite ffi, sem rede: essa classe não chama o dispatcher).
void main() {
  final repository = PortfolioRepository();
  late Directory dir;

  setUp(() async {
    dir = await Directory.systemTemp.createTemp('anchor-live-manual-');
    await databaseFactory.setDatabasesPath(dir.path);
    await AppDatabase.resetForTest();
  });

  tearDown(() async {
    await AppDatabase.resetForTest();
    await dir.delete(recursive: true);
  });

  Future<int> company(String ticker) async {
    final id = await repository.insertAsset(
      Asset(
        ticker: ticker,
        name: ticker,
        assetClass: AssetClass.empresaNaoListada,
        currency: 'BRL',
        createdAt: DateTime(2026, 1, 1),
      ),
    );
    await repository.insertTransaction(
      PortfolioTransaction(
        assetId: id,
        type: TransactionType.compra,
        quantity: 4, // não multiplica o valor total da avaliação
        unitPrice: 100,
        totalValue: 400,
        date: DateTime(2026, 1, 1),
        createdAt: DateTime(2026, 1, 1),
      ),
    );
    return id;
  }

  Future<List<LivePositionRow>> load() async => loadLivePositions(
    await repository.getPositions(),
    repository: repository,
    dispatcher: QuoteDispatcher(),
  );

  test('vale a última avaliação como total, sem multiplicar pela quantidade', () async {
    final id = await company('XYZ');
    await repository.insertAssetValuation(
      assetId: id,
      valuationDate: DateTime(2026, 2, 1),
      value: 40000,
    );
    await repository.insertAssetValuation(
      assetId: id,
      valuationDate: DateTime(2026, 6, 1),
      value: 55000,
    );

    final rows = await load();

    expect(rows.single.manualTotalValue, 55000);
    expect(rows.single.livePrice, isNull); // o card do Portfolio não ganha preço atual
    final items = portfolioValuedItems(rows);
    expect(items.single.currency, 'BRL');
    expect(items.single.value, 55000);
  });

  test('sem avaliação lançada entra com valor nulo, para contar como fora', () async {
    await company('SEM');

    final items = portfolioValuedItems(await load());

    expect(items.single.value, isNull);
  });
}
