import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/models/bank_account.dart';
import 'package:anchor_mobile/models/general_transaction.dart';
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
}
