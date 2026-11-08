import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/domain/finance_error.dart';
import 'package:anchor_mobile/domain/liability.dart';
import 'package:anchor_mobile/models/bank_account.dart';
import 'package:anchor_mobile/models/general_transaction.dart';
import 'package:anchor_mobile/models/general_transaction_category.dart';
import 'package:anchor_mobile/models/liability.dart';
import 'package:anchor_mobile/models/loan.dart';
import 'package:anchor_mobile/services/general_finance_repository.dart';

// Cada erro do repositório de Finanças tem um código e argumentos (ADR 0001): o texto do
// usuário vem do `.arb` pelo código, a mensagem da exceção é inglês para quem depura. Os
// tipos continuam sendo `ArgumentError`/`StateError`, então os `throwsArgumentError` e
// `throwsStateError` de `general_finance_repository_test.dart` seguem valendo.
Matcher financeError(FinanceErrorCode code, [Map<String, String> args = const {}]) => throwsA(
  anyOf(
    isA<FinanceArgumentError>().having((e) => e.code, 'code', code).having((e) => e.args, 'args', args),
    isA<FinanceStateError>().having((e) => e.code, 'code', code).having((e) => e.args, 'args', args),
  ),
);

void main() {
  final repository = GeneralFinanceRepository();
  late Directory dir;

  setUp(() async {
    dir = await Directory.systemTemp.createTemp('anchor-finance-errors-');
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
    int accountId, {
    GeneralTransactionType type = GeneralTransactionType.receita,
    double valor = 10,
    String date = '2026-01-02',
  }) => GeneralTransaction(
    bankAccountId: accountId,
    type: type,
    valor: valor,
    date: date,
    createdAt: DateTime(2026, 1, 1),
  );

  Liability liability(int accountId, {String nome = 'Carro', String start = '2026-01-31'}) => Liability(
    bankAccountId: accountId,
    nome: nome,
    titular: 'Eu',
    principal: 1200,
    taxaPercentual: 1,
    prazoMeses: 12,
    sistema: AmortizationSystem.sac,
    dataInicio: start,
    createdAt: DateTime(2026, 1, 1),
  );

  Future<int> borrower([String nome = 'Irmão']) =>
      repository.insertBorrower(Borrower(nome: nome, createdAt: DateTime(2026, 1, 1)));

  Loan loan(
    int borrowerId, {
    int? accountId,
    double principal = 1000,
    double? rate,
    String currency = 'BRL',
    String start = '2026-01-10',
  }) => Loan(
    borrowerId: borrowerId,
    bankAccountId: accountId,
    principal: principal,
    taxaMensalPercentual: rate,
    dataInicio: start,
    includeInNetWorth: true,
    currency: currency,
    createdAt: DateTime(2026, 1, 10),
  );

  LoanRepayment repayment(
    int loanId, {
    int? accountId,
    double received = 0,
    double writtenOff = 100,
    String date = '2026-03-10',
  }) => LoanRepayment(
    loanId: loanId,
    bankAccountId: accountId,
    data: date,
    valorRecebido: received,
    valorBaixado: writtenOff,
    createdAt: DateTime(2026, 3, 10),
  );

  group('os códigos e os tipos', () {
    test('há um código por erro do repositório e do domínio (32) e nenhum se repete', () {
      expect(FinanceErrorCode.values, hasLength(32));
      expect(FinanceErrorCode.values.toSet(), hasLength(32));
    });

    test('FinanceArgumentError é um ArgumentError e FinanceStateError é um StateError', () {
      final argument = FinanceArgumentError(FinanceErrorCode.invalidDate, {'date': 'x'});
      final state = FinanceStateError(FinanceErrorCode.accountNotFound);
      expect(argument, isA<ArgumentError>());
      expect(state, isA<StateError>());
      expect(argument.args, {'date': 'x'});
      expect(state.args, isEmpty);
    });

    test('a mensagem é inglês de desenvolvedor: não vazia, sem acento, com os argumentos', () {
      final withArgs = FinanceArgumentError(FinanceErrorCode.unknownCurrency, {'currency': 'XYZ'});
      expect(withArgs.message.toString(), contains('XYZ'));
      for (final code in FinanceErrorCode.values) {
        final error = FinanceArgumentError(code, const {'currency': 'XYZ', 'date': 'x'});
        final message = error.message.toString();
        expect(message, isNotEmpty, reason: '$code');
        expect(message, isNot(matches(RegExp(r'[áéíóúâêôãõçÁÉÍÓÚÂÊÔÃÕÇ]'))), reason: '$code');
      }
    });
  });

  group('ArgumentError', () {
    test('unknownCurrency: conta nova, conta editada, empréstimo e moeda base', () async {
      final id = await repository.insertAccount(account('Nubank', 'BRL'));
      final who = await borrower();

      await expectLater(
        repository.insertAccount(account('Banco', 'XYZ')),
        financeError(FinanceErrorCode.unknownCurrency, {'currency': 'XYZ'}),
      );
      await expectLater(
        repository.updateAccount(
          BankAccount(id: id, nome: 'Nubank', titular: 'Eu', currency: 'XYZ', createdAt: DateTime(2026, 1, 1)),
        ),
        financeError(FinanceErrorCode.unknownCurrency, {'currency': 'XYZ'}),
      );
      await expectLater(
        repository.createLoan(loan(who, currency: 'XXX')),
        financeError(FinanceErrorCode.unknownCurrency, {'currency': 'XXX'}),
      );
      await expectLater(
        repository.setBaseCurrency('XYZ'),
        financeError(FinanceErrorCode.unknownCurrency, {'currency': 'XYZ'}),
      );
    });

    test('amountMustBePositive, invalidDate e manualTypeOnly no lançamento', () async {
      final id = await repository.insertAccount(account('Nubank', 'BRL'));

      await expectLater(
        repository.insertTransaction(tx(id, valor: 0)),
        financeError(FinanceErrorCode.amountMustBePositive),
      );
      await expectLater(
        repository.insertTransaction(tx(id, valor: -5)),
        financeError(FinanceErrorCode.amountMustBePositive),
      );
      await expectLater(
        repository.insertTransaction(tx(id, date: '02/01/2026')),
        financeError(FinanceErrorCode.invalidDate, {'date': '02/01/2026'}),
      );
      await expectLater(
        repository.insertTransaction(tx(id, type: GeneralTransactionType.transferenciaSaida)),
        financeError(FinanceErrorCode.manualTypeOnly),
      );
    });

    test('invalidDate em dívida, empréstimo, baixa e transferência', () async {
      final a = await repository.insertAccount(account('Alfa', 'BRL'));
      final b = await repository.insertAccount(account('Beta', 'BRL'));
      final who = await borrower();
      final loanId = await repository.createLoan(loan(who));

      await expectLater(
        repository.createLiability(liability(a, start: 'ontem')),
        financeError(FinanceErrorCode.invalidDate, {'date': 'ontem'}),
      );
      await expectLater(
        repository.createLoan(loan(who, start: '10/01/2026')),
        financeError(FinanceErrorCode.invalidDate, {'date': '10/01/2026'}),
      );
      await expectLater(
        repository.registerLoanRepayment(repayment(loanId, date: 'x')),
        financeError(FinanceErrorCode.invalidDate, {'date': 'x'}),
      );
      await expectLater(
        repository.createTransfer(
          fromAccountId: a,
          toAccountId: b,
          valorSaida: 10,
          valorEntrada: 10,
          date: 'x',
        ),
        financeError(FinanceErrorCode.invalidDate, {'date': 'x'}),
      );
    });

    test('nomes obrigatórios: dívida, devedor e categoria', () async {
      final id = await repository.insertAccount(account('Nubank', 'BRL'));

      await expectLater(
        repository.createLiability(liability(id, nome: '   ')),
        financeError(FinanceErrorCode.liabilityNameRequired),
      );
      await expectLater(
        repository.insertBorrower(Borrower(nome: '  ', createdAt: DateTime(2026, 1, 1))),
        financeError(FinanceErrorCode.borrowerNameRequired),
      );
      await expectLater(
        repository.insertCategory(GeneralTransactionCategory(nome: ' ', createdAt: DateTime(2026, 1, 1))),
        financeError(FinanceErrorCode.categoryNameRequired),
      );
    });

    test('categoryLimitMustBePositive recusa zero, negativo e não finito', () async {
      for (final limit in [0.0, -1.0, double.nan, double.infinity]) {
        await expectLater(
          repository.insertCategory(
            GeneralTransactionCategory(nome: 'Mercado', limiteMensal: limit, createdAt: DateTime(2026, 1, 1)),
          ),
          financeError(FinanceErrorCode.categoryLimitMustBePositive),
          reason: '$limit',
        );
      }
    });

    test('empréstimo: principal e taxa mensal', () async {
      final who = await borrower();

      await expectLater(
        repository.createLoan(loan(who, principal: 0)),
        financeError(FinanceErrorCode.principalMustBePositive),
      );
      await expectLater(
        repository.createLoan(loan(who, rate: -1)),
        financeError(FinanceErrorCode.negativeMonthlyRate),
      );
      final id = await repository.createLoan(loan(who));
      await expectLater(
        repository.updateLoan(id, taxaMensalPercentual: -0.5, includeInNetWorth: true),
        financeError(FinanceErrorCode.negativeMonthlyRate),
      );
    });

    test('baixa de empréstimo: valor baixado, valor recebido e moeda da conta', () async {
      final brl = await repository.insertAccount(account('Nubank', 'BRL'));
      final usd = await repository.insertAccount(account('Chase', 'USD'));
      final who = await borrower();
      final loanId = await repository.createLoan(loan(who, accountId: brl));

      await expectLater(
        repository.registerLoanRepayment(repayment(loanId, writtenOff: 0)),
        financeError(FinanceErrorCode.repaidAmountMustBePositive),
      );
      await expectLater(
        repository.registerLoanRepayment(repayment(loanId, received: -1)),
        financeError(FinanceErrorCode.receivedAmountNegative),
      );
      await expectLater(
        repository.registerLoanRepayment(repayment(loanId, accountId: usd, received: 50)),
        financeError(FinanceErrorCode.loanCurrencyMismatch, {'loanCurrency': 'BRL', 'accountCurrency': 'USD'}),
      );
    });

    test('transferência: valores, mesma conta e mesma moeda com valores diferentes', () async {
      final a = await repository.insertAccount(account('Alfa', 'BRL'));
      final b = await repository.insertAccount(account('Beta', 'BRL'));

      Future<int> transfer({int? to, double out = 10, double inn = 10}) => repository.createTransfer(
        fromAccountId: a,
        toAccountId: to ?? b,
        valorSaida: out,
        valorEntrada: inn,
        date: '2026-01-02',
      );

      await expectLater(transfer(out: 0), financeError(FinanceErrorCode.transferAmountsMustBePositive));
      await expectLater(transfer(inn: -1), financeError(FinanceErrorCode.transferAmountsMustBePositive));
      await expectLater(transfer(to: a), financeError(FinanceErrorCode.transferSameAccount));
      await expectLater(
        transfer(out: 10, inn: 11),
        financeError(FinanceErrorCode.transferSameCurrencyAmountsDiffer),
      );
    });

    // O cálculo das parcelas (domínio) também lança erro tipado: principal, taxa e prazo.
    test('computeInstallments: principal, taxa mensal e prazo inválidos', () {
      List<Installment> compute({double principal = 1000, double rate = 0.01, int months = 12}) =>
          computeInstallments(
            principal: principal,
            taxaMensal: rate,
            prazoMeses: months,
            sistema: AmortizationSystem.sac,
          );
      Matcher code(FinanceErrorCode c) =>
          throwsA(isA<FinanceArgumentError>().having((e) => e.code, 'code', c));

      expect(() => compute(principal: 0), code(FinanceErrorCode.principalMustBePositive));
      expect(() => compute(principal: double.nan), code(FinanceErrorCode.principalMustBePositive));
      expect(() => compute(rate: -0.01), code(FinanceErrorCode.negativeMonthlyRate));
      expect(() => compute(months: 0), code(FinanceErrorCode.termMustBePositive));
      expect(() => compute(), returnsNormally);
    });

    test('createLiability: principal e prazo ruins viram os mesmos códigos', () async {
      final id = await repository.insertAccount(account('Nubank', 'BRL'));
      Liability bad({double principal = 1200, int months = 12}) => Liability(
        bankAccountId: id,
        nome: 'Carro',
        titular: 'Eu',
        principal: principal,
        taxaPercentual: 1,
        prazoMeses: months,
        sistema: AmortizationSystem.sac,
        dataInicio: '2026-01-31',
        createdAt: DateTime(2026, 1, 1),
      );

      await expectLater(
        repository.createLiability(bad(principal: 0)),
        financeError(FinanceErrorCode.principalMustBePositive),
      );
      await expectLater(
        repository.createLiability(bad(months: 0)),
        financeError(FinanceErrorCode.termMustBePositive),
      );
    });
  });

  group('StateError', () {
    test('accountNotFound em edição, dívida, empréstimo, baixa e transferência', () async {
      final a = await repository.insertAccount(account('Alfa', 'BRL'));
      final who = await borrower();
      final loanId = await repository.createLoan(loan(who));

      await expectLater(
        repository.updateAccount(
          BankAccount(id: 999, nome: 'X', titular: 'Eu', currency: 'BRL', createdAt: DateTime(2026, 1, 1)),
        ),
        financeError(FinanceErrorCode.accountNotFound),
      );
      await expectLater(
        repository.createLiability(liability(999)),
        financeError(FinanceErrorCode.accountNotFound),
      );
      await expectLater(
        repository.createLoan(loan(who, accountId: 999)),
        financeError(FinanceErrorCode.accountNotFound),
      );
      await expectLater(
        repository.registerLoanRepayment(repayment(loanId, accountId: 999, received: 50)),
        financeError(FinanceErrorCode.accountNotFound),
      );
      await expectLater(
        repository.createTransfer(
          fromAccountId: a,
          toAccountId: 999,
          valorSaida: 10,
          valorEntrada: 10,
          date: '2026-01-02',
        ),
        financeError(FinanceErrorCode.accountNotFound),
      );
    });

    test('accountCurrencyLocked: a moeda só muda enquanto a conta não tem lançamento', () async {
      final id = await repository.insertAccount(account('Nubank', 'BRL'));
      await repository.insertTransaction(tx(id));

      await expectLater(
        repository.updateAccount(
          BankAccount(id: id, nome: 'Nubank', titular: 'Eu', currency: 'USD', createdAt: DateTime(2026, 1, 1)),
        ),
        financeError(FinanceErrorCode.accountCurrencyLocked),
      );
      // Mesma moeda com lançamentos continua permitido (só renomeia).
      await repository.updateAccount(
        BankAccount(id: id, nome: 'Nubank 2', titular: 'Eu', currency: 'BRL', createdAt: DateTime(2026, 1, 1)),
      );
    });

    test('borrowerHasLoans e borrowerNotFound', () async {
      final who = await borrower();
      await repository.createLoan(loan(who));

      await expectLater(repository.deleteBorrower(who), financeError(FinanceErrorCode.borrowerHasLoans));
      await expectLater(
        repository.createLoan(loan(999)),
        financeError(FinanceErrorCode.borrowerNotFound),
      );
    });

    test('loanNotFound na edição e na baixa', () async {
      await expectLater(
        repository.updateLoan(999, taxaMensalPercentual: null, includeInNetWorth: true),
        financeError(FinanceErrorCode.loanNotFound),
      );
      await expectLater(
        repository.registerLoanRepayment(repayment(999)),
        financeError(FinanceErrorCode.loanNotFound),
      );
    });

    test('linkedTransactionLocked: parcela de dívida só sai junto com a dívida', () async {
      final id = await repository.insertAccount(account('Nubank', 'BRL'));
      await repository.createLiability(liability(id));
      final installment = (await repository.listTransactions()).first;

      await expectLater(
        repository.deleteTransaction(installment.id!),
        financeError(FinanceErrorCode.linkedTransactionLocked),
      );
    });
  });
}
