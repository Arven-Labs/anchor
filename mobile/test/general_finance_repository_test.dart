import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/models/bank_account.dart';
import 'package:anchor_mobile/models/general_transaction.dart';
import 'package:anchor_mobile/models/general_transaction_category.dart';
import 'package:anchor_mobile/services/general_finance_repository.dart';

void main() {
  final repository = GeneralFinanceRepository();
  late Directory dir;

  setUp(() async {
    dir = await Directory.systemTemp.createTemp('anchor-finance-test-');
    await databaseFactory.setDatabasesPath(dir.path);
    await AppDatabase.resetForTest();
  });

  tearDown(() async {
    await AppDatabase.resetForTest();
    await dir.delete(recursive: true);
  });

  BankAccount account(String nome, String currency) => BankAccount(
    nome: nome,
    titular: 'Eu',
    currency: currency,
    createdAt: DateTime(2026, 1, 1),
  );

  GeneralTransaction tx(
    int accountId,
    GeneralTransactionType type,
    double valor,
    String date,
  ) => GeneralTransaction(
    bankAccountId: accountId,
    type: type,
    valor: valor,
    date: date,
    createdAt: DateTime(2026, 1, 1),
  );

  test(
    'conta nova grava a moeda normalizada e a lista volta por nome',
    () async {
      await repository.insertAccount(account('Nubank', 'BRL'));
      await repository.insertAccount(account('Chase', ' usd '));

      final accounts = await repository.listAccounts();

      expect(accounts.map((a) => a.nome), ['Chase', 'Nubank']);
      expect(accounts.first.currency, 'USD');
      expect(accounts.first.label, 'Chase (USD)');
      expect(accounts.last.label, 'Nubank');
    },
  );

  test('moeda fora do catálogo é recusada', () async {
    expect(
      repository.insertAccount(account('Banco', 'XYZ')),
      throwsArgumentError,
    );
  });

  test(
    'lançamentos voltam do mais novo para o mais antigo, por conta',
    () async {
      final a = await repository.insertAccount(account('Chase', 'USD'));
      final b = await repository.insertAccount(account('Nubank', 'BRL'));
      await repository.insertTransaction(
        tx(a, GeneralTransactionType.receita, 100, '2026-01-02'),
      );
      await repository.insertTransaction(
        tx(a, GeneralTransactionType.despesa, 30, '2026-02-01'),
      );
      await repository.insertTransaction(
        tx(b, GeneralTransactionType.receita, 7, '2026-01-10'),
      );

      final ofA = await repository.listTransactions(accountId: a);
      final all = await repository.listTransactions();

      expect(ofA.map((t) => t.date), ['2026-02-01', '2026-01-02']);
      expect(ofA.first.type, GeneralTransactionType.despesa);
      expect(all, hasLength(3));
    },
  );

  test('valor zero ou negativo e data ilegível são recusados', () async {
    final a = await repository.insertAccount(account('Chase', 'USD'));

    expect(
      repository.insertTransaction(
        tx(a, GeneralTransactionType.receita, 0, '2026-01-02'),
      ),
      throwsArgumentError,
    );
    expect(
      repository.insertTransaction(
        tx(a, GeneralTransactionType.receita, -5, '2026-01-02'),
      ),
      throwsArgumentError,
    );
    expect(
      repository.insertTransaction(
        tx(a, GeneralTransactionType.receita, 5, '02/01/2026'),
      ),
      throwsArgumentError,
    );
  });

  test('a moeda só muda enquanto a conta não tem lançamento', () async {
    final id = await repository.insertAccount(account('Chase', 'USD'));
    BankAccount edited(String currency) => BankAccount(
      id: id,
      nome: 'Chase',
      titular: 'Eu',
      currency: currency,
      createdAt: DateTime(2026, 1, 1),
    );

    await repository.updateAccount(edited('EUR'));
    expect((await repository.listAccounts()).single.currency, 'EUR');

    await repository.insertTransaction(
      tx(id, GeneralTransactionType.receita, 10, '2026-01-02'),
    );
    expect(repository.updateAccount(edited('GBP')), throwsStateError);
    // Nome e titular mudam mesmo com histórico, desde que a moeda fique igual.
    await repository.updateAccount(
      BankAccount(
        id: id,
        nome: 'Chase novo',
        titular: 'Eu',
        currency: 'EUR',
        createdAt: DateTime(2026, 1, 1),
      ),
    );
    expect((await repository.listAccounts()).single.nome, 'Chase novo');
  });

  test(
    'excluir a conta leva os lançamentos dela e deixa os das outras',
    () async {
      final a = await repository.insertAccount(account('Chase', 'USD'));
      final b = await repository.insertAccount(account('Nubank', 'BRL'));
      await repository.insertTransaction(
        tx(a, GeneralTransactionType.receita, 100, '2026-01-02'),
      );
      await repository.insertTransaction(
        tx(b, GeneralTransactionType.receita, 7, '2026-01-10'),
      );

      await repository.deleteAccount(a);

      expect((await repository.listAccounts()).single.nome, 'Nubank');
      final left = await repository.listTransactions();
      expect(left.single.bankAccountId, b);
    },
  );

  test('excluir um lançamento tira só ele', () async {
    final a = await repository.insertAccount(account('Chase', 'USD'));
    final first = await repository.insertTransaction(
      tx(a, GeneralTransactionType.receita, 100, '2026-01-02'),
    );
    await repository.insertTransaction(
      tx(a, GeneralTransactionType.despesa, 30, '2026-01-03'),
    );

    await repository.deleteTransaction(first);

    final left = await repository.listTransactions(accountId: a);
    expect(left.single.type, GeneralTransactionType.despesa);
  });

  test(
    'a moeda base é o real até alguém escolher, e a escolha persiste',
    () async {
      expect(await repository.getBaseCurrency(), 'BRL');

      await repository.setBaseCurrency(' usd ');
      expect(await repository.getBaseCurrency(), 'USD');

      await repository.setBaseCurrency('EUR');
      expect(await repository.getBaseCurrency(), 'EUR');
      expect(repository.setBaseCurrency('XYZ'), throwsArgumentError);
    },
  );

  test(
    'migra do v15: cria as tabelas novas e preserva o que já existia',
    () async {
      final old = await openDatabase(
        '${dir.path}/anchor_mobile.db',
        version: 15,
        onCreate: (db, _) async {
          await db.execute('''
          CREATE TABLE custodias (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            instituicao TEXT NOT NULL,
            titular TEXT NOT NULL,
            created_at TEXT NOT NULL)''');
          await db.insert('custodias', {
            'instituicao': 'XP',
            'titular': 'Eu',
            'created_at': '2026-01-01T00:00:00.000',
          });
        },
      );
      await old.close();

      final id = await repository.insertAccount(account('Chase', 'USD'));
      await repository.insertTransaction(
        tx(id, GeneralTransactionType.receita, 10, '2026-01-02'),
      );

      expect(await repository.listTransactions(), hasLength(1));
      final db = await AppDatabase.instance();
      final custodias = await db.query('custodias');
      expect(custodias.single['instituicao'], 'XP');
    },
  );

  test('categoria: cria, edita, lista por nome e valida nome e limite', () async {
    final mercado = await repository.insertCategory(
      GeneralTransactionCategory(
        nome: ' Mercado ',
        limiteMensal: 800,
        createdAt: DateTime(2026, 1, 1),
      ),
    );
    await repository.insertCategory(
      GeneralTransactionCategory(
        nome: 'casa',
        createdAt: DateTime(2026, 1, 1),
      ),
    );

    var list = await repository.listCategories();
    expect(list.map((c) => c.nome), ['casa', 'Mercado']);
    expect(list.last.limiteMensal, 800);
    expect(list.first.limiteMensal, isNull);

    await repository.updateCategory(
      GeneralTransactionCategory(
        id: mercado,
        nome: 'Supermercado',
        createdAt: DateTime(2030, 1, 1),
      ),
    );
    list = await repository.listCategories();
    final updated = list.firstWhere((c) => c.id == mercado);
    expect(updated.nome, 'Supermercado');
    expect(updated.limiteMensal, isNull);
    expect(updated.createdAt, DateTime(2026, 1, 1)); // não regrava a criação

    for (final bad in [
      GeneralTransactionCategory(nome: '  ', createdAt: DateTime(2026, 1, 1)),
      GeneralTransactionCategory(
        nome: 'X',
        limiteMensal: 0,
        createdAt: DateTime(2026, 1, 1),
      ),
      GeneralTransactionCategory(
        nome: 'X',
        limiteMensal: double.nan,
        createdAt: DateTime(2026, 1, 1),
      ),
    ]) {
      expect(() => repository.insertCategory(bad), throwsArgumentError);
    }
  });

  test('categoria só vale para despesa; apagar solta os lançamentos', () async {
    final account = await repository.insertAccount(
      BankAccount(
        nome: 'Nubank',
        titular: 'Eu',
        currency: 'BRL',
        createdAt: DateTime(2026, 1, 1),
      ),
    );
    final category = await repository.insertCategory(
      GeneralTransactionCategory(nome: 'Mercado', createdAt: DateTime(2026, 1, 1)),
    );
    GeneralTransaction withCategory(GeneralTransactionType type) =>
        GeneralTransaction(
          bankAccountId: account,
          type: type,
          categoryId: category,
          valor: 10,
          date: '2026-08-05',
          createdAt: DateTime(2026, 1, 1),
        );
    await repository.insertTransaction(
      withCategory(GeneralTransactionType.despesa),
    );
    await repository.insertTransaction(
      withCategory(GeneralTransactionType.receita),
    );

    var txs = await repository.listTransactions();
    expect(
      txs.where((t) => t.type == GeneralTransactionType.despesa).single.categoryId,
      category,
    );
    expect(
      txs.where((t) => t.type == GeneralTransactionType.receita).single.categoryId,
      isNull,
    );

    await repository.deleteCategory(category);

    expect(await repository.listCategories(), isEmpty);
    txs = await repository.listTransactions();
    expect(txs, hasLength(2));
    expect(txs.every((t) => t.categoryId == null), isTrue);
  });

  test('banco v16 sobe para a v17 sem perder lançamentos', () async {
    final old = await openDatabase(
      '${dir.path}/anchor_mobile.db',
      version: 16,
      onCreate: (db, _) async {
        await db.execute('''CREATE TABLE bank_accounts (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          nome TEXT NOT NULL, titular TEXT NOT NULL,
          currency TEXT NOT NULL, created_at TEXT NOT NULL)''');
        await db.execute('''CREATE TABLE general_transactions (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          bank_account_id INTEGER NOT NULL,
          transaction_type TEXT NOT NULL, valor REAL NOT NULL,
          transaction_date TEXT NOT NULL, notes TEXT,
          created_at TEXT NOT NULL)''');
        await db.execute('''CREATE TABLE app_settings (
          key TEXT PRIMARY KEY, value TEXT NOT NULL)''');
        await db.insert('bank_accounts', {
          'nome': 'Velha',
          'titular': 'Eu',
          'currency': 'BRL',
          'created_at': '2026-01-01T00:00:00.000',
        });
        await db.insert('general_transactions', {
          'bank_account_id': 1,
          'transaction_type': 'despesa',
          'valor': 7.5,
          'transaction_date': '2026-08-05',
          'created_at': '2026-01-01T00:00:00.000',
        });
      },
    );
    await old.close();

    final txs = await repository.listTransactions();
    expect(txs.single.valor, 7.5);
    expect(txs.single.categoryId, isNull);
    final category = await repository.insertCategory(
      GeneralTransactionCategory(nome: 'Mercado', createdAt: DateTime(2026, 1, 1)),
    );
    expect(await repository.listCategories(), hasLength(1));
    expect(category, isPositive);
  });
}
