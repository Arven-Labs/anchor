import 'package:sqflite/sqflite.dart' show ConflictAlgorithm, DatabaseExecutor;

import '../data/db.dart';
import '../domain/currency_catalog.dart';
import '../domain/finance_error.dart';
import '../domain/generated_tx_info.dart';
import '../models/bank_account.dart';
import '../models/general_transaction.dart';
import '../models/general_transaction_category.dart';
import '../models/liability.dart';
import '../models/loan.dart';
import '../domain/general_finance.dart' show ReconciliationPreview, reconciliationPreview, toIsoDay;
import '../domain/liability.dart';
import '../domain/porquinho_yield.dart';

/// Contas, lançamentos e a moeda base de Finanças Gerais. Cálculo de saldo e
/// de patrimônio fica em `domain/general_finance.dart`, puro.
class GeneralFinanceRepository {
  static const _baseCurrencyKey = 'base_currency';

  Future<int> insertAccount(BankAccount account) async {
    final currency = validateCurrency(account.currency);
    if (currency == null) {
      throw FinanceArgumentError(FinanceErrorCode.unknownCurrency, {'currency': account.currency});
    }
    final db = await AppDatabase.instance();
    final map = BankAccount(
      nome: account.nome,
      titular: account.titular,
      currency: currency,
      createdAt: account.createdAt,
    ).toMap()..remove('id');
    return db.insert('bank_accounts', map);
  }

  Future<List<BankAccount>> listAccounts() async {
    final db = await AppDatabase.instance();
    final rows = await db.query('bank_accounts', orderBy: 'nome');
    return rows.map(BankAccount.fromMap).toList();
  }

  /// Nome, titular e moeda. A moeda só muda enquanto a conta não tem
  /// lançamento: com histórico, ele seria relido em outra moeda (mesma regra
  /// do desktop).
  Future<void> updateAccount(BankAccount account) async {
    final currency = validateCurrency(account.currency);
    if (currency == null) {
      throw FinanceArgumentError(FinanceErrorCode.unknownCurrency, {'currency': account.currency});
    }
    final db = await AppDatabase.instance();
    final rows = await db.query(
      'bank_accounts',
      where: 'id = ?',
      whereArgs: [account.id],
    );
    if (rows.isEmpty) throw FinanceStateError(FinanceErrorCode.accountNotFound);
    final current = BankAccount.fromMap(rows.first);
    if (current.currency != currency && await accountInUse(account.id!)) {
      throw FinanceStateError(FinanceErrorCode.accountCurrencyLocked);
    }
    await db.update(
      'bank_accounts',
      {'nome': account.nome, 'titular': account.titular, 'currency': currency},
      where: 'id = ?',
      whereArgs: [account.id],
    );
  }

  /// Apaga a conta e os lançamentos dela (o banco não aplica `ON DELETE
  /// CASCADE`: `PRAGMA foreign_keys` está desligado).
  Future<void> deleteAccount(int id) async {
    final db = await AppDatabase.instance();
    await db.transaction((txn) async {
      // As parcelas da dívida (lançamentos da própria conta) saem no delete
      // abaixo; aqui vão os registros da dívida.
      await txn.delete(
        'liability_installments',
        where:
            'liability_id IN (SELECT id FROM liabilities WHERE bank_account_id = ?)',
        whereArgs: [id],
      );
      await txn.delete(
        'liabilities',
        where: 'bank_account_id = ?',
        whereArgs: [id],
      );
      // Empréstimo e baixa ficam como registro, sem a conta: os lançamentos
      // deles (da própria conta) saem no delete abaixo.
      for (final table in ['loans', 'loan_repayments']) {
        await txn.update(
          table,
          {'bank_account_id': null, 'general_transaction_id': null},
          where: 'bank_account_id = ?',
          whereArgs: [id],
        );
      }
      // A outra perna de cada transferência da conta vai junto: sozinha ela
      // deixaria dinheiro entrando ou saindo do nada.
      await txn.delete(
        'general_transactions',
        where:
            'bank_account_id = ? OR transfer_id IN '
            '(SELECT transfer_id FROM general_transactions '
            'WHERE bank_account_id = ? AND transfer_id IS NOT NULL)',
        whereArgs: [id, id],
      );
      await txn.delete('bank_accounts', where: 'id = ?', whereArgs: [id]);
    });
  }

  Future<bool> accountInUse(int id) async {
    final db = await AppDatabase.instance();
    final rows = await db.query(
      'general_transactions',
      columns: ['id'],
      where: 'bank_account_id = ?',
      whereArgs: [id],
      limit: 1,
    );
    return rows.isNotEmpty;
  }

  Future<int> insertTransaction(GeneralTransaction tx) async {
    if (!tx.valor.isFinite || tx.valor <= 0) {
      throw FinanceArgumentError(FinanceErrorCode.amountMustBePositive);
    }
    if (DateTime.tryParse(tx.date) == null || tx.date.length != 10) {
      throw FinanceArgumentError(FinanceErrorCode.invalidDate, {'date': tx.date});
    }
    if (!GeneralTransactionType.manual.contains(tx.type)) {
      throw FinanceArgumentError(FinanceErrorCode.manualTypeOnly);
    }
    final db = await AppDatabase.instance();
    final map = tx.toMap()..remove('id');
    // Categoria é de despesa: receita não entra em orçamento.
    if (tx.type != GeneralTransactionType.despesa) map['category_id'] = null;
    return db.insert('general_transactions', map);
  }

  /// Cadastra a dívida e gera todas as parcelas como lançamentos
  /// `parcela_divida` na conta (parcela `n` vence `n` meses depois do início),
  /// numa transação só. Parcela futura não entra no saldo da conta; o saldo
  /// devedor cai quando ela vence.
  Future<int> createLiability(Liability liability) async {
    final nome = liability.nome.trim();
    if (nome.isEmpty) throw FinanceArgumentError(FinanceErrorCode.liabilityNameRequired);
    final start = DateTime.tryParse(liability.dataInicio);
    if (start == null || liability.dataInicio.length != 10) {
      throw FinanceArgumentError(FinanceErrorCode.invalidDate, {'date': liability.dataInicio});
    }
    final schedule = computeInstallments(
      principal: liability.principal,
      taxaMensal: liability.taxaPercentual / 100,
      prazoMeses: liability.prazoMeses,
      sistema: liability.sistema,
    );
    final db = await AppDatabase.instance();
    return db.transaction((txn) async {
      final accounts = await txn.query(
        'bank_accounts',
        where: 'id = ?',
        whereArgs: [liability.bankAccountId],
      );
      if (accounts.isEmpty) throw FinanceStateError(FinanceErrorCode.accountNotFound);
      final now = liability.createdAt.toIso8601String();
      final liabilityId = await txn.insert(
        'liabilities',
        Liability(
          bankAccountId: liability.bankAccountId,
          nome: nome,
          titular: liability.titular,
          principal: liability.principal,
          taxaPercentual: liability.taxaPercentual,
          prazoMeses: liability.prazoMeses,
          sistema: liability.sistema,
          dataInicio: liability.dataInicio,
          createdAt: liability.createdAt,
        ).toMap()..remove('id'),
      );
      for (final installment in schedule) {
        final due = toIsoDay(addMonths(start, installment.numeroParcela));
        final transactionId = await txn.insert('general_transactions', {
          'bank_account_id': liability.bankAccountId,
          'transaction_type': GeneralTransactionType.parcelaDivida.value,
          'valor': installment.valorParcela,
          'transaction_date': due,
          // Sem `notes`: a tela deriva a frase do vínculo (`generatedTransactionInfo`).
          'created_at': now,
        });
        await txn.insert('liability_installments', {
          'liability_id': liabilityId,
          'general_transaction_id': transactionId,
          'numero_parcela': installment.numeroParcela,
          'valor_parcela': installment.valorParcela,
          'valor_juros': installment.valorJuros,
          'valor_amortizacao': installment.valorAmortizacao,
          'saldo_devedor_apos': installment.saldoDevedorApos,
          'data_vencimento': due,
        });
      }
      return liabilityId;
    });
  }

  /// Dívidas com as parcelas, por nome.
  Future<List<LiabilityWithInstallments>> listLiabilities() async {
    final db = await AppDatabase.instance();
    final liabilities = await db.query('liabilities', orderBy: 'nome COLLATE NOCASE');
    final installments = await db.query(
      'liability_installments',
      orderBy: 'numero_parcela',
    );
    final byLiability = <int, List<LiabilityInstallment>>{};
    for (final row in installments) {
      final installment = LiabilityInstallment.fromMap(row);
      byLiability.putIfAbsent(installment.liabilityId, () => []).add(installment);
    }
    return [
      for (final row in liabilities)
        LiabilityWithInstallments(
          Liability.fromMap(row),
          byLiability[row['id'] as int] ?? const [],
        ),
    ];
  }

  /// Apaga a dívida, as parcelas e os lançamentos que elas geraram (sem isso
  /// ficariam "fantasma" no saldo da conta).
  Future<void> deleteLiability(int id) async {
    final db = await AppDatabase.instance();
    await db.transaction((txn) async {
      await txn.delete(
        'general_transactions',
        where:
            'id IN (SELECT general_transaction_id FROM liability_installments '
            'WHERE liability_id = ?)',
        whereArgs: [id],
      );
      await txn.delete(
        'liability_installments',
        where: 'liability_id = ?',
        whereArgs: [id],
      );
      await txn.delete('liabilities', where: 'id = ?', whereArgs: [id]);
    });
  }

  Future<int> insertBorrower(Borrower borrower) async {
    final nome = borrower.nome.trim();
    if (nome.isEmpty) throw FinanceArgumentError(FinanceErrorCode.borrowerNameRequired);
    final db = await AppDatabase.instance();
    return db.insert('borrowers', {
      'nome': nome,
      'notes': _cleanText(borrower.notes),
      'created_at': borrower.createdAt.toIso8601String(),
    });
  }

  Future<List<Borrower>> listBorrowers() async {
    final db = await AppDatabase.instance();
    final rows = await db.query('borrowers', orderBy: 'nome COLLATE NOCASE');
    return rows.map(Borrower.fromMap).toList();
  }

  /// Só apaga o devedor que não tem empréstimo (mesma regra do desktop).
  Future<void> deleteBorrower(int id) async {
    final db = await AppDatabase.instance();
    final loans = await db.query(
      'loans',
      columns: ['id'],
      where: 'borrower_id = ?',
      whereArgs: [id],
      limit: 1,
    );
    if (loans.isNotEmpty) {
      throw FinanceStateError(FinanceErrorCode.borrowerHasLoans);
    }
    await db.delete('borrowers', where: 'id = ?', whereArgs: [id]);
  }

  /// Cadastra o empréstimo. Com conta, a moeda é a da conta e a saída do
  /// principal vira o lançamento `emprestimo_concedido` na `dataInicio`; sem
  /// conta, vale a moeda informada (do catálogo) e nada mexe em saldo. Taxa
  /// zero ou ausente = sem juros.
  Future<int> createLoan(Loan loan) async {
    if (!loan.principal.isFinite || loan.principal <= 0) {
      throw FinanceArgumentError(FinanceErrorCode.principalMustBePositive);
    }
    final taxa = loan.taxaMensalPercentual;
    if (taxa != null && (!taxa.isFinite || taxa < 0)) {
      throw FinanceArgumentError(FinanceErrorCode.negativeMonthlyRate);
    }
    _validateDay(loan.dataInicio);
    final db = await AppDatabase.instance();
    return db.transaction((txn) async {
      final borrower = await txn.query(
        'borrowers',
        where: 'id = ?',
        whereArgs: [loan.borrowerId],
      );
      if (borrower.isEmpty) throw FinanceStateError(FinanceErrorCode.borrowerNotFound);
      var currency = validateCurrency(loan.currency);
      int? transactionId;
      final accountId = loan.bankAccountId;
      if (accountId != null) {
        final accounts = await txn.query(
          'bank_accounts',
          where: 'id = ?',
          whereArgs: [accountId],
        );
        if (accounts.isEmpty) throw FinanceStateError(FinanceErrorCode.accountNotFound);
        currency = BankAccount.fromMap(accounts.first).currency;
        transactionId = await txn.insert('general_transactions', {
          'bank_account_id': accountId,
          'transaction_type': GeneralTransactionType.emprestimoConcedido.value,
          'valor': loan.principal,
          'transaction_date': loan.dataInicio,
          'created_at': loan.createdAt.toIso8601String(),
        });
      }
      if (currency == null) {
        throw FinanceArgumentError(FinanceErrorCode.unknownCurrency, {'currency': loan.currency});
      }
      return txn.insert(
        'loans',
        Loan(
          borrowerId: loan.borrowerId,
          bankAccountId: accountId,
          generalTransactionId: transactionId,
          principal: loan.principal,
          taxaMensalPercentual: taxa != null && taxa > 0 ? taxa : null,
          dataInicio: loan.dataInicio,
          includeInNetWorth: loan.includeInNetWorth,
          currency: currency,
          notes: _cleanText(loan.notes),
          createdAt: loan.createdAt,
        ).toMap()..remove('id'),
      );
    });
  }

  /// Muda só a taxa, a flag do patrimônio e a observação (o resto é histórico
  /// de caixa, como no desktop).
  Future<void> updateLoan(
    int id, {
    required double? taxaMensalPercentual,
    required bool includeInNetWorth,
    String? notes,
  }) async {
    if (taxaMensalPercentual != null &&
        (!taxaMensalPercentual.isFinite || taxaMensalPercentual < 0)) {
      throw FinanceArgumentError(FinanceErrorCode.negativeMonthlyRate);
    }
    final db = await AppDatabase.instance();
    final changed = await db.update(
      'loans',
      {
        'taxa_mensal_percentual':
            taxaMensalPercentual != null && taxaMensalPercentual > 0
            ? taxaMensalPercentual
            : null,
        'include_in_net_worth': includeInNetWorth ? 1 : 0,
        'notes': _cleanText(notes),
      },
      where: 'id = ?',
      whereArgs: [id],
    );
    if (changed == 0) throw FinanceStateError(FinanceErrorCode.loanNotFound);
  }

  /// Mais recentes primeiro, com devedor e baixas (da mais antiga à mais nova).
  Future<List<LoanWithRepayments>> listLoans() async {
    final db = await AppDatabase.instance();
    final loans = await db.query('loans', orderBy: 'data_inicio DESC, id DESC');
    final borrowers = {
      for (final b in (await db.query('borrowers')).map(Borrower.fromMap))
        b.id!: b,
    };
    final repayments = (await db.query(
      'loan_repayments',
      orderBy: 'data, id',
    )).map(LoanRepayment.fromMap);
    return [
      for (final row in loans)
        LoanWithRepayments(
          Loan.fromMap(row),
          borrowers[row['borrower_id'] as int]!,
          [
            for (final r in repayments)
              if (r.loanId == row['id']) r,
          ],
        ),
    ];
  }

  /// Apaga o empréstimo, as baixas e os lançamentos que eles geraram (sem
  /// isso ficariam "fantasma" no saldo da conta).
  Future<void> deleteLoan(int id) async {
    final db = await AppDatabase.instance();
    await db.transaction((txn) async {
      await txn.delete(
        'general_transactions',
        where:
            'id IN (SELECT general_transaction_id FROM loan_repayments '
            'WHERE loan_id = ? AND general_transaction_id IS NOT NULL) '
            'OR id IN (SELECT general_transaction_id FROM loans '
            'WHERE id = ? AND general_transaction_id IS NOT NULL)',
        whereArgs: [id, id],
      );
      await txn.delete('loan_repayments', where: 'loan_id = ?', whereArgs: [id]);
      await txn.delete('loans', where: 'id = ?', whereArgs: [id]);
    });
  }

  /// Baixa manual: `valorRecebido` é o dinheiro que entrou (0 = perdão) e
  /// `valorBaixado` o que abate do saldo, independente do recebido. Com conta e
  /// `valorRecebido > 0`, a entrada vira o lançamento `recebimento_emprestimo`;
  /// a conta tem de ser da moeda do empréstimo.
  Future<int> registerLoanRepayment(LoanRepayment repayment) async {
    if (!repayment.valorBaixado.isFinite || repayment.valorBaixado <= 0) {
      throw FinanceArgumentError(FinanceErrorCode.repaidAmountMustBePositive);
    }
    if (!repayment.valorRecebido.isFinite || repayment.valorRecebido < 0) {
      throw FinanceArgumentError(FinanceErrorCode.receivedAmountNegative);
    }
    _validateDay(repayment.data);
    final db = await AppDatabase.instance();
    return db.transaction((txn) async {
      final loans = await txn.query(
        'loans',
        where: 'id = ?',
        whereArgs: [repayment.loanId],
      );
      if (loans.isEmpty) throw FinanceStateError(FinanceErrorCode.loanNotFound);
      final loan = Loan.fromMap(loans.first);
      int? transactionId;
      final accountId = repayment.bankAccountId;
      if (accountId != null) {
        final accounts = await txn.query(
          'bank_accounts',
          where: 'id = ?',
          whereArgs: [accountId],
        );
        if (accounts.isEmpty) throw FinanceStateError(FinanceErrorCode.accountNotFound);
        final account = BankAccount.fromMap(accounts.first);
        if (account.currency != loan.currency) {
          throw FinanceArgumentError(FinanceErrorCode.loanCurrencyMismatch, {
            'loanCurrency': loan.currency,
            'accountCurrency': account.currency,
          });
        }
        if (repayment.valorRecebido > 0) {
          transactionId = await txn.insert('general_transactions', {
            'bank_account_id': accountId,
            'transaction_type':
                GeneralTransactionType.recebimentoEmprestimo.value,
            'valor': repayment.valorRecebido,
            'transaction_date': repayment.data,
            'created_at': repayment.createdAt.toIso8601String(),
          });
        }
      }
      return txn.insert(
        'loan_repayments',
        LoanRepayment(
          loanId: repayment.loanId,
          bankAccountId: accountId,
          generalTransactionId: transactionId,
          data: repayment.data,
          valorRecebido: repayment.valorRecebido,
          valorBaixado: repayment.valorBaixado,
          notes: _cleanText(repayment.notes),
          createdAt: repayment.createdAt,
        ).toMap()..remove('id'),
      );
    });
  }

  Future<void> deleteLoanRepayment(int id) async {
    final db = await AppDatabase.instance();
    await db.transaction((txn) async {
      final rows = await txn.query(
        'loan_repayments',
        where: 'id = ?',
        whereArgs: [id],
      );
      if (rows.isEmpty) return;
      final transactionId = rows.first['general_transaction_id'] as int?;
      await txn.delete('loan_repayments', where: 'id = ?', whereArgs: [id]);
      if (transactionId != null) {
        await txn.delete(
          'general_transactions',
          where: 'id = ?',
          whereArgs: [transactionId],
        );
      }
    });
  }

  static void _validateDay(String date) {
    if (DateTime.tryParse(date) == null || date.length != 10) {
      throw FinanceArgumentError(FinanceErrorCode.invalidDate, {'date': date});
    }
  }

  static String? _cleanText(String? text) {
    final trimmed = text?.trim();
    return trimmed == null || trimmed.isEmpty ? null : trimmed;
  }

  Future<int> insertCategory(GeneralTransactionCategory category) async {
    final db = await AppDatabase.instance();
    return db.insert('general_transaction_categories', _categoryMap(category));
  }

  Future<List<GeneralTransactionCategory>> listCategories() async {
    final db = await AppDatabase.instance();
    final rows = await db.query(
      'general_transaction_categories',
      orderBy: 'nome COLLATE NOCASE',
    );
    return rows.map(GeneralTransactionCategory.fromMap).toList();
  }

  Future<void> updateCategory(GeneralTransactionCategory category) async {
    final db = await AppDatabase.instance();
    await db.update(
      'general_transaction_categories',
      _categoryMap(category)..remove('created_at'),
      where: 'id = ?',
      whereArgs: [category.id],
    );
  }

  /// Apaga a categoria e solta os lançamentos dela (ficam sem categoria; o
  /// banco não aplica a chave estrangeira).
  Future<void> deleteCategory(int id) async {
    final db = await AppDatabase.instance();
    await db.transaction((txn) async {
      await txn.update(
        'general_transactions',
        {'category_id': null},
        where: 'category_id = ?',
        whereArgs: [id],
      );
      await txn.delete(
        'general_transaction_categories',
        where: 'id = ?',
        whereArgs: [id],
      );
    });
  }

  Map<String, Object?> _categoryMap(GeneralTransactionCategory category) {
    final nome = category.nome.trim();
    if (nome.isEmpty) throw FinanceArgumentError(FinanceErrorCode.categoryNameRequired);
    final limit = category.limiteMensal;
    if (limit != null && (!limit.isFinite || limit <= 0)) {
      throw FinanceArgumentError(FinanceErrorCode.categoryLimitMustBePositive);
    }
    return GeneralTransactionCategory(
      nome: nome,
      limiteMensal: limit,
      createdAt: category.createdAt,
    ).toMap()..remove('id');
  }

  /// Mais recentes primeiro; `accountId == null` traz todas as contas.
  Future<List<GeneralTransaction>> listTransactions({int? accountId}) async {
    final db = await AppDatabase.instance();
    final rows = await db.query(
      'general_transactions',
      where: accountId == null ? null : 'bank_account_id = ?',
      whereArgs: accountId == null ? null : [accountId],
      orderBy: 'transaction_date DESC, id DESC',
    );
    return rows.map(GeneralTransaction.fromMap).toList();
  }

  /// Quem gerou cada lançamento de dívida e de empréstimo, por id do lançamento. O vínculo
  /// está nas tabelas filhas (`general_transaction_id`); lançamento sem vínculo não entra.
  Future<Map<int, GeneratedTxInfo>> generatedTransactionInfo() async {
    final db = await AppDatabase.instance();
    final info = <int, GeneratedTxInfo>{};
    final installments = await db.rawQuery('''
      SELECT i.general_transaction_id AS tx_id, l.nome AS nome,
             i.numero_parcela AS numero, l.prazo_meses AS total
      FROM liability_installments i
      JOIN liabilities l ON l.id = i.liability_id
    ''');
    for (final row in installments) {
      info[row['tx_id'] as int] = GeneratedTxInfo(
        kind: GeneratedTxKind.installment,
        name: row['nome'] as String,
        installmentNumber: row['numero'] as int,
        installmentTotal: row['total'] as int,
      );
    }
    final granted = await db.rawQuery('''
      SELECT l.general_transaction_id AS tx_id, b.nome AS nome
      FROM loans l
      JOIN borrowers b ON b.id = l.borrower_id
      WHERE l.general_transaction_id IS NOT NULL
    ''');
    for (final row in granted) {
      info[row['tx_id'] as int] = GeneratedTxInfo(
        kind: GeneratedTxKind.loanGranted,
        name: row['nome'] as String,
      );
    }
    final repaid = await db.rawQuery('''
      SELECT r.general_transaction_id AS tx_id, b.nome AS nome
      FROM loan_repayments r
      JOIN loans l ON l.id = r.loan_id
      JOIN borrowers b ON b.id = l.borrower_id
      WHERE r.general_transaction_id IS NOT NULL
    ''');
    for (final row in repaid) {
      info[row['tx_id'] as int] = GeneratedTxInfo(
        kind: GeneratedTxKind.loanRepayment,
        name: row['nome'] as String,
      );
    }
    return info;
  }

  /// Apagar uma perna de transferência apaga a outra junto (como no desktop).
  /// Parcela de dívida não se apaga solta: o saldo devedor continuaria
  /// contando a amortização dela. Quem apaga é `deleteLiability`.
  Future<void> deleteTransaction(int id) async {
    final db = await AppDatabase.instance();
    final rows = await db.query(
      'general_transactions',
      columns: ['transfer_id', 'transaction_type'],
      where: 'id = ?',
      whereArgs: [id],
    );
    if (rows.isNotEmpty &&
        GeneralTransactionType.fromValue(
          rows.first['transaction_type'] as String,
        ).isGenerated) {
      throw FinanceStateError(FinanceErrorCode.linkedTransactionLocked);
    }
    final transferId = rows.isEmpty ? null : rows.first['transfer_id'] as int?;
    if (transferId == null) {
      await db.delete('general_transactions', where: 'id = ?', whereArgs: [id]);
    } else {
      await db.delete(
        'general_transactions',
        where: 'transfer_id = ?',
        whereArgs: [transferId],
      );
    }
  }

  /// Transferência entre duas contas: duas pernas gravadas juntas, com o mesmo
  /// `transfer_id` (o id da perna de saída). Na mesma moeda entra exatamente o
  /// que saiu; em moedas diferentes o usuário informa os dois valores, como no
  /// extrato, e o spread fica embutido (mesma regra do desktop).
  Future<int> createTransfer({
    required int fromAccountId,
    required int toAccountId,
    required double valorSaida,
    required double valorEntrada,
    required String date,
    String? notes,
    DateTime? now,
  }) async {
    for (final valor in [valorSaida, valorEntrada]) {
      if (!valor.isFinite || valor <= 0) {
        throw FinanceArgumentError(FinanceErrorCode.transferAmountsMustBePositive);
      }
    }
    if (DateTime.tryParse(date) == null || date.length != 10) {
      throw FinanceArgumentError(FinanceErrorCode.invalidDate, {'date': date});
    }
    if (fromAccountId == toAccountId) {
      throw FinanceArgumentError(FinanceErrorCode.transferSameAccount);
    }
    final db = await AppDatabase.instance();
    return db.transaction((txn) async {
      Future<BankAccount> load(int id) async {
        final rows = await txn.query(
          'bank_accounts',
          where: 'id = ?',
          whereArgs: [id],
        );
        if (rows.isEmpty) throw FinanceStateError(FinanceErrorCode.accountNotFound);
        return BankAccount.fromMap(rows.first);
      }

      final from = await load(fromAccountId);
      final to = await load(toAccountId);
      if (from.currency == to.currency &&
          (valorEntrada - valorSaida).abs() > 1e-9) {
        throw FinanceArgumentError(FinanceErrorCode.transferSameCurrencyAmountsDiffer);
      }
      final createdAt = (now ?? DateTime.now()).toIso8601String();
      Map<String, Object?> leg(
        int account,
        GeneralTransactionType type,
        double valor,
      ) => {
        'bank_account_id': account,
        'transaction_type': type.value,
        'valor': valor,
        'transaction_date': date,
        'notes': notes,
        'created_at': createdAt,
      };
      final transferId = await txn.insert(
        'general_transactions',
        leg(fromAccountId, GeneralTransactionType.transferenciaSaida, valorSaida),
      );
      await txn.update(
        'general_transactions',
        {'transfer_id': transferId},
        where: 'id = ?',
        whereArgs: [transferId],
      );
      await txn.insert('general_transactions', {
        ...leg(
          toAccountId,
          GeneralTransactionType.transferenciaEntrada,
          valorEntrada,
        ),
        'transfer_id': transferId,
      });
      return transferId;
    });
  }

  /// Começo da nota do rendimento conciliado (o mesmo texto do desktop).
  static const reconciliationYieldNote = 'Yield by balance reconciliation';

  /// Começo da nota do ajuste de saldo (o mesmo texto do desktop).
  static const reconciliationAdjustmentNote = 'Balance adjustment by reconciliation';

  /// Nome da categoria do rendimento conciliado, criada na primeira vez.
  static const yieldCategoryName = 'Yield';

  Future<ReconciliationPreview> _reconciliationPreviewIn(
    DatabaseExecutor db, {
    required int accountId,
    required double bankBalance,
    required String date,
    required DateTime now,
  }) async {
    final accounts = await db.query('bank_accounts', where: 'id = ?', whereArgs: [accountId]);
    if (accounts.isEmpty) throw FinanceStateError(FinanceErrorCode.accountNotFound);
    final rows = await db.query('general_transactions', where: 'bank_account_id = ?', whereArgs: [accountId]);
    return reconciliationPreview(
      transactions: rows.map(GeneralTransaction.fromMap),
      accountId: accountId,
      bankBalance: bankBalance,
      date: date,
      today: toIsoDay(now),
    );
  }

  /// Calcula a conciliação sem gravar nada: saldo dos lançamentos em [date] e a
  /// diferença contra o saldo que o banco mostra.
  Future<ReconciliationPreview> previewReconciliation({
    required int accountId,
    required double bankBalance,
    required String date,
    DateTime? now,
  }) async {
    final db = await AppDatabase.instance();
    return _reconciliationPreviewIn(
      db,
      accountId: accountId,
      bankBalance: bankBalance,
      date: date,
      now: now ?? DateTime.now(),
    );
  }

  /// Grava a diferença como um lançamento e devolve o id dele. Rendimento
  /// (padrão): só aceita diferença positiva e grava receita na categoria
  /// `Yield` (criada se faltar). Ajuste de saldo: aceita diferença para os dois
  /// lados e grava receita ou despesa sem categoria. Diferença zero é recusada
  /// nos dois modos, então conciliar duas vezes na mesma data não duplica.
  /// Porta de `confirm_balance_reconciliation_in` do desktop.
  Future<int> confirmReconciliation({
    required int accountId,
    required double bankBalance,
    required String date,
    bool adjustment = false,
    DateTime? now,
  }) async {
    final moment = now ?? DateTime.now();
    final db = await AppDatabase.instance();
    return db.transaction((txn) async {
      final preview = await _reconciliationPreviewIn(
        txn,
        accountId: accountId,
        bankBalance: bankBalance,
        date: date,
        now: moment,
      );
      if (preview.difference == 0) {
        throw FinanceArgumentError(FinanceErrorCode.nothingToReconcile);
      }
      if (!adjustment && preview.difference < 0) {
        throw FinanceArgumentError(FinanceErrorCode.bankBalanceBelowLedger);
      }
      final details =
          'bank balance ${preview.bankBalance.toStringAsFixed(2)} on ${preview.date}, '
          'ledger ${preview.ledgerBalance.toStringAsFixed(2)}';
      final isIncome = preview.difference > 0;
      return txn.insert('general_transactions', {
        'bank_account_id': accountId,
        'transaction_type': (isIncome ? GeneralTransactionType.receita : GeneralTransactionType.despesa).value,
        'category_id': adjustment ? null : await _ensureYieldCategory(txn, moment),
        'valor': preview.difference.abs(),
        'transaction_date': preview.date,
        'notes': '${adjustment ? reconciliationAdjustmentNote : reconciliationYieldNote}: $details',
        'created_at': moment.toIso8601String(),
      });
    });
  }

  Future<int> _ensureYieldCategory(DatabaseExecutor db, DateTime now) =>
      _ensureCategory(db, yieldCategoryName, now);

  /// Procura a categoria pelo nome exato e a cria se faltar (a mesma regra do desktop).
  Future<int> _ensureCategory(DatabaseExecutor db, String name, DateTime now) async {
    final rows = await db.query('general_transaction_categories', where: 'nome = ?', whereArgs: [name]);
    if (rows.isNotEmpty) return rows.first['id'] as int;
    return db.insert('general_transaction_categories', {
      'nome': name,
      'limite_mensal': null,
      'created_at': now.toIso8601String(),
    });
  }

  /// Nome da categoria dos impostos retidos (IOF e IR), criada na primeira estimativa lançada.
  static const taxesCategoryName = 'Taxes';

  /// Começo da nota da receita de uma estimativa lançada (o mesmo texto do desktop).
  static const estimatedYieldNote = 'Yield CDB Porquinho';

  /// Fluxos da conta para o modelo do Porquinho: as pernas de transferência, em ordem de data e, no
  /// mesmo dia, as aplicações (entrada) antes dos resgates (saída).
  Future<List<Leg>> porquinhoLegs(int accountId) async {
    final db = await AppDatabase.instance();
    final rows = await db.query(
      'general_transactions',
      where: 'bank_account_id = ? AND transaction_type IN (?, ?)',
      whereArgs: [
        accountId,
        GeneralTransactionType.transferenciaEntrada.value,
        GeneralTransactionType.transferenciaSaida.value,
      ],
      orderBy: 'transaction_date, id',
    );
    final legs = rows.map((row) {
      final parsed = DateTime.parse(row['transaction_date'] as String);
      return Leg(
        date: DateTime.utc(parsed.year, parsed.month, parsed.day),
        kind: row['transaction_type'] == GeneralTransactionType.transferenciaEntrada.value
            ? LegKind.application
            : LegKind.redemption,
        amount: (row['valor'] as num).toDouble(),
      );
    }).toList();
    // `List.sort` não é estável: o índice original desempata.
    final order = {for (var i = 0; i < legs.length; i++) legs[i]: i};
    legs.sort((a, b) {
      final byDate = a.date.compareTo(b.date);
      if (byDate != 0) return byDate;
      final byKind = (a.kind == LegKind.redemption ? 1 : 0) - (b.kind == LegKind.redemption ? 1 : 0);
      return byKind != 0 ? byKind : order[a]!.compareTo(order[b]!);
    });
    return legs;
  }

  /// Dias (`YYYY-MM-DD`) da conta que já têm rendimento: toda receita dela, menos as da conciliação e do
  /// ajuste de saldo, que são de outro tipo (porta de `days_with_yield`).
  Future<Set<String>> daysWithYield(int accountId) async {
    final db = await AppDatabase.instance();
    final rows = await db.query(
      'general_transactions',
      where: 'bank_account_id = ? AND transaction_type = ?',
      whereArgs: [accountId, GeneralTransactionType.receita.value],
    );
    return {
      for (final row in rows)
        if (!((row['notes'] as String?) ?? '').startsWith(reconciliationYieldNote) &&
            !((row['notes'] as String?) ?? '').startsWith(reconciliationAdjustmentNote))
          row['transaction_date'] as String,
    };
  }

  /// Lança as estimativas aprovadas, como a aprovação do desktop (`confirm_yield_pending_in`): receita do
  /// rendimento bruto em `Yield` e uma despesa para o IOF e outra para o IR em `Taxes` (só as maiores que
  /// zero), no dia do resgate. O saldo da conta sobe pelo rendimento líquido. Tudo ou nada: uma estimativa
  /// inválida desfaz as outras. Devolve quantos lançamentos gravou.
  Future<int> bookYieldEstimates({
    required int accountId,
    required List<RedemptionEstimate> estimates,
    DateTime? now,
  }) async {
    final moment = now ?? DateTime.now();
    final db = await AppDatabase.instance();
    return db.transaction((txn) async {
      final accounts = await txn.query('bank_accounts', where: 'id = ?', whereArgs: [accountId]);
      if (accounts.isEmpty) throw FinanceStateError(FinanceErrorCode.accountNotFound);
      var rows = 0;
      for (final estimate in estimates) {
        if (!estimate.grossYield.isFinite || estimate.grossYield <= 0) {
          throw FinanceArgumentError(FinanceErrorCode.amountMustBePositive);
        }
        final day = toIsoDay(estimate.date);
        Future<void> insert(GeneralTransactionType type, int categoryId, double valor, String notes) async {
          await txn.insert('general_transactions', {
            'bank_account_id': accountId,
            'transaction_type': type.value,
            'category_id': categoryId,
            'valor': valor,
            'transaction_date': day,
            'notes': notes,
            'created_at': moment.toIso8601String(),
          });
          rows++;
        }

        final yieldCategory = await _ensureYieldCategory(txn, moment);
        await insert(GeneralTransactionType.receita, yieldCategory, estimate.grossYield, '$estimatedYieldNote (gross)');
        if (estimate.iof > 0 || estimate.ir > 0) {
          final taxes = await _ensureCategory(txn, taxesCategoryName, moment);
          if (estimate.iof > 0) {
            await insert(GeneralTransactionType.despesa, taxes, estimate.iof, 'IOF withheld on CDB Porquinho yield');
          }
          if (estimate.ir > 0) {
            await insert(
              GeneralTransactionType.despesa,
              taxes,
              estimate.ir,
              'IR withheld on CDB Porquinho yield (${(porquinhoIrRate * 100).toStringAsFixed(1)}%)',
            );
          }
        }
      }
      return rows;
    });
  }

  /// Moeda base do patrimônio; o real quando nunca foi escolhida.
  Future<String> getBaseCurrency() async {
    final db = await AppDatabase.instance();
    final rows = await db.query(
      'app_settings',
      where: 'key = ?',
      whereArgs: [_baseCurrencyKey],
    );
    if (rows.isEmpty) return defaultBaseCurrency;
    return validateCurrency(rows.first['value'] as String) ??
        defaultBaseCurrency;
  }

  Future<void> setBaseCurrency(String code) async {
    final currency = validateCurrency(code);
    if (currency == null) {
      throw FinanceArgumentError(FinanceErrorCode.unknownCurrency, {'currency': code});
    }
    final db = await AppDatabase.instance();
    await db.insert('app_settings', {
      'key': _baseCurrencyKey,
      'value': currency,
    }, conflictAlgorithm: ConflictAlgorithm.replace);
  }
}
