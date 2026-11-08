import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/domain/finance_error.dart';
import 'package:anchor_mobile/domain/general_finance.dart';
import 'package:anchor_mobile/domain/porquinho_yield.dart';
import 'package:anchor_mobile/models/bank_account.dart';
import 'package:anchor_mobile/models/general_transaction.dart';
import 'package:anchor_mobile/services/general_finance_repository.dart';

// Persistência da estimativa do Porquinho: que fluxos entram no modelo, que dias já contam
// como "com rendimento" e o que a aprovação grava. Porta de `account_legs`, `days_with_yield`
// (`commands/porquinho_estimate.rs`) e de `confirm_yield_pending_in` (`commands/fixed_income_io.rs`).
Matcher financeError(FinanceErrorCode code, [Map<String, String> args = const {}]) => throwsA(
  anyOf(
    isA<FinanceArgumentError>().having((e) => e.code, 'code', code).having((e) => e.args, 'args', args),
    isA<FinanceStateError>().having((e) => e.code, 'code', code).having((e) => e.args, 'args', args),
  ),
);

void main() {
  final repository = GeneralFinanceRepository();
  late Directory dir;
  late int porquinho;
  late int corrente;

  DateTime d(int y, int m, int day) => DateTime.utc(y, m, day);

  setUp(() async {
    dir = await Directory.systemTemp.createTemp('anchor-porquinho-test-');
    await databaseFactory.setDatabasesPath(dir.path);
    await AppDatabase.resetForTest();
    BankAccount account(String nome) =>
        BankAccount(nome: nome, titular: 'Eu', currency: 'BRL', createdAt: DateTime(2026, 1, 1));
    porquinho = await repository.insertAccount(account('Porquinho'));
    corrente = await repository.insertAccount(account('Corrente'));
  });

  tearDown(() async {
    await AppDatabase.resetForTest();
    await dir.delete(recursive: true);
  });

  /// Aplicação: dinheiro que entra no Porquinho.
  Future<void> apply(double valor, String date) => repository.createTransfer(
    fromAccountId: corrente,
    toAccountId: porquinho,
    valorSaida: valor,
    valorEntrada: valor,
    date: date,
  );

  /// Resgate: o que o banco pagou, saindo do Porquinho.
  Future<void> redeem(double valor, String date) => repository.createTransfer(
    fromAccountId: porquinho,
    toAccountId: corrente,
    valorSaida: valor,
    valorEntrada: valor,
    date: date,
  );

  Future<List<GeneralTransaction>> ledger() => repository.listTransactions(accountId: porquinho);

  Future<double> balance() async => computeBalances(await ledger(), '2099-01-01')[porquinho] ?? 0;

  group('fluxos da conta (porquinhoLegs)', () {
    test('só as pernas de transferência da conta, aplicação como entrada e resgate como saída', () async {
      await apply(1000, '2025-01-02');
      await redeem(500, '2025-02-03');
      await repository.insertTransaction(
        GeneralTransaction(
          bankAccountId: porquinho,
          type: GeneralTransactionType.receita,
          valor: 9,
          date: '2025-02-04',
          createdAt: DateTime(2026, 1, 1),
        ),
      );

      final legs = await repository.porquinhoLegs(porquinho);

      expect(legs.map((l) => (l.date, l.kind, l.amount)), [
        (d(2025, 1, 2), LegKind.application, 1000.0),
        (d(2025, 2, 3), LegKind.redemption, 500.0),
      ]);
    });

    test('no mesmo dia a aplicação vem antes do resgate, mesmo gravada depois', () async {
      await redeem(50, '2025-06-02');
      await apply(50, '2025-06-02');

      final legs = await repository.porquinhoLegs(porquinho);

      expect(legs.map((l) => l.kind), [LegKind.application, LegKind.redemption]);
    });

    test('fluxos de outra conta não entram', () async {
      await apply(1000, '2025-01-02');

      expect(await repository.porquinhoLegs(corrente), [
        isA<Leg>().having((l) => l.kind, 'kind', LegKind.redemption).having((l) => l.amount, 'amount', 1000.0),
      ]);
    });
  });

  group('dias que já têm rendimento (daysWithYield)', () {
    Future<void> income(int account, String date, {String? notes}) => repository.insertTransaction(
      GeneralTransaction(
        bankAccountId: account,
        type: GeneralTransactionType.receita,
        valor: 5,
        date: date,
        notes: notes,
        createdAt: DateTime(2026, 1, 1),
      ),
    );

    test('conta o dia de cada receita da conta', () async {
      await income(porquinho, '2025-03-01');
      await income(porquinho, '2025-03-05', notes: 'Yield CDB Porquinho (gross)');

      expect(await repository.daysWithYield(porquinho), {'2025-03-01', '2025-03-05'});
    });

    test('receita da conciliação e do ajuste de saldo não cobre o dia', () async {
      await income(porquinho, '2025-03-01', notes: 'Yield by balance reconciliation: bank balance 1.00');
      await income(porquinho, '2025-03-02', notes: 'Balance adjustment by reconciliation: bank balance 1.00');

      expect(await repository.daysWithYield(porquinho), isEmpty);
    });

    test('despesa e receita de outra conta não contam', () async {
      await repository.insertTransaction(
        GeneralTransaction(
          bankAccountId: porquinho,
          type: GeneralTransactionType.despesa,
          valor: 5,
          date: '2025-03-01',
          createdAt: DateTime(2026, 1, 1),
        ),
      );
      await income(corrente, '2025-03-02');

      expect(await repository.daysWithYield(porquinho), isEmpty);
    });
  });

  group('lançar a estimativa (bookYieldEstimates)', () {
    RedemptionEstimate estimate({
      double gross = 10,
      double iof = 2,
      double ir = 1.8,
      DateTime? date,
    }) => RedemptionEstimate(
      legIndex: 1,
      date: date ?? d(2025, 2, 3),
      paid: 1000,
      principal: 993.8,
      grossYield: gross,
      iof: iof,
      ir: ir,
      netYield: gross - iof - ir,
      holdingDays: 32,
      fullyCovered: true,
    );

    test('grava receita bruta em Yield, IOF e IR em Taxes, e o saldo sobe pelo líquido', () async {
      await apply(1000, '2025-01-02');
      final before = await balance();

      final rows = await repository.bookYieldEstimates(accountId: porquinho, estimates: [estimate()]);

      final categories = {for (final c in await repository.listCategories()) c.id!: c.nome};
      final booked = (await ledger()).where((t) => t.date == '2025-02-03').toList();
      expect(rows, 3);
      expect(booked, hasLength(3));
      final gross = booked.singleWhere((t) => t.type == GeneralTransactionType.receita);
      expect(gross.valor, 10);
      expect(categories[gross.categoryId], 'Yield');
      expect(gross.notes, 'Yield CDB Porquinho (gross)');
      final expenses = booked.where((t) => t.type == GeneralTransactionType.despesa).toList();
      expect(expenses.map((t) => categories[t.categoryId]).toSet(), {'Taxes'});
      expect(expenses.map((t) => t.notes), containsAll(['IOF withheld on CDB Porquinho yield', 'IR withheld on CDB Porquinho yield (22.5%)']));
      expect(await balance() - before, closeTo(6.2, 1e-9));
    });

    test('sem IOF (acima de 30 dias) não grava a despesa de IOF', () async {
      final rows = await repository.bookYieldEstimates(
        accountId: porquinho,
        estimates: [estimate(iof: 0, ir: 2.25)],
      );

      expect(rows, 2);
      expect((await ledger()).map((t) => t.notes), isNot(contains('IOF withheld on CDB Porquinho yield')));
    });

    test('reaproveita as categorias Yield e Taxes entre lançamentos', () async {
      await repository.bookYieldEstimates(accountId: porquinho, estimates: [estimate()]);
      await repository.bookYieldEstimates(accountId: porquinho, estimates: [estimate(date: d(2025, 3, 3))]);

      expect((await repository.listCategories()).map((c) => c.nome).toList()..sort(), ['Taxes', 'Yield']);
    });

    test('depois de lançado, o dia passa a contar como coberto (estimar de novo não duplica)', () async {
      await repository.bookYieldEstimates(accountId: porquinho, estimates: [estimate()]);

      expect(await repository.daysWithYield(porquinho), {'2025-02-03'});
    });

    test('tudo ou nada: uma estimativa inválida na lista não deixa as outras gravadas', () async {
      await expectLater(
        repository.bookYieldEstimates(
          accountId: porquinho,
          estimates: [estimate(), estimate(gross: 0, iof: 0, ir: 0, date: d(2025, 3, 3))],
        ),
        financeError(FinanceErrorCode.amountMustBePositive),
      );

      expect(await ledger(), isEmpty);
      expect(await repository.listCategories(), isEmpty);
    });

    test('conta que não existe: accountNotFound, nada gravado', () async {
      await expectLater(
        repository.bookYieldEstimates(accountId: 999, estimates: [estimate()]),
        financeError(FinanceErrorCode.accountNotFound),
      );

      expect(await repository.listCategories(), isEmpty);
    });
  });
}
