import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/domain/finance_error.dart';
import 'package:anchor_mobile/models/asset.dart';
import 'package:anchor_mobile/services/portfolio_repository.dart';
import 'package:anchor_mobile/services/thesis_repository.dart';

// Teses salvas (tabela `theses`, schema v25). Tabela de testes TR1 a TR10. Dados fictícios.
Matcher _error(FinanceErrorCode code) => throwsA(
  anyOf(
    isA<FinanceArgumentError>().having((e) => e.code, 'code', code),
    isA<FinanceStateError>().having((e) => e.code, 'code', code),
  ),
);

void main() {
  var clock = DateTime(2026, 11, 8, 10);
  final repository = ThesisRepository(now: () => clock);
  final portfolio = PortfolioRepository();
  late Directory dir;

  setUp(() async {
    clock = DateTime(2026, 11, 8, 10);
    dir = await Directory.systemTemp.createTemp('anchor-thesis-test-');
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
    test('TR1: banco novo nasce com a tabela theses vazia', () async {
      expect(await count('theses'), 0);
      expect(await repository.list(), isEmpty);
    });

    test('TR1: banco v24 migra sem perder ativo nem transação e ganha a tabela', () async {
      final old = await openDatabase(
        '${dir.path}/anchor_mobile.db',
        version: 24,
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

      expect(await repository.list(), isEmpty);
      expect(await count('assets', where: "ticker = 'PETR4'"), 1);
      expect(await count('portfolio_transactions'), 1);
    });
  });

  group('criar', () {
    test('TR2: tese global e tese ligada à PETR4 aparecem; só a ligada traz o ticker', () async {
      final petr = await addAsset('PETR4');
      await repository.create(title: 'Juros 2027');
      clock = DateTime(2026, 11, 8, 11);
      await repository.create(title: 'Petrobras', assetId: petr);

      final entries = await repository.list();

      expect(entries.map((e) => e.thesis.title), ['Petrobras', 'Juros 2027']);
      expect(entries.map((e) => e.asset?.ticker), ['PETR4', null]);
    });

    test('TR3: guarda título sem espaços, texto literal e criada = atualizada = agora', () async {
      await repository.create(title: '  Juros 2027  ', content: '# Cenário\n\n- selic\n');

      final thesis = (await repository.list()).single.thesis;

      expect(thesis.title, 'Juros 2027');
      expect(thesis.contentMarkdown, '# Cenário\n\n- selic\n');
      expect(thesis.createdAt, DateTime(2026, 11, 8, 10));
      expect(thesis.updatedAt, DateTime(2026, 11, 8, 10));
      expect(thesis.assetId, isNull);
    });

    test('TR4: título em branco é recusado e nada é gravado', () async {
      await expectLater(repository.create(title: '   '), _error(FinanceErrorCode.thesisTitleRequired));
      expect(await count('theses'), 0);
    });
  });

  group('editar e apagar', () {
    test('TR5: editar muda título, texto e ativo, atualiza a hora e mantém a criação', () async {
      final petr = await addAsset('PETR4');
      final id = await repository.create(title: 'Juros', content: 'a');

      clock = DateTime(2026, 11, 8, 11);
      await repository.update(id, title: ' Juros 2028 ', content: 'b', assetId: petr);

      final entry = (await repository.list()).single;
      expect(entry.thesis.title, 'Juros 2028');
      expect(entry.thesis.contentMarkdown, 'b');
      expect(entry.asset?.ticker, 'PETR4');
      expect(entry.thesis.createdAt, DateTime(2026, 11, 8, 10));
      expect(entry.thesis.updatedAt, DateTime(2026, 11, 8, 11));
    });

    test('TR6: editar com título em branco é recusado e a tese fica como estava', () async {
      final id = await repository.create(title: 'Juros', content: 'a');

      await expectLater(repository.update(id, title: '  ', content: 'z'), _error(FinanceErrorCode.thesisTitleRequired));

      final thesis = (await repository.list()).single.thesis;
      expect((thesis.title, thesis.contentMarkdown), ('Juros', 'a'));
    });

    test('TR7: editar um id que não existe é erro nomeado', () async {
      await expectLater(repository.update(9999, title: 'x'), _error(FinanceErrorCode.thesisNotFound));
    });

    test('TR8: apagar uma de duas teses deixa só a outra', () async {
      final a = await repository.create(title: 'A');
      await repository.create(title: 'B');

      await repository.delete(a);

      expect((await repository.list()).map((e) => e.thesis.title), ['B']);
    });
  });

  group('lista', () {
    test('TR9: a mais recentemente atualizada vem primeiro', () async {
      final a = await repository.create(title: 'A');
      clock = DateTime(2026, 11, 8, 10, 30);
      await repository.create(title: 'B');
      expect((await repository.list()).map((e) => e.thesis.title), ['B', 'A']);

      clock = DateTime(2026, 11, 8, 12);
      await repository.update(a, title: 'A');

      expect((await repository.list()).map((e) => e.thesis.title), ['A', 'B']);
    });

    test('TR10: ativo apagado do cadastro deixa a tese na lista, sem ativo', () async {
      final petr = await addAsset('PETR4');
      await repository.create(title: 'Petrobras', assetId: petr);

      await portfolio.deleteAsset(petr);

      final entry = (await repository.list()).single;
      expect(entry.thesis.title, 'Petrobras');
      expect(entry.asset, isNull);
    });
  });
}
