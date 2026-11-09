import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/domain/fx_history.dart';
import 'package:anchor_mobile/domain/profitability_error.dart';
import 'package:anchor_mobile/domain/yahoo_monthly_history.dart';
import 'package:anchor_mobile/models/asset.dart';
import 'package:anchor_mobile/models/portfolio_transaction.dart';
import 'package:anchor_mobile/services/portfolio_repository.dart';
import 'package:anchor_mobile/services/profitability_repository.dart';
import 'package:anchor_mobile/services/quote_exceptions.dart';

// Preço mensal guardado (tabelas do schema v24) e a montagem da rentabilidade a partir dele.
// Tabela de testes B7 a B14. Dados fictícios, sem rede (Yahoo e câmbio injetados).
void main() {
  final portfolio = PortfolioRepository();
  late Directory dir;
  late List<String> fetchedSymbols;
  late List<(String, DateTime)> fxCalls;
  late Map<String, MonthlyPriceHistory> histories;
  late Set<String> failing;

  ProfitabilityRepository repo({DateTime? now}) => ProfitabilityRepository(
    portfolio: portfolio,
    now: () => now ?? DateTime(2026, 1, 31),
    fetchHistory: (symbol) async {
      fetchedSymbols.add(symbol);
      if (failing.contains(symbol)) throw QuoteNotFoundException(symbol);
      return histories[symbol] ?? const MonthlyPriceHistory(closes: {}, splits: []);
    },
    fetchFx: (currency, from) async {
      fxCalls.add((currency, from));
      return FxHistory([(DateTime.utc(2026, 1, 10), 5.0), (DateTime.utc(2026, 1, 31), 5.5)]);
    },
  );

  setUp(() async {
    dir = await Directory.systemTemp.createTemp('anchor-profitability-test-');
    await databaseFactory.setDatabasesPath(dir.path);
    await AppDatabase.resetForTest();
    fetchedSymbols = [];
    fxCalls = [];
    histories = {};
    failing = {};
  });

  tearDown(() async {
    await AppDatabase.resetForTest();
    await dir.delete(recursive: true);
  });

  Future<int> addAsset(String ticker, AssetClass cls, {String currency = 'BRL'}) => portfolio.insertAsset(
    Asset(ticker: ticker, name: ticker, assetClass: cls, currency: currency, createdAt: DateTime(2026, 1, 1)),
  );

  Future<void> buy(int assetId, DateTime date, double qty, double total) => portfolio
      .insertTransaction(
        PortfolioTransaction(
          assetId: assetId,
          type: TransactionType.compra,
          quantity: qty,
          totalValue: total,
          date: date,
          createdAt: date,
        ),
      )
      .then((_) {});

  Future<int> count(String table, {String? where}) async {
    final db = await AppDatabase.instance();
    final rows = await db.rawQuery('SELECT COUNT(*) AS n FROM $table${where == null ? '' : ' WHERE $where'}');
    return rows.single['n']! as int;
  }

  Future<Map<String, double>> storedCloses(String ticker) async {
    final db = await AppDatabase.instance();
    final rows = await db.query('price_month_close', where: 'ticker = ?', whereArgs: [ticker]);
    return {for (final r in rows) r['month']! as String: r['close']! as double};
  }

  group('banco e migração', () {
    test('B7: banco novo nasce com as duas tabelas vazias', () async {
      expect(await count('price_month_close'), 0);
      expect(await count('price_split_event'), 0);
    });

    test('B7: banco v23 migra sem perder ativo nem transação e ganha as tabelas', () async {
      final old = await openDatabase(
        '${dir.path}/anchor_mobile.db',
        version: 23,
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

      expect(await count('price_month_close'), 0);
      expect(await count('price_split_event'), 0);
      expect(await count('assets', where: "ticker = 'PETR4'"), 1);
      expect(await count('portfolio_transactions'), 1);
    });
  });

  group('atualizar o histórico', () {
    test('B8: só busca ativo de classe medida que tenha transação', () async {
      final petr = await addAsset('PETR4', AssetClass.acaoBr);
      final aapl = await addAsset('AAPL', AssetClass.acaoInternacional, currency: 'USD');
      final btc = await addAsset('BTC', AssetClass.cripto);
      final fund = await addAsset('FIDX', AssetClass.fundo);
      await addAsset('VALE3', AssetClass.acaoBr); // cadastrado, sem transação
      for (final id in [petr, aapl, btc, fund]) {
        await buy(id, DateTime(2026, 1, 10), 1, 100);
      }
      histories = {
        'PETR4.SA': const MonthlyPriceHistory(closes: {'2026-01': 32.0}, splits: []),
        'AAPL': const MonthlyPriceHistory(closes: {'2026-01': 110.0}, splits: []),
      };

      final summary = await repo().refreshPriceHistory();

      expect(fetchedSymbols, unorderedEquals(['PETR4.SA', 'AAPL']));
      expect(summary.updated, unorderedEquals(['PETR4', 'AAPL']));
      expect(summary.failed, isEmpty);
      expect(await storedCloses('PETR4'), {'2026-01': 32.0});
      expect(await storedCloses('AAPL'), {'2026-01': 110.0});
    });

    test('B9: atualizar duas vezes mantém o que o Yahoo mandou por último', () async {
      final petr = await addAsset('PETR4', AssetClass.acaoBr);
      await buy(petr, DateTime(2026, 1, 10), 10, 300);
      histories = {'PETR4.SA': const MonthlyPriceHistory(closes: {'2026-01': 30.0, '2026-02': 31.0}, splits: [])};
      await repo().refreshPriceHistory();
      await repo().refreshPriceHistory();
      expect(await count('price_month_close', where: "ticker = 'PETR4'"), 2);

      histories = {'PETR4.SA': const MonthlyPriceHistory(closes: {'2026-01': 30.5}, splits: [])};
      await repo().refreshPriceHistory();

      expect(await storedCloses('PETR4'), {'2026-01': 30.5});
    });

    test('B10: um ticker que falha não derruba os outros e mantém os dados antigos dele', () async {
      final petr = await addAsset('PETR4', AssetClass.acaoBr);
      final vale = await addAsset('VALE3', AssetClass.acaoBr);
      await buy(petr, DateTime(2026, 1, 10), 10, 300);
      await buy(vale, DateTime(2026, 1, 10), 10, 200);
      histories = {
        'PETR4.SA': const MonthlyPriceHistory(closes: {'2026-01': 32.0}, splits: []),
        'VALE3.SA': const MonthlyPriceHistory(closes: {'2026-01': 21.0}, splits: []),
      };
      await repo().refreshPriceHistory();

      failing = {'VALE3.SA'};
      histories['PETR4.SA'] = const MonthlyPriceHistory(closes: {'2026-01': 33.0}, splits: []);
      final summary = await repo().refreshPriceHistory();

      expect(summary.updated, ['PETR4']);
      expect(summary.failed, ['VALE3']);
      expect(await storedCloses('PETR4'), {'2026-01': 33.0});
      expect(await storedCloses('VALE3'), {'2026-01': 21.0});
    });
  });

  group('calcular', () {
    test('B11: compra de PETR4 em jan com fechamento a 32 dá o mesmo retorno da função pura', () async {
      final petr = await addAsset('PETR4', AssetClass.acaoBr);
      await buy(petr, DateTime(2026, 1, 10), 10, 300);
      histories = {'PETR4.SA': const MonthlyPriceHistory(closes: {'2026-01': 32.0}, splits: [])};
      await repo().refreshPriceHistory();

      final result = await repo().compute();

      expect(result.months, hasLength(1));
      expect(result.months.single.emv, 320);
      expect(result.months.single.rMonthPct, closeTo(20 / (300 * 21 / 31) * 100, 1e-9));
    });

    test('B12: carteira só em reais não pede câmbio; ação em dólar pede desde a 1ª transação', () async {
      final petr = await addAsset('PETR4', AssetClass.acaoBr);
      await buy(petr, DateTime(2026, 1, 10), 10, 300);
      histories = {'PETR4.SA': const MonthlyPriceHistory(closes: {'2026-01': 32.0}, splits: [])};
      await repo().refreshPriceHistory();
      await repo().compute();
      expect(fxCalls, isEmpty);

      final aapl = await addAsset('AAPL', AssetClass.acaoInternacional, currency: 'USD');
      await buy(aapl, DateTime(2026, 1, 12), 10, 1000);
      histories['AAPL'] = const MonthlyPriceHistory(closes: {'2026-01': 110.0}, splits: []);
      await repo().refreshPriceHistory();
      await repo().compute();

      expect(fxCalls, hasLength(1));
      expect(fxCalls.single.$1, 'USD');
      expect(fxCalls.single.$2.year * 10000 + fxCalls.single.$2.month * 100 + fxCalls.single.$2.day, 20260112);
    });

    test('B12: câmbio que o Yahoo não entrega vira erro nomeado de câmbio faltando', () async {
      final aapl = await addAsset('AAPL', AssetClass.acaoInternacional, currency: 'USD');
      await buy(aapl, DateTime(2026, 1, 12), 10, 1000);
      histories = {'AAPL': const MonthlyPriceHistory(closes: {'2026-01': 110.0}, splits: [])};
      await repo().refreshPriceHistory();

      final offline = ProfitabilityRepository(
        portfolio: portfolio,
        now: () => DateTime(2026, 1, 31),
        fetchHistory: (_) async => const MonthlyPriceHistory(closes: {}, splits: []),
        fetchFx: (currency, _) async => throw QuoteUnavailableException('yahoo'),
      );

      await expectLater(
        offline.compute(),
        throwsA(
          isA<ProfitabilityError>()
              .having((e) => e.code, 'code', ProfitabilityErrorCode.missingFx)
              .having((e) => e.args, 'args', {'currency': 'USD'}),
        ),
      );
    });

    test('B13: sem nenhum preço gravado é recusado com erro nomeado, sem número parcial', () async {
      final petr = await addAsset('PETR4', AssetClass.acaoBr);
      await buy(petr, DateTime(2026, 1, 10), 10, 300);

      await expectLater(
        repo().compute(),
        throwsA(
          isA<ProfitabilityError>()
              .having((e) => e.code, 'code', ProfitabilityErrorCode.missingMonthPrice)
              .having((e) => e.args, 'args', {'ticker': 'PETR4', 'month': '2026-01'}),
        ),
      );
    });

    test('B14: split gravado depois da 1ª compra deixa o ativo em "excluídos"', () async {
      final petr = await addAsset('PETR4', AssetClass.acaoBr);
      await buy(petr, DateTime(2026, 1, 10), 10, 300);
      histories = {
        'PETR4.SA': MonthlyPriceHistory(closes: const {'2026-01': 32.0}, splits: [DateTime.utc(2026, 1, 20)]),
      };
      await repo().refreshPriceHistory();

      final result = await repo().compute();

      expect(result.excluded.map((e) => e.asset.ticker), ['PETR4']);
      expect(result.months, isEmpty);
      expect(await count('price_split_event', where: "ticker = 'PETR4'"), 1);
    });
  });
}
