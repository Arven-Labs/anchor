import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/domain/general_finance.dart';
import 'package:anchor_mobile/models/bank_account.dart';
import 'package:anchor_mobile/models/general_transaction.dart';
import 'package:anchor_mobile/screens/financas_screen.dart';
import 'package:anchor_mobile/services/general_finance_repository.dart';

// A aba de Finanças montada de verdade (SQLite ffi, sem rede).
void main() {
  final repository = GeneralFinanceRepository();
  late Directory dir;

  setUp(() async {
    dir = await Directory.systemTemp.createTemp('anchor-financas-screen-');
    await databaseFactory.setDatabasesPath(dir.path);
    await AppDatabase.resetForTest();
  });

  tearDown(() async {
    await AppDatabase.resetForTest();
    await dir.delete(recursive: true);
  });

  Future<int> account(String nome, String currency) => repository
      .insertAccount(
        BankAccount(
          nome: nome,
          titular: 'Eu',
          currency: currency,
          createdAt: DateTime(2026, 1, 1),
        ),
      );

  Future<void> add(int id, GeneralTransactionType type, double valor, String date) =>
      repository.insertTransaction(
        GeneralTransaction(
          bankAccountId: id,
          type: type,
          valor: valor,
          date: date,
          createdAt: DateTime(2026, 1, 1),
        ),
      );

  group('helpers de texto', () {
    test('formatIsoDay vira dd/MM/yyyy e deixa passar o que não é data ISO', () {
      expect(formatIsoDay('2026-03-09'), '09/03/2026');
      expect(formatIsoDay('9/3/2026'), '9/3/2026');
    });

    test('toIsoDay completa com zeros', () {
      expect(toIsoDay(DateTime(2026, 3, 9)), '2026-03-09');
      expect(toIsoDay(DateTime(987, 1, 2)), '0987-01-02');
    });

    test('parseAmount aceita vírgula e recusa zero, negativo e lixo', () {
      expect(parseAmount(' 12,5 '), 12.5);
      expect(parseAmount('1000'), 1000);
      expect(parseAmount('0'), isNull);
      expect(parseAmount('-3'), isNull);
      expect(parseAmount('abc'), isNull);
      expect(parseAmount('NaN'), isNull);
    });
  });

  // O SQLite ffi responde em tempo real, fora do relógio falso do teste: sonda até o
  // indicador de carregamento sair, em vez de esperar um tempo fixo.
  Future<void> pumpScreen(WidgetTester tester) async {
    await tester.pumpWidget(const MaterialApp(home: FinancasScreen()));
    for (var i = 0; i < 200; i++) {
      await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
      await tester.pump();
      if (find.byType(CircularProgressIndicator).evaluate().isEmpty) return;
    }
    fail('a tela não terminou de carregar');
  }

  testWidgets('sem conta, a aba avisa que não há nada cadastrado', (tester) async {
    await pumpScreen(tester);

    expect(find.text('Nenhuma conta cadastrada ainda.'), findsOneWidget);
  });

  testWidgets('cada conta mostra o saldo na moeda dela, sem lançamento futuro', (tester) async {
    late int usd;
    late int brl;
    await tester.runAsync(() async {
      usd = await account('Chase', 'USD');
      brl = await account('Nubank', 'BRL');
      await add(usd, GeneralTransactionType.receita, 100, '2026-01-02');
      await add(usd, GeneralTransactionType.despesa, 30, '2026-01-05');
      await add(usd, GeneralTransactionType.receita, 999, '2999-01-01'); // futuro
      await add(brl, GeneralTransactionType.despesa, 12.5, '2026-01-06');
    });
    await pumpScreen(tester);

    expect(tester.widget<Text>(find.byKey(Key('financas-balance-$usd'))).data, 'USD 70,00');
    expect(tester.widget<Text>(find.byKey(Key('financas-balance-$brl'))).data, 'BRL -12,50');
    expect(find.text('Chase'), findsOneWidget);
  });
}
