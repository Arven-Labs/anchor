import 'package:sqflite/sqflite.dart' show ConflictAlgorithm;

import '../data/db.dart';
import '../domain/currency_catalog.dart';
import '../models/bank_account.dart';
import '../models/general_transaction.dart';
import '../models/general_transaction_category.dart';

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
      await txn.delete(
        'general_transactions',
        where: 'bank_account_id = ?',
        whereArgs: [id],
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
    final db = await AppDatabase.instance();
    final map = tx.toMap()..remove('id');
    // Categoria é de despesa: receita não entra em orçamento.
    if (tx.type != GeneralTransactionType.despesa) map['category_id'] = null;
    return db.insert('general_transactions', map);
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

  Future<void> deleteTransaction(int id) async {
    final db = await AppDatabase.instance();
    await db.delete('general_transactions', where: 'id = ?', whereArgs: [id]);
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
