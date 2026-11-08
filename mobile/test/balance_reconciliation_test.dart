import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/domain/finance_error.dart';
import 'package:anchor_mobile/domain/general_finance.dart';
import 'package:anchor_mobile/models/bank_account.dart';
import 'package:anchor_mobile/models/general_transaction.dart';
import 'package:anchor_mobile/services/general_finance_repository.dart';

// Conciliação de saldo: o usuário digita o saldo que o banco mostra numa data e a diferença contra
// o saldo calculado dos lançamentos vira um lançamento. Porta de
// `desktop/src-tauri/src/commands/balance_reconciliation.rs`.

Matcher financeError(FinanceErrorCode code, [Map<String, String> args = const {}]) => throwsA(
  anyOf(
    isA<FinanceArgumentError>().having((e) => e.code, 'code', code).having((e) => e.args, 'args', args),
    isA<FinanceStateError>().having((e) => e.code, 'code', code).having((e) => e.args, 'args', args),
  ),
);

GeneralTransaction tx(int account, GeneralTransactionType type, double valor, String date) => GeneralTransaction(
  bankAccountId: account,
  type: type,
  valor: valor,
  date: date,
  createdAt: DateTime(2026, 1, 1),
);

void main() {
  const receita = GeneralTransactionType.receita;
  const despesa = GeneralTransactionType.despesa;

  group('prévia (domínio puro)', () {
    ReconciliationPreview preview(
      List<GeneralTransaction> transactions, {
      double bank = 112.5,
      String date = '2026-11-01',
      String today = '2026-11-08',
    }) => reconciliationPreview(
      transactions: transactions,
      accountId: 1,
      bankBalance: bank,
      date: date,
      today: today,
    );

    test('banco 112,50 contra saldo 100,00: diferença de 12,50', () {
      final result = preview([tx(1, receita, 100, '2026-10-01')]);

      expect(result.ledgerBalance, 100);
      expect(result.bankBalance, 112.5);
      expect(result.difference, 12.5);
      expect(result.date, '2026-11-01');
    });

    test('lançamento depois da data não entra no saldo daquele dia', () {
      final result = preview([tx(1, receita, 100, '2026-10-01'), tx(1, receita, 50, '2026-11-05')]);

      expect(result.ledgerBalance, 100);
    });

    test('lançamento de outra conta não entra', () {
      final result = preview([tx(1, receita, 100, '2026-10-01'), tx(2, receita, 999, '2026-10-01')]);

      expect(result.ledgerBalance, 100);
    });

    test('ruído de ponto flutuante vira centavos: 0,1 + 0,2 contra 0,30 dá diferença zero', () {
      final result = preview([tx(1, receita, 0.1, '2026-10-01'), tx(1, receita, 0.2, '2026-10-02')], bank: 0.3);

      expect(result.difference, 0);
    });

    test('saldo do banco negativo, NaN ou infinito é recusado', () {
      for (final bad in [-0.01, double.nan, double.infinity]) {
        expect(() => preview([], bank: bad), throwsA(isA<FinanceArgumentError>()));
      }
      expect(() => preview([], bank: -1), throwsA(isA<FinanceArgumentError>().having((e) => e.code, 'code', FinanceErrorCode.balanceMustBeNonNegative)));
    });

    test('data de amanhã é recusada; data de hoje vale', () {
      expect(
        () => preview([], date: '2026-11-09'),
        throwsA(isA<FinanceArgumentError>().having((e) => e.code, 'code', FinanceErrorCode.dateInFuture)),
      );
      expect(preview([], date: '2026-11-08').date, '2026-11-08');
    });

    test('data que não é YYYY-MM-DD é recusada', () {
      for (final bad in ['2026-13-01', '2026-02-30', '01/11/2026', '']) {
        expect(
          () => preview([], date: bad),
          throwsA(isA<FinanceArgumentError>().having((e) => e.code, 'code', FinanceErrorCode.invalidDate)),
        );
      }
    });
  });

  group('conciliar no banco', () {
    final repository = GeneralFinanceRepository();
    final now = DateTime(2026, 11, 8);
    late Directory dir;
    late int accountId;

    setUp(() async {
      dir = await Directory.systemTemp.createTemp('anchor-reconcile-test-');
      await databaseFactory.setDatabasesPath(dir.path);
      await AppDatabase.resetForTest();
      accountId = await repository.insertAccount(
        BankAccount(nome: 'Porquinho', titular: 'Eu', currency: 'BRL', createdAt: DateTime(2026, 1, 1)),
      );
      await repository.insertTransaction(tx(accountId, receita, 100, '2026-10-01'));
    });

    tearDown(() async {
      await AppDatabase.resetForTest();
      await dir.delete(recursive: true);
    });

    Future<int> reconcile(double bank, {bool adjustment = false, String date = '2026-11-01'}) =>
        repository.confirmReconciliation(
          accountId: accountId,
          bankBalance: bank,
          date: date,
          adjustment: adjustment,
          now: now,
        );

    Future<double> balanceOn(String date) async =>
        computeBalances(await repository.listTransactions(accountId: accountId), date)[accountId] ?? 0;

    test('rendimento: banco 112,50 contra 100,00 grava receita de 12,50 na categoria Yield', () async {
      final id = await reconcile(112.5);

      final transactions = await repository.listTransactions(accountId: accountId);
      final booked = transactions.firstWhere((t) => t.id == id);
      final categories = await repository.listCategories();
      expect(booked.type, receita);
      expect(booked.valor, 12.5);
      expect(booked.date, '2026-11-01');
      expect(categories.map((c) => c.nome), ['Yield']);
      expect(booked.categoryId, categories.single.id);
      expect(booked.notes, startsWith('Yield by balance reconciliation'));
      expect(await balanceOn('2026-11-01'), 112.5);
    });

    test('rendimento reaproveita a categoria Yield que já existe', () async {
      await reconcile(112.5);
      await reconcile(120, date: '2026-11-05');

      expect((await repository.listCategories()).map((c) => c.nome), ['Yield']);
    });

    test('conciliar de novo a mesma data e o mesmo saldo: nada a conciliar, nada duplicado', () async {
      await reconcile(112.5);

      await expectLater(reconcile(112.5), financeError(FinanceErrorCode.nothingToReconcile));
      expect(await repository.listTransactions(accountId: accountId), hasLength(2));
    });

    test('rendimento com banco 90,00 contra 100,00 é recusado e não cria a categoria Yield', () async {
      await expectLater(reconcile(90), financeError(FinanceErrorCode.bankBalanceBelowLedger));

      expect(await repository.listTransactions(accountId: accountId), hasLength(1));
      expect(await repository.listCategories(), isEmpty);
    });

    test('ajuste com banco 90,00 contra 100,00 grava despesa de 10,00 sem categoria', () async {
      final id = await reconcile(90, adjustment: true);

      final booked = (await repository.listTransactions(accountId: accountId)).firstWhere((t) => t.id == id);
      expect(booked.type, despesa);
      expect(booked.valor, 10);
      expect(booked.categoryId, isNull);
      expect(booked.notes, startsWith('Balance adjustment by reconciliation'));
      expect(await balanceOn('2026-11-01'), 90);
    });

    test('ajuste com banco 112,50 grava receita de 12,50 sem categoria e sem criar Yield', () async {
      final id = await reconcile(112.5, adjustment: true);

      final booked = (await repository.listTransactions(accountId: accountId)).firstWhere((t) => t.id == id);
      expect(booked.type, receita);
      expect(booked.valor, 12.5);
      expect(booked.categoryId, isNull);
      expect(await repository.listCategories(), isEmpty);
    });

    test('ajuste com diferença zero: nada a conciliar', () async {
      await expectLater(reconcile(100, adjustment: true), financeError(FinanceErrorCode.nothingToReconcile));
      expect(await repository.listTransactions(accountId: accountId), hasLength(1));
    });

    test('conta que não existe: accountNotFound', () async {
      await expectLater(
        repository.confirmReconciliation(accountId: 999, bankBalance: 10, date: '2026-11-01', now: now),
        financeError(FinanceErrorCode.accountNotFound),
      );
    });

    test('a prévia do repositório calcula sem gravar nada', () async {
      final result = await repository.previewReconciliation(
        accountId: accountId,
        bankBalance: 112.5,
        date: '2026-11-01',
        now: now,
      );

      expect(result.ledgerBalance, 100);
      expect(result.difference, 12.5);
      expect(await repository.listTransactions(accountId: accountId), hasLength(1));
      expect(await repository.listCategories(), isEmpty);
    });

    test('data futura é recusada e não grava', () async {
      await expectLater(
        reconcile(112.5, date: '2026-11-09'),
        financeError(FinanceErrorCode.dateInFuture, {'date': '2026-11-09'}),
      );
      expect(await repository.listTransactions(accountId: accountId), hasLength(1));
    });
  });
}
