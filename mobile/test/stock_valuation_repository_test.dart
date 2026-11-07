import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/domain/stock_valuation.dart';
import 'package:anchor_mobile/services/stock_valuation_repository.dart';

// Cálculos de valuation salvos (tabela `stock_valuations`, schema v22). Dados fictícios.
void main() {
  var clock = DateTime(2026, 11, 7, 10);
  final repository = StockValuationRepository(now: () => clock);
  late Directory dir;

  const bazin = {'average_dividend': 3.0, 'desired_yield': 0.06};
  const graham = {'eps': 2.0, 'book_value_per_share': 20.0};

  setUp(() async {
    clock = DateTime(2026, 11, 7, 10);
    dir = await Directory.systemTemp.createTemp('anchor-stock-valuation-test-');
    await databaseFactory.setDatabasesPath(dir.path);
    await AppDatabase.resetForTest();
  });

  tearDown(() async {
    await AppDatabase.resetForTest();
    await dir.delete(recursive: true);
  });

  Future<int> count(String table, {String? where}) async {
    final db = await AppDatabase.instance();
    final rows = await db.rawQuery(
      'SELECT COUNT(*) AS n FROM $table${where == null ? '' : ' WHERE $where'}',
    );
    return rows.single['n']! as int;
  }

  group('banco e migração', () {
    test('banco novo nasce com a tabela stock_valuations vazia', () async {
      expect(await count('stock_valuations'), 0);
      expect(await repository.list(), isEmpty);
    });

    test('banco v21 migra sem perder linha e ganha a tabela', () async {
      final old = await openDatabase(
        '${dir.path}/anchor_mobile.db',
        version: 21,
        onCreate: (db, _) async {
          await db.execute('''
            CREATE TABLE assets (
              id INTEGER PRIMARY KEY AUTOINCREMENT, ticker TEXT NOT NULL,
              name TEXT NOT NULL, asset_class TEXT NOT NULL,
              currency TEXT NOT NULL, created_at TEXT NOT NULL)''');
          await db.execute('''
            CREATE TABLE bank_accounts (
              id INTEGER PRIMARY KEY AUTOINCREMENT, nome TEXT NOT NULL,
              titular TEXT NOT NULL, currency TEXT NOT NULL,
              created_at TEXT NOT NULL)''');
          await db.insert('assets', {
            'ticker': 'PETR4',
            'name': 'Petrobras',
            'asset_class': 'acao',
            'currency': 'BRL',
            'created_at': '2026-01-01T00:00:00.000',
          });
          await db.insert('bank_accounts', {
            'nome': 'Nubank',
            'titular': 'Eu',
            'currency': 'BRL',
            'created_at': '2026-01-01T00:00:00.000',
          });
        },
      );
      await old.close();

      expect(await repository.list(), isEmpty);
      expect(await count('stock_valuations'), 0);
      expect(await count('assets', where: "ticker = 'PETR4'"), 1);
      expect(await count('bank_accounts', where: "nome = 'Nubank'"), 1);
    });

    test('reabrir o banco não apaga os cálculos salvos', () async {
      await repository.save(
        ticker: 'PETR4',
        model: ValuationModel.bazin,
        currentPrice: 40,
        inputs: bazin,
      );
      await AppDatabase.resetForTest();

      expect(await repository.list(), hasLength(1));
    });
  });

  group('salvar', () {
    test('recalcula a partir das premissas e guarda ticker em maiúsculas e o ano do relógio', () async {
      final saved = await repository.save(
        ticker: ' petr4 ',
        model: ValuationModel.bazin,
        currentPrice: 40,
        inputs: bazin,
      );

      expect(saved.ticker, 'PETR4');
      expect(saved.model, ValuationModel.bazin);
      expect(saved.referenceYear, 2026);
      expect(saved.currentPrice, 40);
      expect(saved.fairPrice, closeTo(50, 1e-9));
      expect(saved.safetyMargin, closeTo(0.2, 1e-9));
      expect(saved.verdict, ValuationVerdict.cheap);
      expect(saved.createdAt, clock);
      expect(saved.inputs, bazin);
    });

    test('guarda só as premissas do modelo, descartando chaves a mais', () async {
      final saved = await repository.save(
        ticker: 'PETR4',
        model: ValuationModel.bazin,
        currentPrice: 40,
        inputs: {...bazin, 'eps': 9.0, 'qualquer_coisa': 1.0},
      );

      expect(saved.inputs.keys.toSet(), {'average_dividend', 'desired_yield'});
      expect((await repository.list()).single.inputs.keys.toSet(), {
        'average_dividend',
        'desired_yield',
      });
    });

    test('as premissas voltam do banco com o mesmo valor', () async {
      await repository.save(
        ticker: 'PETR4',
        model: ValuationModel.gordon,
        currentPrice: 25,
        inputs: {'current_dividend': 2.0, 'expected_growth': 0.03, 'ke': 0.1},
      );

      expect((await repository.list()).single.inputs, {
        'current_dividend': 2.0,
        'expected_growth': 0.03,
        'ke': 0.1,
      });
    });

    test('premissa inválida é recusada e nada é gravado', () async {
      await expectLater(
        repository.save(
          ticker: 'PETR4',
          model: ValuationModel.bazin,
          currentPrice: 40,
          inputs: {...bazin, 'desired_yield': 0},
        ),
        throwsA(isA<ValuationException>()),
      );
      await expectLater(
        repository.save(
          ticker: 'PETR4',
          model: ValuationModel.bazin,
          currentPrice: 40,
          inputs: {'average_dividend': -3.0, 'desired_yield': 0.06},
        ),
        throwsA(isA<ValuationException>()),
      );
      expect(await count('stock_valuations'), 0);
    });

    test('ticker vazio é recusado', () async {
      await expectLater(
        repository.save(
          ticker: '   ',
          model: ValuationModel.bazin,
          currentPrice: 40,
          inputs: bazin,
        ),
        throwsArgumentError,
      );
      expect(await count('stock_valuations'), 0);
    });

    test('salvar duas vezes cria duas linhas (cada cálculo é novo, sem sobrescrever)', () async {
      for (final price in [40.0, 60.0]) {
        await repository.save(
          ticker: 'PETR4',
          model: ValuationModel.bazin,
          currentPrice: price,
          inputs: bazin,
        );
      }

      expect(await count('stock_valuations'), 2);
    });
  });

  group('listar e excluir', () {
    Future<void> saveAt(String ticker, ValuationModel model, Map<String, double> inputs, DateTime at) {
      clock = at;
      return repository.save(ticker: ticker, model: model, currentPrice: 20, inputs: inputs);
    }

    test('lista do mais novo para o mais antigo', () async {
      await saveAt('PETR4', ValuationModel.bazin, bazin, DateTime(2026, 11, 1));
      await saveAt('PETR4', ValuationModel.graham, graham, DateTime(2026, 11, 3));
      await saveAt('PETR4', ValuationModel.bazin, bazin, DateTime(2026, 11, 2));

      final models = (await repository.list()).map((v) => (v.createdAt.day, v.model));
      expect(models, [
        (3, ValuationModel.graham),
        (2, ValuationModel.bazin),
        (1, ValuationModel.bazin),
      ]);
    });

    test('filtra por ticker, sem diferenciar maiúscula', () async {
      await saveAt('PETR4', ValuationModel.bazin, bazin, DateTime(2026, 11, 1));
      await saveAt('VALE3', ValuationModel.graham, graham, DateTime(2026, 11, 2));

      expect((await repository.list(ticker: 'vale3')).map((v) => v.ticker), ['VALE3']);
      expect(await repository.list(ticker: 'ITUB4'), isEmpty);
      expect(await repository.list(), hasLength(2));
    });

    test('excluir tira só a linha pedida', () async {
      await saveAt('PETR4', ValuationModel.bazin, bazin, DateTime(2026, 11, 1));
      await saveAt('PETR4', ValuationModel.graham, graham, DateTime(2026, 11, 2));
      final first = (await repository.list()).last;

      await repository.delete(first.id);

      final left = await repository.list();
      expect(left, hasLength(1));
      expect(left.single.model, ValuationModel.graham);
    });

    test('linha com modelo desconhecido é ignorada na lista, sem derrubar o resto', () async {
      await saveAt('PETR4', ValuationModel.bazin, bazin, DateTime(2026, 11, 1));
      final db = await AppDatabase.instance();
      await db.insert('stock_valuations', {
        'ticker': 'PETR4',
        'model': 'modelo_do_futuro',
        'reference_year': 2026,
        'current_price': 20.0,
        'fair_price': 30.0,
        'safety_margin': 0.3,
        'verdict': 'cheap',
        'inputs_json': '{}',
        'created_at': '2026-11-02T00:00:00.000',
      });

      expect(await repository.list(), hasLength(1));
    });
  });
}
