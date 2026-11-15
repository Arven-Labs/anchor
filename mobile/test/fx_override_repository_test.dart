import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/models/asset.dart';
import 'package:anchor_mobile/models/portfolio_transaction.dart';
import 'package:anchor_mobile/services/portfolio_repository.dart';

// Câmbio digitado (P16, schema v27): a coluna `fx_rate_override`, a recusa de taxa inválida e a
// migração do banco da v26. Dados fictícios.
void main() {
  final repository = PortfolioRepository();
  late Directory dir;

  setUp(() async {
    dir = await Directory.systemTemp.createTemp('anchor-fx-override-test-');
    await databaseFactory.setDatabasesPath(dir.path);
    await AppDatabase.resetForTest();
  });

  tearDown(() async {
    await AppDatabase.resetForTest();
    await dir.delete(recursive: true);
  });

  Future<int> addUsdAsset() => repository.insertAsset(
    Asset(
      ticker: 'AAPL',
      name: 'Apple',
      assetClass: AssetClass.acaoInternacional,
      currency: 'USD',
      createdAt: DateTime(2024, 1, 1),
    ),
  );

  PortfolioTransaction buy(int assetId, {double? fxRate, int? id}) => PortfolioTransaction(
    id: id,
    assetId: assetId,
    type: TransactionType.compra,
    quantity: 10,
    unitPrice: 100,
    totalValue: 1000,
    fxRateOverride: fxRate,
    date: DateTime(2024, 3, 11),
    createdAt: DateTime(2024, 3, 11),
  );

  test('M1: o câmbio digitado grava e volta pelo banco', () async {
    final assetId = await addUsdAsset();
    await repository.insertTransaction(buy(assetId, fxRate: 5.5));

    final saved = (await repository.listAllTransactions()).single;

    expect(saved.fxRateOverride, 5.5);
  });

  test('M2: editar a transação troca e apaga o câmbio digitado', () async {
    final assetId = await addUsdAsset();
    final id = await repository.insertTransaction(buy(assetId, fxRate: 5.5));

    await repository.updateTransaction(buy(assetId, fxRate: 6, id: id));
    expect((await repository.listAllTransactions()).single.fxRateOverride, 6);

    await repository.updateTransaction(buy(assetId, id: id));
    expect((await repository.listAllTransactions()).single.fxRateOverride, isNull);
  });

  test('M3: taxa zero, negativa, NaN ou infinita é recusada e nada é gravado', () async {
    final assetId = await addUsdAsset();
    final id = await repository.insertTransaction(buy(assetId, fxRate: 5.5));

    for (final bad in [0.0, -1.0, double.nan, double.infinity]) {
      await expectLater(
        repository.insertTransaction(buy(assetId, fxRate: bad)),
        throwsFormatException,
        reason: 'inserir $bad',
      );
      await expectLater(
        repository.updateTransaction(buy(assetId, fxRate: bad, id: id)),
        throwsFormatException,
        reason: 'editar $bad',
      );
      await expectLater(
        repository.insertTransactions([buy(assetId, fxRate: bad)]),
        throwsFormatException,
        reason: 'inserir em lote $bad',
      );
    }

    final all = await repository.listAllTransactions();
    expect(all, hasLength(1));
    expect(all.single.fxRateOverride, 5.5);
  });

  test('M4: um banco da v26 migra para a v27 com o câmbio automático e os dados intactos', () async {
    // O banco como o app o deixava na v26: só o que a migração e a leitura precisam.
    final path = '${dir.path}/anchor_mobile.db';
    final old = await databaseFactory.openDatabase(
      path,
      options: OpenDatabaseOptions(
        version: 26,
        onCreate: (db, _) async {
          await db.execute('''
            CREATE TABLE portfolio_transactions (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              asset_id INTEGER,
              transaction_type TEXT NOT NULL,
              quantity REAL,
              unit_price REAL,
              total_value REAL NOT NULL,
              transaction_date TEXT NOT NULL,
              created_at TEXT NOT NULL
            )
          ''');
          await db.insert('portfolio_transactions', {
            'transaction_type': 'compra',
            'quantity': 10.0,
            'unit_price': 100.0,
            'total_value': 1000.0,
            'transaction_date': '2024-03-11T00:00:00.000',
            'created_at': '2024-03-11T00:00:00.000',
          });
        },
      ),
    );
    await old.close();

    final all = await repository.listAllTransactions();

    expect(all, hasLength(1));
    expect(all.single.totalValue, 1000);
    expect(all.single.fxRateOverride, isNull);
  });
}
