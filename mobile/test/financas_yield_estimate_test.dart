import 'dart:io';

import 'package:material_ui/material_ui.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/domain/general_finance.dart';
import 'package:anchor_mobile/domain/porquinho_yield.dart';
import 'package:anchor_mobile/models/bank_account.dart';
import 'package:anchor_mobile/models/general_transaction.dart';
import 'package:anchor_mobile/screens/financas_yield_estimate_screen.dart';
import 'package:anchor_mobile/services/general_finance_repository.dart';
import 'package:anchor_mobile/services/quote_exceptions.dart';

import 'support/localized_app.dart';

// A tela "Estimar rendimento" do Porquinho montada de verdade (SQLite ffi, Selic de mentira).
// A lista de resgates, a caixa de seleção de cada um e o botão de lançar substituem a fila de
// Pendências do desktop.
void main() {
  final repository = GeneralFinanceRepository();
  late Directory dir;
  late BankAccount porquinho;
  late int corrente;

  setUp(() async {
    dir = await Directory.systemTemp.createTemp('anchor-yield-screen-');
    await databaseFactory.setDatabasesPath(dir.path);
    await AppDatabase.resetForTest();
  });

  tearDown(() async {
    await AppDatabase.resetForTest();
    await dir.delete(recursive: true);
  });

  Future<void> seed({bool withRedemption = true}) async {
    BankAccount account(String nome) =>
        BankAccount(nome: nome, titular: 'Eu', currency: 'BRL', createdAt: DateTime(2026, 1, 1));
    final porquinhoId = await repository.insertAccount(account('Porquinho'));
    corrente = await repository.insertAccount(account('Corrente'));
    porquinho = (await repository.listAccounts()).firstWhere((a) => a.id == porquinhoId);
    await repository.createTransfer(
      fromAccountId: corrente,
      toAccountId: porquinhoId,
      valorSaida: 1000,
      valorEntrada: 1000,
      date: '2025-01-02',
    );
    if (withRedemption) {
      await repository.createTransfer(
        fromAccountId: porquinhoId,
        toAccountId: corrente,
        valorSaida: 1012,
        valorEntrada: 1012,
        date: '2025-02-03',
      );
    }
  }

  Future<List<SelicPoint>> flatSelic(DateTime from) async => [SelicPoint(DateTime.utc(2024, 1, 1), 14.0)];

  Future<void> pumpScreen(
    WidgetTester tester, {
    Future<List<SelicPoint>> Function(DateTime from)? fetchSelic,
  }) async {
    await tester.pumpWidget(
      localizedApp(
        FinancasYieldEstimateScreen(account: porquinho, fetchSelic: fetchSelic ?? flatSelic),
      ),
    );
    await tester.pumpAndSettle();
  }

  // O SQLite ffi responde em tempo real, fora do relógio falso do teste.
  Future<void> waitFor(WidgetTester tester, Finder finder) async {
    for (var i = 0; i < 200; i++) {
      await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
      await tester.pump(const Duration(milliseconds: 50));
      if (finder.evaluate().isNotEmpty) break;
    }
    await tester.pumpAndSettle();
  }

  Future<void> estimate(WidgetTester tester) async {
    await tester.tap(find.byKey(const Key('financas-yield-estimate')));
    await tester.pump();
  }

  Future<List<GeneralTransaction>> ledger() => repository.listTransactions(accountId: porquinho.id);

  testWidgets('a % do CDI começa em 100 e o botão Lançar começa desligado', (tester) async {
    await tester.runAsync(seed);
    await pumpScreen(tester);

    expect(tester.widget<TextField>(find.byKey(const Key('financas-yield-pct'))).controller!.text, '100');
    expect(find.byKey(const Key('financas-yield-book')), findsNothing);
  });

  testWidgets('estimar lista um resgate marcado; lançar grava o rendimento e o saldo sobe pelo líquido', (tester) async {
    await tester.runAsync(seed);
    await pumpScreen(tester);
    final before = await tester.runAsync(() async => computeBalances(await ledger(), '2099-01-01')[porquinho.id]!);

    await estimate(tester);
    await waitFor(tester, find.byKey(const Key('financas-yield-row-0')));

    final row = tester.widget<CheckboxListTile>(find.byKey(const Key('financas-yield-row-0')));
    expect(row.value, isTrue);
    await tester.tap(find.byKey(const Key('financas-yield-book')));
    await waitFor(tester, find.byKey(const Key('financas-yield-booked')));

    final rows = await tester.runAsync(ledger);
    final after = await tester.runAsync(() async => computeBalances(await ledger(), '2099-01-01')[porquinho.id]!);
    final yieldRow = rows!.singleWhere((t) => t.type == GeneralTransactionType.receita);
    expect(yieldRow.date, '2025-02-03');
    expect(yieldRow.valor, greaterThan(0));
    expect(after! - before!, greaterThan(0));
    expect(after - before, lessThan(yieldRow.valor));
  });

  testWidgets('depois de lançar, estimar de novo não propõe nada: o dia já tem rendimento', (tester) async {
    await tester.runAsync(seed);
    await pumpScreen(tester);
    await estimate(tester);
    await waitFor(tester, find.byKey(const Key('financas-yield-row-0')));
    await tester.tap(find.byKey(const Key('financas-yield-book')));
    await waitFor(tester, find.byKey(const Key('financas-yield-booked')));

    await estimate(tester);
    await waitFor(tester, find.byKey(const Key('financas-yield-empty')));

    expect(find.byKey(const Key('financas-yield-row-0')), findsNothing);
    final receitas = (await tester.runAsync(ledger))!.where((t) => t.type == GeneralTransactionType.receita);
    expect(receitas, hasLength(1));
  });

  testWidgets('desmarcar o resgate desliga o botão e nada é gravado', (tester) async {
    await tester.runAsync(seed);
    await pumpScreen(tester);
    await estimate(tester);
    await waitFor(tester, find.byKey(const Key('financas-yield-row-0')));

    await tester.tap(find.byKey(const Key('financas-yield-row-0')));
    await tester.pump();

    expect(tester.widget<FilledButton>(find.byKey(const Key('financas-yield-book'))).onPressed, isNull);
    expect((await tester.runAsync(ledger))!.where((t) => t.type == GeneralTransactionType.receita), isEmpty);
  });

  testWidgets('conta sem resgate: avisa que não há o que estimar', (tester) async {
    await tester.runAsync(() => seed(withRedemption: false));
    await pumpScreen(tester);

    await estimate(tester);
    await waitFor(tester, find.byKey(const Key('financas-yield-empty')));

    expect(find.byKey(const Key('financas-yield-row-0')), findsNothing);
  });

  testWidgets('Selic fora do ar: mostra o erro e não grava nada', (tester) async {
    await tester.runAsync(seed);
    await pumpScreen(tester, fetchSelic: (_) async => throw QuoteUnavailableException('bcb'));

    await estimate(tester);
    await waitFor(tester, find.byKey(const Key('financas-yield-error')));

    expect(find.byKey(const Key('financas-yield-row-0')), findsNothing);
    expect((await tester.runAsync(ledger))!.where((t) => t.type == GeneralTransactionType.receita), isEmpty);
  });

  testWidgets('% do CDI fora de 0 a 200: mostra o erro e não consulta a Selic', (tester) async {
    await tester.runAsync(seed);
    var calls = 0;
    await pumpScreen(
      tester,
      fetchSelic: (from) async {
        calls++;
        return flatSelic(from);
      },
    );

    await tester.enterText(find.byKey(const Key('financas-yield-pct')), '250');
    await estimate(tester);
    await tester.pumpAndSettle();

    expect(find.byKey(const Key('financas-yield-error')), findsOneWidget);
    expect(calls, 0);
  });
}
