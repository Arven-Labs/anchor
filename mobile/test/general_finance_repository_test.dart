import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/domain/general_finance.dart';
import 'package:anchor_mobile/domain/liability.dart';
import 'package:anchor_mobile/models/bank_account.dart';
import 'package:anchor_mobile/models/liability.dart';
import 'package:anchor_mobile/models/loan.dart';
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

  group('transferência', () {
    Future<int> open(String nome, String currency) =>
        repository.insertAccount(account(nome, currency));

    test('mesma moeda: duas pernas com o mesmo transfer_id e saldo que se move', () async {
      final nubank = await open('Nubank', 'BRL');
      final inter = await open('Inter', 'BRL');

      final id = await repository.createTransfer(
        fromAccountId: nubank,
        toAccountId: inter,
        valorSaida: 400,
        valorEntrada: 400,
        date: '2026-08-05',
        notes: 'Reserva',
      );

      final txs = await repository.listTransactions();
      expect(txs, hasLength(2));
      expect(txs.every((t) => t.transferId == id), isTrue);
      final out = txs.firstWhere((t) => t.bankAccountId == nubank);
      final incoming = txs.firstWhere((t) => t.bankAccountId == inter);
      expect(out.type, GeneralTransactionType.transferenciaSaida);
      expect(incoming.type, GeneralTransactionType.transferenciaEntrada);
      expect(out.id, id);
      expect(out.notes, 'Reserva');
      expect(computeBalances(txs, '2026-12-31'), {nubank: -400.0, inter: 400.0});
    });

    test('moedas diferentes: cada perna com o seu valor', () async {
      final nubank = await open('Nubank', 'BRL');
      final chase = await open('Chase', 'USD');

      await repository.createTransfer(
        fromAccountId: nubank,
        toAccountId: chase,
        valorSaida: 520,
        valorEntrada: 100,
        date: '2026-08-05',
      );

      final balances = computeBalances(
        await repository.listTransactions(),
        '2026-12-31',
      );
      expect(balances, {nubank: -520.0, chase: 100.0});
    });

    test('recusa mesma conta, valor inválido, data ruim e entrada diferente na mesma moeda', () async {
      final a = await open('A', 'BRL');
      final b = await open('B', 'BRL');

      Future<void> transfer({
        int? from,
        int? to,
        double out = 10,
        double incoming = 10,
        String date = '2026-08-05',
      }) => repository.createTransfer(
        fromAccountId: from ?? a,
        toAccountId: to ?? b,
        valorSaida: out,
        valorEntrada: incoming,
        date: date,
      );

      expect(() => transfer(to: a), throwsArgumentError);
      expect(() => transfer(out: 0, incoming: 0), throwsArgumentError);
      expect(() => transfer(out: double.nan), throwsArgumentError);
      expect(() => transfer(date: '05/08/2026'), throwsArgumentError);
      expect(() => transfer(incoming: 9), throwsArgumentError);
      expect(() => transfer(to: 999), throwsStateError);
      expect(await repository.listTransactions(), isEmpty);
    });

    test('apagar uma perna apaga a outra; lançamento comum fica de pé', () async {
      final a = await open('A', 'BRL');
      final b = await open('B', 'BRL');
      await repository.insertTransaction(
        tx(a, GeneralTransactionType.receita, 5, '2026-08-01'),
      );
      await repository.createTransfer(
        fromAccountId: a,
        toAccountId: b,
        valorSaida: 10,
        valorEntrada: 10,
        date: '2026-08-05',
      );
      final incoming = (await repository.listTransactions()).firstWhere(
        (t) => t.type == GeneralTransactionType.transferenciaEntrada,
      );

      await repository.deleteTransaction(incoming.id!);

      final left = await repository.listTransactions();
      expect(left.single.type, GeneralTransactionType.receita);
    });

    test('apagar a conta apaga também a perna da conta do outro lado', () async {
      final a = await open('A', 'BRL');
      final b = await open('B', 'BRL');
      await repository.createTransfer(
        fromAccountId: a,
        toAccountId: b,
        valorSaida: 10,
        valorEntrada: 10,
        date: '2026-08-05',
      );
      await repository.insertTransaction(
        tx(b, GeneralTransactionType.receita, 3, '2026-08-06'),
      );

      await repository.deleteAccount(a);

      final left = await repository.listTransactions();
      expect(left.single.type, GeneralTransactionType.receita);
      expect(left.single.bankAccountId, b);
    });

    test('lançamento manual não aceita tipo de transferência', () async {
      final a = await open('A', 'BRL');

      expect(
        () => repository.insertTransaction(
          tx(a, GeneralTransactionType.transferenciaSaida, 10, '2026-08-05'),
        ),
        throwsArgumentError,
      );
    });
  });

  group('dívida', () {
    Liability liability(int accountId, {String start = '2026-01-31'}) => Liability(
      bankAccountId: accountId,
      nome: 'Carro',
      titular: 'Eu',
      principal: 1200,
      taxaPercentual: 1,
      prazoMeses: 12,
      sistema: AmortizationSystem.sac,
      dataInicio: start,
      createdAt: DateTime(2026, 1, 1),
    );

    test('gera uma parcela por mês como lançamento da conta, com vencimento no fim do mês', () async {
      final id = await repository.insertAccount(account('Nubank', 'BRL'));

      await repository.createLiability(liability(id));

      final txs = await repository.listTransactions();
      expect(txs, hasLength(12));
      expect(
        txs.every((t) => t.type == GeneralTransactionType.parcelaDivida),
        isTrue,
      );
      final dates = txs.map((t) => t.date).toList()..sort();
      expect(dates.first, '2026-02-28');
      expect(dates[1], '2026-03-31');
      expect(dates.last, '2027-01-31');
      expect(txs.firstWhere((t) => t.date == '2026-02-28').valor, closeTo(112, 1e-9));
      expect(txs.firstWhere((t) => t.date == '2026-02-28').notes, 'Carro — parcela 1/12');

      final listed = (await repository.listLiabilities()).single;
      expect(listed.installments, hasLength(12));
      expect(listed.balanceOn('2026-03-15'), closeTo(1100, 1e-9));
      expect(listed.paidOn('2026-03-15'), 1);
      expect(listed.nextAfter('2026-03-15')!.numeroParcela, 2);
      expect(listed.nextAfter('2030-01-01'), isNull);
    });

    test('só a parcela vencida entra no saldo da conta', () async {
      final id = await repository.insertAccount(account('Nubank', 'BRL'));
      await repository.createLiability(liability(id));

      final balances = computeBalances(
        await repository.listTransactions(),
        '2026-03-15',
      );

      expect(balances[id], closeTo(-112, 1e-9));
    });

    test('recusa nome vazio, data ruim, dados inválidos e conta inexistente, sem gravar nada', () async {
      final id = await repository.insertAccount(account('Nubank', 'BRL'));
      Liability copy({String? nome, String? start, double? principal, int? term}) => Liability(
        bankAccountId: id,
        nome: nome ?? 'Carro',
        titular: 'Eu',
        principal: principal ?? 1200,
        taxaPercentual: 1,
        prazoMeses: term ?? 12,
        sistema: AmortizationSystem.price,
        dataInicio: start ?? '2026-01-31',
        createdAt: DateTime(2026, 1, 1),
      );

      expect(() => repository.createLiability(copy(nome: ' ')), throwsArgumentError);
      expect(() => repository.createLiability(copy(start: '31/01/2026')), throwsArgumentError);
      expect(() => repository.createLiability(copy(principal: 0)), throwsArgumentError);
      expect(() => repository.createLiability(copy(term: 0)), throwsArgumentError);
      expect(() => repository.createLiability(liability(999)), throwsStateError);
      expect(await repository.listLiabilities(), isEmpty);
      expect(await repository.listTransactions(), isEmpty);
    });

    test('parcela não se apaga solta, e lançamento manual não aceita o tipo', () async {
      final id = await repository.insertAccount(account('Nubank', 'BRL'));
      await repository.createLiability(liability(id));
      final parcela = (await repository.listTransactions()).first;

      expect(() => repository.deleteTransaction(parcela.id!), throwsStateError);
      expect(
        () => repository.insertTransaction(
          tx(id, GeneralTransactionType.parcelaDivida, 10, '2026-08-05'),
        ),
        throwsArgumentError,
      );
      expect(await repository.listTransactions(), hasLength(12));
    });

    test('apagar a dívida apaga as parcelas e os lançamentos dela, e preserva os outros', () async {
      final id = await repository.insertAccount(account('Nubank', 'BRL'));
      await repository.insertTransaction(
        tx(id, GeneralTransactionType.receita, 5, '2026-08-01'),
      );
      final debt = await repository.createLiability(liability(id));

      await repository.deleteLiability(debt);

      expect(await repository.listLiabilities(), isEmpty);
      final left = await repository.listTransactions();
      expect(left.single.type, GeneralTransactionType.receita);
      final db = await AppDatabase.instance();
      expect(await db.query('liability_installments'), isEmpty);
    });

    test('apagar a conta leva a dívida junto', () async {
      final a = await repository.insertAccount(account('A', 'BRL'));
      final b = await repository.insertAccount(account('B', 'BRL'));
      await repository.createLiability(liability(a));
      await repository.createLiability(liability(b));

      await repository.deleteAccount(a);

      final left = await repository.listLiabilities();
      expect(left.single.liability.bankAccountId, b);
      expect(await repository.listTransactions(), hasLength(12));
      final db = await AppDatabase.instance();
      expect(await db.query('liability_installments'), hasLength(12));
    });
  });

  group('empréstimo concedido', () {
    Future<int> borrower([String nome = 'Irmão']) => repository.insertBorrower(
      Borrower(nome: nome, createdAt: DateTime(2026, 1, 1)),
    );

    Loan loan(
      int borrowerId, {
      int? accountId,
      double principal = 1000,
      double? rate,
      String currency = 'BRL',
      bool include = true,
    }) => Loan(
      borrowerId: borrowerId,
      bankAccountId: accountId,
      principal: principal,
      taxaMensalPercentual: rate,
      dataInicio: '2026-01-10',
      includeInNetWorth: include,
      currency: currency,
      createdAt: DateTime(2026, 1, 10),
    );

    LoanRepayment repayment(
      int loanId, {
      int? accountId,
      double received = 0,
      double writtenOff = 100,
    }) => LoanRepayment(
      loanId: loanId,
      bankAccountId: accountId,
      data: '2026-03-10',
      valorRecebido: received,
      valorBaixado: writtenOff,
      createdAt: DateTime(2026, 3, 10),
    );

    test('com conta: a saída do principal vira lançamento e a moeda é a da conta', () async {
      final usd = await repository.insertAccount(account('Chase', 'USD'));
      final who = await borrower();

      await repository.createLoan(loan(who, accountId: usd, currency: 'BRL'));

      final txs = await repository.listTransactions();
      expect(txs.single.type, GeneralTransactionType.emprestimoConcedido);
      expect(txs.single.valor, 1000);
      expect(txs.single.date, '2026-01-10');
      expect(txs.single.notes, 'Empréstimo para Irmão');
      expect(computeBalances(txs, '2026-12-31'), {usd: -1000.0});
      final listed = (await repository.listLoans()).single;
      expect(listed.loan.currency, 'USD');
      expect(listed.loan.generalTransactionId, txs.single.id);
    });

    test('sem conta: só registro, na moeda informada, e nada mexe em saldo', () async {
      final who = await borrower();

      await repository.createLoan(loan(who, currency: ' eur '));

      expect(await repository.listTransactions(), isEmpty);
      expect((await repository.listLoans()).single.loan.currency, 'EUR');
    });

    test('taxa zero grava sem juros; recusa principal, taxa negativa, data, moeda e devedor ruins', () async {
      final who = await borrower();
      await repository.createLoan(loan(who, rate: 0));
      expect((await repository.listLoans()).single.loan.taxaMensalPercentual, isNull);

      expect(() => repository.createLoan(loan(who, principal: 0)), throwsArgumentError);
      expect(() => repository.createLoan(loan(who, rate: -1)), throwsArgumentError);
      expect(() => repository.createLoan(loan(who, currency: 'XXX')), throwsArgumentError);
      expect(() => repository.createLoan(loan(999)), throwsStateError);
      expect(
        () => repository.createLoan(loan(who, accountId: 999)),
        throwsStateError,
      );
      expect(await repository.listLoans(), hasLength(1));
    });

    test('baixa com conta: o recebimento entra na conta; perdão não lança nada', () async {
      final nubank = await repository.insertAccount(account('Nubank', 'BRL'));
      final who = await borrower();
      final id = await repository.createLoan(loan(who, accountId: nubank));

      await repository.registerLoanRepayment(
        repayment(id, accountId: nubank, received: 110, writtenOff: 100),
      );
      await repository.registerLoanRepayment(repayment(id, writtenOff: 50));

      final txs = await repository.listTransactions();
      final received = txs.singleWhere(
        (t) => t.type == GeneralTransactionType.recebimentoEmprestimo,
      );
      expect(received.valor, 110);
      expect(received.notes, 'Recebimento de Irmão');
      expect(computeBalances(txs, '2026-12-31'), {nubank: -890.0}); // -1000 + 110
      final item = (await repository.listLoans()).single;
      expect(item.repayments, hasLength(2));
      expect(item.totalWrittenOff, 150);
      expect(item.extra, closeTo(-40, 1e-9)); // +10 a mais, 50 perdoados
      expect(item.balanceOn(DateTime(2026, 1, 10)), 850);
    });

    test('baixa recusa valores ruins, data ruim, empréstimo e conta inexistentes e moeda diferente', () async {
      final usd = await repository.insertAccount(account('Chase', 'USD'));
      final who = await borrower();
      final id = await repository.createLoan(loan(who));

      expect(() => repository.registerLoanRepayment(repayment(id, writtenOff: 0)), throwsArgumentError);
      expect(() => repository.registerLoanRepayment(repayment(id, received: -1)), throwsArgumentError);
      expect(
        () => repository.registerLoanRepayment(
          LoanRepayment(
            loanId: id,
            data: '10/03/2026',
            valorRecebido: 0,
            valorBaixado: 10,
            createdAt: DateTime(2026, 3, 10),
          ),
        ),
        throwsArgumentError,
      );
      expect(() => repository.registerLoanRepayment(repayment(999)), throwsStateError);
      expect(
        () => repository.registerLoanRepayment(repayment(id, accountId: 999)),
        throwsStateError,
      );
      // Empréstimo em BRL, conta em USD.
      expect(
        () => repository.registerLoanRepayment(repayment(id, accountId: usd, received: 10)),
        throwsArgumentError,
      );
      expect((await repository.listLoans()).single.repayments, isEmpty);
      expect(await repository.listTransactions(), isEmpty);
    });

    test('apagar a baixa leva o recebimento; apagar o empréstimo leva baixas e lançamentos', () async {
      final nubank = await repository.insertAccount(account('Nubank', 'BRL'));
      await repository.insertTransaction(
        tx(nubank, GeneralTransactionType.receita, 5, '2026-01-01'),
      );
      final who = await borrower();
      final id = await repository.createLoan(loan(who, accountId: nubank));
      final first = await repository.registerLoanRepayment(
        repayment(id, accountId: nubank, received: 100),
      );
      await repository.registerLoanRepayment(
        repayment(id, accountId: nubank, received: 200, writtenOff: 200),
      );

      await repository.deleteLoanRepayment(first);
      expect(
        (await repository.listTransactions())
            .where((t) => t.type == GeneralTransactionType.recebimentoEmprestimo)
            .map((t) => t.valor),
        [200.0],
      );

      await repository.deleteLoan(id);

      expect(await repository.listLoans(), isEmpty);
      final left = await repository.listTransactions();
      expect(left.single.type, GeneralTransactionType.receita);
      final db = await AppDatabase.instance();
      expect(await db.query('loan_repayments'), isEmpty);
    });

    test('lançamento gerado não se apaga solta e o manual não aceita o tipo', () async {
      final nubank = await repository.insertAccount(account('Nubank', 'BRL'));
      final who = await borrower();
      await repository.createLoan(loan(who, accountId: nubank));
      final generated = (await repository.listTransactions()).single;

      expect(() => repository.deleteTransaction(generated.id!), throwsStateError);
      for (final type in [
        GeneralTransactionType.emprestimoConcedido,
        GeneralTransactionType.recebimentoEmprestimo,
      ]) {
        expect(
          () => repository.insertTransaction(tx(nubank, type, 10, '2026-08-05')),
          throwsArgumentError,
        );
      }
    });

    test('editar muda taxa, flag e observação; devedor com empréstimo não se apaga', () async {
      final who = await borrower();
      final id = await repository.createLoan(loan(who, rate: 2));

      await repository.updateLoan(
        id,
        taxaMensalPercentual: 0,
        includeInNetWorth: false,
        notes: '  família  ',
      );

      final changed = (await repository.listLoans()).single.loan;
      expect(changed.taxaMensalPercentual, isNull);
      expect(changed.includeInNetWorth, isFalse);
      expect(changed.notes, 'família');
      expect(changed.principal, 1000);
      expect(
        () => repository.updateLoan(id, taxaMensalPercentual: -1, includeInNetWorth: true),
        throwsArgumentError,
      );
      expect(
        () => repository.updateLoan(999, taxaMensalPercentual: null, includeInNetWorth: true),
        throwsStateError,
      );
      expect(() => repository.deleteBorrower(who), throwsStateError);

      await repository.deleteLoan(id);
      await repository.deleteBorrower(who);
      expect(await repository.listBorrowers(), isEmpty);
    });

    test('apagar a conta deixa o empréstimo como registro, sem a conta', () async {
      final a = await repository.insertAccount(account('A', 'BRL'));
      final who = await borrower();
      final id = await repository.createLoan(loan(who, accountId: a));
      await repository.registerLoanRepayment(
        repayment(id, accountId: a, received: 100),
      );

      await repository.deleteAccount(a);

      final item = (await repository.listLoans()).single;
      expect(item.loan.bankAccountId, isNull);
      expect(item.loan.generalTransactionId, isNull);
      expect(item.repayments.single.bankAccountId, isNull);
      expect(item.repayments.single.generalTransactionId, isNull);
      expect(item.loan.currency, 'BRL');
      expect(await repository.listTransactions(), isEmpty);
    });

    test('devedores listados por nome', () async {
      await borrower('beto');
      await borrower('Ana');

      expect((await repository.listBorrowers()).map((b) => b.nome), ['Ana', 'beto']);
      expect(() => repository.insertBorrower(Borrower(nome: '  ', createdAt: DateTime(2026, 1, 1))), throwsArgumentError);
    });
  });

  test('banco v19 sobe para a v20 e ganha as tabelas de empréstimo', () async {
    final old = await openDatabase(
      '${dir.path}/anchor_mobile.db',
      version: 19,
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
          created_at TEXT NOT NULL, category_id INTEGER, transfer_id INTEGER)''');
        await db.execute('''CREATE TABLE liabilities (
          id INTEGER PRIMARY KEY AUTOINCREMENT)''');
        await db.insert('bank_accounts', {
          'nome': 'Velha',
          'titular': 'Eu',
          'currency': 'BRL',
          'created_at': '2026-01-01T00:00:00.000',
        });
      },
    );
    await old.close();

    expect(await repository.listLoans(), isEmpty);
    final who = await repository.insertBorrower(
      Borrower(nome: 'Irmão', createdAt: DateTime(2026, 1, 1)),
    );
    await repository.createLoan(
      Loan(
        borrowerId: who,
        bankAccountId: 1,
        principal: 10,
        dataInicio: '2026-01-10',
        includeInNetWorth: true,
        currency: 'BRL',
        createdAt: DateTime(2026, 1, 10),
      ),
    );
    expect(await repository.listTransactions(), hasLength(1));
  });

  test('banco v18 sobe para a v19 e ganha as tabelas de dívida', () async {
    final old = await openDatabase(
      '${dir.path}/anchor_mobile.db',
      version: 18,
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
          created_at TEXT NOT NULL, category_id INTEGER, transfer_id INTEGER)''');
        await db.insert('bank_accounts', {
          'nome': 'Velha',
          'titular': 'Eu',
          'currency': 'BRL',
          'created_at': '2026-01-01T00:00:00.000',
        });
      },
    );
    await old.close();

    expect(await repository.listLiabilities(), isEmpty);
    await repository.createLiability(
      Liability(
        bankAccountId: 1,
        nome: 'Carro',
        titular: 'Eu',
        principal: 1000,
        taxaPercentual: 0,
        prazoMeses: 2,
        sistema: AmortizationSystem.price,
        dataInicio: '2026-01-10',
        createdAt: DateTime(2026, 1, 1),
      ),
    );
    expect(await repository.listTransactions(), hasLength(2));
  });

  test('banco v17 sobe para a v18 e ganha transfer_id', () async {
    final old = await openDatabase(
      '${dir.path}/anchor_mobile.db',
      version: 17,
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
          created_at TEXT NOT NULL, category_id INTEGER)''');
        await db.insert('bank_accounts', {
          'nome': 'Velha',
          'titular': 'Eu',
          'currency': 'BRL',
          'created_at': '2026-01-01T00:00:00.000',
        });
        await db.insert('general_transactions', {
          'bank_account_id': 1,
          'transaction_type': 'receita',
          'valor': 9.0,
          'transaction_date': '2026-08-05',
          'created_at': '2026-01-01T00:00:00.000',
        });
      },
    );
    await old.close();

    final txs = await repository.listTransactions();
    expect(txs.single.valor, 9.0);
    expect(txs.single.transferId, isNull);
    final other = await repository.insertAccount(account('Nova', 'BRL'));
    await repository.createTransfer(
      fromAccountId: 1,
      toAccountId: other,
      valorSaida: 4,
      valorEntrada: 4,
      date: '2026-08-06',
    );
    expect(await repository.listTransactions(), hasLength(3));
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
