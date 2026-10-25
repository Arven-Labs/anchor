import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/domain/fx_history.dart';
import 'package:anchor_mobile/domain/general_finance.dart';
import 'package:anchor_mobile/models/asset.dart';
import 'package:anchor_mobile/models/bank_account.dart';
import 'package:anchor_mobile/models/general_transaction.dart';
import 'package:anchor_mobile/models/general_transaction_category.dart';
import 'package:anchor_mobile/screens/financas_categories_screen.dart';
import 'package:anchor_mobile/models/position.dart';
import 'package:anchor_mobile/screens/financas_screen.dart';
import 'package:anchor_mobile/services/general_finance_repository.dart';
import 'package:anchor_mobile/services/live_positions.dart';
import 'package:anchor_mobile/services/quote_exceptions.dart';

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
  // A tela só fica pronta quando o resumo (patrimônio) aparece.
  Future<void> pumpScreen(
    WidgetTester tester, {
    Future<FxHistory> Function(String currency)? fetchFx,
    List<LivePositionRow> positions = const [],
  }) async {
    await tester.pumpWidget(
      MaterialApp(
        home: FinancasScreen(
          fetchFx: fetchFx ?? (_) async => throw QuoteNotFoundException('sem rede nos testes'),
          loadPositions: () async => positions,
        ),
      ),
    );
    for (var i = 0; i < 200; i++) {
      await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
      await tester.pump();
      if (find.byKey(const Key('financas-net-worth')).evaluate().isNotEmpty) return;
    }
    fail('a tela não terminou de carregar');
  }

  Text textOf(WidgetTester tester, String key) => tester.widget<Text>(find.byKey(Key(key)));

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

    expect(textOf(tester, 'financas-balance-$usd').data, 'USD 70,00');
    expect(textOf(tester, 'financas-balance-$brl').data, 'BRL -12,50');
    expect(find.text('Chase'), findsOneWidget);
  });

  testWidgets('o patrimônio soma caixa e carteira na moeda base, e cada conta mostra o equivalente', (tester) async {
    late int usd;
    await tester.runAsync(() async {
      usd = await account('Chase', 'USD');
      final brl = await account('Nubank', 'BRL');
      await add(usd, GeneralTransactionType.receita, 100, '2026-01-02');
      await add(usd, GeneralTransactionType.despesa, 30, '2026-01-05');
      await add(brl, GeneralTransactionType.receita, 50, '2026-01-06');
    });

    await pumpScreen(
      tester,
      fetchFx: (currency) async => FxHistory([(DateTime.now(), currency == 'USD' ? 5.0 : 1.0)]),
      positions: [
        LivePositionRow(
          Position(
            asset: Asset(
              id: 1,
              ticker: 'AAPL',
              name: 'Apple',
              assetClass: AssetClass.acaoInternacional,
              currency: 'USD',
              createdAt: DateTime(2026, 1, 1),
            ),
            netQuantity: 2,
            averageBuyPrice: null,
          ),
          10, // US$ 20 de carteira
        ),
      ],
    );

    // Caixa: US$ 70 × 5 + R$ 50 = R$ 400. Carteira: US$ 20 × 5 = R$ 100.
    expect(textOf(tester, 'financas-cash').data, 'Caixa BRL 400,00');
    expect(textOf(tester, 'financas-portfolio').data, 'Carteira BRL 100,00');
    expect(textOf(tester, 'financas-net-worth').data, 'BRL 500,00');
    expect(textOf(tester, 'financas-equivalent-$usd').data, '≈ BRL 350,00');
    expect(find.byKey(const Key('financas-missing')), findsNothing);
    expect(find.byKey(const Key('financas-fx-banner')), findsNothing);
  });

  testWidgets('conta sem câmbio fica fora do patrimônio, é contada e o aviso nomeia a moeda', (tester) async {
    late int pyg;
    await tester.runAsync(() async {
      pyg = await account('Banco', 'PYG');
      final brl = await account('Nubank', 'BRL');
      await add(pyg, GeneralTransactionType.receita, 1000000, '2026-01-02');
      await add(brl, GeneralTransactionType.receita, 50, '2026-01-06');
    });

    await pumpScreen(tester); // o câmbio de teste sempre diz "sem dados"

    expect(textOf(tester, 'financas-cash').data, 'Caixa BRL 50,00');
    expect(textOf(tester, 'financas-missing').data, '1 item fora da soma por falta de câmbio ou cotação.');
    expect(textOf(tester, 'financas-fx-banner').data, fxUnavailableBanner({'PYG'}));
    expect(textOf(tester, 'financas-equivalent-$pyg').data, 'sem câmbio PYG/BRL');
    expect(textOf(tester, 'financas-balance-$pyg').data, 'PYG 1.000.000,00');
  });

  testWidgets('categorias: gasto do mês contra o limite, com aviso de estouro', (tester) async {
    late int cartao;
    late int mercado;
    late int lazer;
    final month = toIsoDay(DateTime.now()).substring(0, 7);
    await tester.runAsync(() async {
      cartao = await account('Nubank', 'BRL');
      mercado = await repository.insertCategory(
        GeneralTransactionCategory(
          nome: 'Mercado',
          limiteMensal: 500,
          createdAt: DateTime(2026, 1, 1),
        ),
      );
      lazer = await repository.insertCategory(
        GeneralTransactionCategory(
          nome: 'Lazer',
          limiteMensal: 1000,
          createdAt: DateTime(2026, 1, 1),
        ),
      );
      Future<void> spend(int category, double valor) => repository.insertTransaction(
        GeneralTransaction(
          bankAccountId: cartao,
          type: GeneralTransactionType.despesa,
          categoryId: category,
          valor: valor,
          date: '$month-01',
          createdAt: DateTime(2026, 1, 1),
        ),
      );
      await spend(mercado, 350);
      await spend(mercado, 250);
      await spend(lazer, 100);
    });

    await tester.pumpWidget(
      MaterialApp(
        home: FinancasCategoriesScreen(
          base: 'BRL',
          ratesToBrl: const {},
          accounts: [
            BankAccount(
              id: cartao,
              nome: 'Nubank',
              titular: 'Eu',
              currency: 'BRL',
              createdAt: DateTime(2026, 1, 1),
            ),
          ],
        ),
      ),
    );
    for (var i = 0; i < 200; i++) {
      await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
      await tester.pump();
      if (find.byKey(Key('financas-category-spent-$mercado')).evaluate().isNotEmpty) break;
    }

    expect(textOf(tester, 'financas-category-spent-$mercado').data, 'BRL 600,00 de BRL 500,00');
    expect(textOf(tester, 'financas-category-over-$mercado').data, 'Estourou em BRL 100,00');
    expect(textOf(tester, 'financas-category-spent-$lazer').data, 'BRL 100,00 de BRL 1.000,00');
    expect(find.byKey(Key('financas-category-over-$lazer')), findsNothing);
  });

  testWidgets('o botão Categorias abre a tela de categorias', (tester) async {
    await pumpScreen(tester);

    await tester.tap(find.byTooltip('Categorias'));
    for (var i = 0; i < 200; i++) {
      await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
      await tester.pump();
      if (find.text('Nenhuma categoria cadastrada ainda.').evaluate().isNotEmpty) break;
    }

    expect(find.text('Nenhuma categoria cadastrada ainda.'), findsOneWidget);
  });

  test('textos do aviso: moedas em ordem e plural dos itens fora', () {
    expect(fxUnavailableBanner({'PYG', 'ARS'}), contains('ARS, PYG'));
    expect(missingItemsNote(0), isNull);
    expect(missingItemsNote(3), '3 itens fora da soma por falta de câmbio ou cotação.');
  });
}
