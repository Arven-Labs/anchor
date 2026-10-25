import 'package:sqflite/sqflite.dart' show ConflictAlgorithm;

import '../data/db.dart';
import '../domain/currency_catalog.dart';
import '../models/bank_account.dart';
import '../models/general_transaction.dart';
import '../models/general_transaction_category.dart';
import '../models/liability.dart';
import '../domain/general_finance.dart' show toIsoDay;
import '../domain/liability.dart';

/// Contas, lançamentos e a moeda base de Finanças Gerais. Cálculo de saldo e
/// de patrimônio fica em `domain/general_finance.dart`, puro.
class GeneralFinanceRepository {
  static const _baseCurrencyKey = 'base_currency';

  Future<int> insertAccount(BankAccount account) async {
    final currency = validateCurrency(account.currency);
    if (currency == null) {
      throw ArgumentError('Moeda fora do catálogo: ${account.currency}');
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
      throw ArgumentError('Moeda fora do catálogo: ${account.currency}');
    }
    final db = await AppDatabase.instance();
    final rows = await db.query(
      'bank_accounts',
      where: 'id = ?',
      whereArgs: [account.id],
    );
    if (rows.isEmpty) throw StateError('Conta não encontrada');
    final current = BankAccount.fromMap(rows.first);
    if (current.currency != currency && await accountInUse(account.id!)) {
      throw StateError('A moeda não muda numa conta que já tem lançamentos');
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
      throw ArgumentError('O valor deve ser maior que zero');
    }
    if (DateTime.tryParse(tx.date) == null || tx.date.length != 10) {
      throw ArgumentError('Data inválida: ${tx.date}');
    }
    if (!GeneralTransactionType.manual.contains(tx.type)) {
      throw ArgumentError(
        'Só receita e despesa são lançadas à mão; transferência nasce em '
        'createTransfer e parcela de dívida em createLiability',
      );
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
    if (nome.isEmpty) throw ArgumentError('Informe o nome da dívida');
    final start = DateTime.tryParse(liability.dataInicio);
    if (start == null || liability.dataInicio.length != 10) {
      throw ArgumentError('Data inválida: ${liability.dataInicio}');
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
      if (accounts.isEmpty) throw StateError('Conta não encontrada');
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
          'notes':
              '$nome — parcela ${installment.numeroParcela}/${liability.prazoMeses}',
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
    if (nome.isEmpty) throw ArgumentError('Informe o nome da categoria');
    final limit = category.limiteMensal;
    if (limit != null && (!limit.isFinite || limit <= 0)) {
      throw ArgumentError('O limite mensal deve ser maior que zero');
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
        rows.first['transaction_type'] ==
            GeneralTransactionType.parcelaDivida.value) {
      throw StateError('Parcela de dívida só se apaga junto com a dívida');
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
        throw ArgumentError('Os valores devem ser maiores que zero');
      }
    }
    if (DateTime.tryParse(date) == null || date.length != 10) {
      throw ArgumentError('Data inválida: $date');
    }
    if (fromAccountId == toAccountId) {
      throw ArgumentError('Escolha duas contas diferentes');
    }
    final db = await AppDatabase.instance();
    return db.transaction((txn) async {
      Future<BankAccount> load(int id) async {
        final rows = await txn.query(
          'bank_accounts',
          where: 'id = ?',
          whereArgs: [id],
        );
        if (rows.isEmpty) throw StateError('Conta não encontrada');
        return BankAccount.fromMap(rows.first);
      }

      final from = await load(fromAccountId);
      final to = await load(toAccountId);
      if (from.currency == to.currency &&
          (valorEntrada - valorSaida).abs() > 1e-9) {
        throw ArgumentError(
          'Contas na mesma moeda recebem exatamente o que saiu',
        );
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
      throw ArgumentError('Moeda fora do catálogo: $code');
    }
    final db = await AppDatabase.instance();
    await db.insert('app_settings', {
      'key': _baseCurrencyKey,
      'value': currency,
    }, conflictAlgorithm: ConflictAlgorithm.replace);
  }
}
