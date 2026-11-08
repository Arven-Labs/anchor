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
import 'package:anchor_mobile/domain/liability.dart';
import 'package:anchor_mobile/models/liability.dart';
import 'package:anchor_mobile/models/loan.dart';
import 'package:anchor_mobile/screens/financas_loans_screen.dart';
import 'package:anchor_mobile/screens/financas_categories_screen.dart';
import 'package:anchor_mobile/screens/financas_liabilities_screen.dart';
import 'package:anchor_mobile/models/position.dart';
import 'package:anchor_mobile/screens/financas_screen.dart';
import 'package:anchor_mobile/screens/financas_transactions_screen.dart';
import 'package:anchor_mobile/services/general_finance_repository.dart';
import 'package:anchor_mobile/services/live_positions.dart';
import 'package:anchor_mobile/services/quote_exceptions.dart';
import 'package:anchor_mobile/l10n/app_localizations.dart';

import 'support/localized_app.dart';

// A aba de Finanças montada de verdade (SQLite ffi, sem rede).
void main() {
  final repository = GeneralFinanceRepository();
  late Directory dir;

  late AppLocalizations en;
  late AppLocalizations pt;

  setUpAll(() async {
    en = await AppLocalizations.delegate.load(const Locale('en'));
    pt = await AppLocalizations.delegate.load(const Locale('pt'));
  });

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
  // Idioma fixado em pt: as asserções são o texto que o app já mostrava (ADR 0001).
  Future<void> pumpScreen(
    WidgetTester tester, {
    Future<FxHistory> Function(String currency)? fetchFx,
    List<LivePositionRow> positions = const [],
    String locale = 'pt',
  }) async {
    await tester.pumpWidget(
      localizedApp(
        FinancasScreen(
          fetchFx: fetchFx ?? (_) async => throw QuoteNotFoundException('sem rede nos testes'),
          loadPositions: () async => positions,
        ),
        locale: locale,
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

  // Espera algo que depende do SQLite real (fora do relógio falso) aparecer e deixa as
  // animações de rota terminarem.
  Future<void> waitFor(WidgetTester tester, Finder finder) async {
    for (var i = 0; i < 200; i++) {
      await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
      await tester.pump(const Duration(milliseconds: 50));
      if (finder.evaluate().isNotEmpty) break;
    }
    await tester.pumpAndSettle();
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
    expect(textOf(tester, 'financas-fx-banner').data, fxUnavailableBanner(pt, {'PYG'}));
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
      localizedApp(
        FinancasCategoriesScreen(
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

  testWidgets('transferir move o saldo de uma conta para a outra', (tester) async {
    late int a;
    late int b;
    await tester.runAsync(() async {
      a = await account('Alfa', 'BRL');
      b = await account('Beta', 'BRL');
      await add(a, GeneralTransactionType.receita, 500, '2026-01-02');
    });
    await pumpScreen(tester);

    await tester.tap(find.byTooltip('Transferir'));
    await tester.pumpAndSettle();
    await tester.enterText(find.byKey(const Key('financas-transfer-out')), '120,5');
    await tester.tap(find.widgetWithText(FilledButton, 'Transferir'));
    await tester.pumpAndSettle();
    for (var i = 0; i < 200; i++) {
      await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
      await tester.pump();
      // Também espera o resumo: o spinner some quando a recarga termina.
      if (find.text('BRL 120,50').evaluate().isNotEmpty &&
          find.byType(CircularProgressIndicator).evaluate().isEmpty) {
        break;
      }
    }

    expect(textOf(tester, 'financas-balance-$a').data, 'BRL 379,50');
    expect(textOf(tester, 'financas-balance-$b').data, 'BRL 120,50');
  });

  group('conciliar saldo', () {
    Future<int> openReconcile(WidgetTester tester) async {
      late int id;
      await tester.runAsync(() async {
        id = await account('Porquinho', 'BRL');
        await add(id, GeneralTransactionType.receita, 500, '2026-01-02');
      });
      await pumpScreen(tester);
      await tester.tap(find.byType(PopupMenuButton<String>));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Conciliar saldo'));
      // O diálogo só abre depois de ler os lançamentos no SQLite (I/O real, fora do relógio falso).
      for (var i = 0; i < 200; i++) {
        await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
        await tester.pump();
        if (find.byKey(const Key('financas-reconcile-balance')).evaluate().isNotEmpty) break;
      }
      await tester.pumpAndSettle();
      return id;
    }

    Future<void> waitBalance(WidgetTester tester, int id, String text) async {
      for (var i = 0; i < 200; i++) {
        await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
        await tester.pump();
        if (find.byType(CircularProgressIndicator).evaluate().isEmpty &&
            textOf(tester, 'financas-balance-$id').data == text) {
          break;
        }
      }
    }

    bool confirmEnabled(WidgetTester tester) =>
        tester.widget<FilledButton>(find.byKey(const Key('financas-reconcile-confirm'))).onPressed != null;

    testWidgets('rendimento: banco 512,50 contra 500,00 mostra a prévia e lança 12,50', (tester) async {
      final id = await openReconcile(tester);

      await tester.enterText(find.byKey(const Key('financas-reconcile-balance')), '512,50');
      await tester.pump();

      expect(textOf(tester, 'financas-reconcile-ledger').data, 'Saldo dos lançamentos nessa data: BRL 500,00');
      expect(textOf(tester, 'financas-reconcile-difference').data, 'Rendimento a lançar: BRL 12,50');
      expect(confirmEnabled(tester), isTrue);

      await tester.tap(find.byKey(const Key('financas-reconcile-confirm')));
      await tester.pumpAndSettle();
      await waitBalance(tester, id, 'BRL 512,50');

      expect(textOf(tester, 'financas-balance-$id').data, 'BRL 512,50');
    });

    testWidgets('rendimento com banco 480,00 contra 500,00 avisa e não deixa lançar', (tester) async {
      await openReconcile(tester);

      await tester.enterText(find.byKey(const Key('financas-reconcile-balance')), '480');
      await tester.pump();

      expect(find.byKey(const Key('financas-reconcile-negative')), findsOneWidget);
      expect(confirmEnabled(tester), isFalse);
    });

    testWidgets('ajuste de saldo aceita a diferença negativa e baixa o saldo para 480,00', (tester) async {
      final id = await openReconcile(tester);

      await tester.enterText(find.byKey(const Key('financas-reconcile-balance')), '480');
      await tester.tap(find.byKey(const Key('financas-reconcile-adjustment')));
      await tester.pump();

      expect(find.byKey(const Key('financas-reconcile-negative')), findsNothing);
      expect(find.byKey(const Key('financas-reconcile-difference')), findsOneWidget);
      expect(confirmEnabled(tester), isTrue);

      await tester.tap(find.byKey(const Key('financas-reconcile-confirm')));
      await tester.pumpAndSettle();
      await waitBalance(tester, id, 'BRL 480,00');

      expect(textOf(tester, 'financas-balance-$id').data, 'BRL 480,00');
    });

    testWidgets('sem digitar o saldo não há prévia e o botão fica desligado', (tester) async {
      await openReconcile(tester);

      expect(find.byKey(const Key('financas-reconcile-difference')), findsNothing);
      expect(confirmEnabled(tester), isFalse);
    });
  });

  testWidgets('transferir com uma conta só avisa e não abre o diálogo', (tester) async {
    await tester.runAsync(() => account('Alfa', 'BRL'));
    await pumpScreen(tester);

    await tester.tap(find.byTooltip('Transferir'));
    await tester.pump();

    expect(find.text('Cadastre duas contas para transferir.'), findsOneWidget);
    expect(find.byKey(const Key('financas-transfer-out')), findsNothing);
  });

  testWidgets('em moedas diferentes o diálogo pede o que saiu e o que chegou, e mostra a taxa', (tester) async {
    await tester.runAsync(() async {
      await account('Alfa', 'BRL');
      await account('Beta', 'USD');
    });
    await pumpScreen(tester);

    await tester.tap(find.byTooltip('Transferir'));
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('financas-transfer-in')), findsOneWidget);
    await tester.enterText(find.byKey(const Key('financas-transfer-out')), '520');
    await tester.enterText(find.byKey(const Key('financas-transfer-in')), '100');
    await tester.pump();

    // A taxa segue o idioma: em pt a vírgula (antes saía "0.1923", com ponto).
    expect(find.textContaining('Taxa efetiva: 1 BRL = 0,1923 USD'), findsOneWidget);
  });

  testWidgets('a dívida abate o patrimônio e aparece no resumo', (tester) async {
    await tester.runAsync(() async {
      final id = await account('Nubank', 'BRL');
      await add(id, GeneralTransactionType.receita, 5000, '2026-01-02');
      // Principal 1200 sem juros em 12 meses a partir de 2026-01-15: as parcelas
      // vencem de 15/02/2026 a 15/01/2027 e todas as já vencidas abatem.
      await repository.createLiability(
        Liability(
          bankAccountId: id,
          nome: 'Carro',
          titular: 'Eu',
          principal: 1200,
          taxaPercentual: 0,
          prazoMeses: 12,
          sistema: AmortizationSystem.price,
          dataInicio: '2020-01-15',
          createdAt: DateTime(2026, 1, 1),
        ),
      );
    });
    await pumpScreen(tester);

    // 2020 + 12 meses: tudo vencido, saldo devedor zero, nada a mostrar.
    expect(find.byKey(const Key('financas-debt')), findsNothing);
    expect(textOf(tester, 'financas-net-worth').data, 'BRL 3.800,00'); // 5000 - 12 * 100
  });

  testWidgets('dívida em aberto: o resumo mostra o saldo devedor e subtrai do total', (tester) async {
    final start = DateTime.now().add(const Duration(days: 1));
    await tester.runAsync(() async {
      final id = await account('Nubank', 'BRL');
      await add(id, GeneralTransactionType.receita, 5000, '2026-01-02');
      await repository.createLiability(
        Liability(
          bankAccountId: id,
          nome: 'Carro',
          titular: 'Eu',
          principal: 1200,
          taxaPercentual: 0,
          prazoMeses: 12,
          sistema: AmortizationSystem.price,
          dataInicio: toIsoDay(start),
          createdAt: DateTime(2026, 1, 1),
        ),
      );
    });
    await pumpScreen(tester);

    // Nenhuma parcela venceu: a conta segue com 5000 e a dívida inteira abate.
    expect(textOf(tester, 'financas-debt').data, 'Dívidas −BRL 1.200,00');
    expect(textOf(tester, 'financas-net-worth').data, 'BRL 3.800,00');
  });

  testWidgets('tela de dívidas: cadastra, mostra saldo e cronograma, e exclui', (tester) async {
    late BankAccount nubank;
    await tester.runAsync(() async {
      final id = await account('Nubank', 'BRL');
      nubank = BankAccount(
        id: id,
        nome: 'Nubank',
        titular: 'Eu',
        currency: 'BRL',
        createdAt: DateTime(2026, 1, 1),
      );
    });
    await tester.pumpWidget(
      localizedApp(
        FinancasLiabilitiesScreen(accounts: [nubank], now: DateTime(2026, 3, 15)),
      ),
    );
    Future<void> settle(bool Function() ready) async {
      for (var i = 0; i < 200; i++) {
        await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
        await tester.pump();
        if (ready()) break;
      }
    }

    await settle(() => find.text('Nenhuma dívida cadastrada ainda.').evaluate().isNotEmpty);
    expect(find.text('Nenhuma dívida cadastrada ainda.'), findsOneWidget);

    await tester.tap(find.text('Dívida'));
    await tester.pumpAndSettle();
    await tester.enterText(find.byKey(const Key('financas-liability-name')), 'Carro');
    await tester.enterText(find.byKey(const Key('financas-liability-principal')), '1200');
    await tester.enterText(find.byKey(const Key('financas-liability-rate')), '0');
    await tester.enterText(find.byKey(const Key('financas-liability-term')), '12');
    await tester.tap(find.widgetWithText(FilledButton, 'Cadastrar'));
    await tester.pumpAndSettle();
    await settle(() => find.text('Carro').evaluate().isNotEmpty);

    // Início em 15/03/2026 (o "hoje" injetado): nenhuma parcela venceu ainda.
    final summary = tester.widget<Text>(find.byWidgetPredicate(
      (w) => w is Text && (w.key as ValueKey?)?.value.toString().contains('financas-liability-summary-') == true,
    ));
    expect(summary.data, contains('Saldo devedor BRL 1.200,00'));
    expect(summary.data, contains('0 de 12 parcelas pagas'));
    expect(summary.data, contains('Próxima: BRL 100,00 em 15/04/2026'));

    await tester.tap(find.text('Carro'));
    await tester.pumpAndSettle();
    expect(find.textContaining('01 · 15/04/2026 · BRL 100,00'), findsOneWidget);

    await tester.tap(find.text('Excluir dívida'));
    await tester.pumpAndSettle();
    await tester.tap(find.widgetWithText(FilledButton, 'Excluir'));
    await tester.pumpAndSettle();
    await settle(() => find.text('Nenhuma dívida cadastrada ainda.').evaluate().isNotEmpty);
    expect(find.text('Nenhuma dívida cadastrada ainda.'), findsOneWidget);
  });

  testWidgets('empréstimo marcado soma ao patrimônio; desmarcado fica de fora', (tester) async {
    await tester.runAsync(() async {
      await account('Nubank', 'BRL');
      final who = await repository.insertBorrower(
        Borrower(nome: 'Irmão', createdAt: DateTime(2026, 1, 1)),
      );
      Future<int> lend(double principal, bool include) => repository.createLoan(
        Loan(
          borrowerId: who,
          principal: principal,
          dataInicio: '2026-01-10',
          includeInNetWorth: include,
          currency: 'BRL',
          createdAt: DateTime(2026, 1, 10),
        ),
      );
      await lend(500, true);
      await lend(9000, false);
    });
    await pumpScreen(tester);

    expect(textOf(tester, 'financas-loans').data, 'Empréstimos a receber BRL 500,00');
    expect(textOf(tester, 'financas-net-worth').data, 'BRL 500,00');
  });

  testWidgets('tela de empréstimos: cadastra, baixa, mostra o saldo e exclui', (tester) async {
    await tester.pumpWidget(
      localizedApp(FinancasLoansScreen(accounts: const [], now: DateTime(2026, 3, 15))),
    );
    Future<void> settle(bool Function() ready) async {
      for (var i = 0; i < 200; i++) {
        await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
        await tester.pump();
        if (ready()) break;
      }
    }

    String summary() => tester
        .widget<Text>(find.byWidgetPredicate(
          (w) => w is Text && (w.key as ValueKey?)?.value.toString().contains('financas-loan-summary-') == true,
        ))
        .data!;

    await settle(() => find.text('Nenhum empréstimo cadastrado ainda.').evaluate().isNotEmpty);
    expect(find.text('Nenhum empréstimo cadastrado ainda.'), findsOneWidget);

    await tester.tap(find.text('Empréstimo'));
    await tester.pumpAndSettle();
    await tester.enterText(find.byKey(const Key('financas-loan-new-borrower')), 'Irmão');
    await tester.enterText(find.byKey(const Key('financas-loan-principal')), '1000');
    await tester.enterText(find.byKey(const Key('financas-loan-rate')), '1');
    await tester.tap(find.widgetWithText(FilledButton, 'Cadastrar'));
    await tester.pumpAndSettle();
    await settle(() => find.text('Irmão').evaluate().isNotEmpty);

    // Início no "hoje" injetado (15/03/2026): nenhum mês completo, sem juros.
    expect(summary(), contains('A receber BRL 1.000,00'));
    expect(summary(), isNot(contains('Juros acumulados')));

    await tester.tap(find.text('Irmão'));
    await tester.pumpAndSettle();
    expect(find.text('Nenhuma baixa ainda.'), findsOneWidget);
    await tester.tap(find.text('Registrar baixa'));
    await tester.pumpAndSettle();
    await tester.enterText(find.byKey(const Key('financas-repay-received')), '300');
    await tester.tap(find.widgetWithText(FilledButton, 'Registrar'));
    await tester.pumpAndSettle();
    await settle(() => find.textContaining('baixou BRL 300,00').evaluate().isNotEmpty);

    expect(summary(), contains('A receber BRL 700,00'));
    expect(find.textContaining('recebido BRL 300,00 · baixou BRL 300,00'), findsOneWidget);

    await tester.tap(find.text('Excluir'));
    await tester.pumpAndSettle();
    await tester.tap(find.widgetWithText(FilledButton, 'Excluir'));
    await tester.pumpAndSettle();
    await settle(() => find.text('Nenhum empréstimo cadastrado ainda.').evaluate().isNotEmpty);
    expect(find.text('Nenhum empréstimo cadastrado ainda.'), findsOneWidget);
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
    expect(fxUnavailableBanner(pt, {'PYG', 'ARS'}), contains('ARS, PYG'));
    expect(missingItemsNote(pt, 0), isNull);
    expect(missingItemsNote(pt, 1), '1 item fora da soma por falta de câmbio ou cotação.');
    expect(missingItemsNote(pt, 3), '3 itens fora da soma por falta de câmbio ou cotação.');
  });

  test('textos do aviso em inglês', () {
    expect(
      fxUnavailableBanner(en, {'PYG', 'ARS'}),
      'Exchange rate unavailable right now for ARS, PYG. '
      'Items in those currencies are left out of net worth.',
    );
    expect(missingItemsNote(en, 0), isNull);
    expect(missingItemsNote(en, 1), '1 item left out of the sum for lack of exchange rate or quote.');
    expect(missingItemsNote(en, 3), '3 items left out of the sum for lack of exchange rate or quote.');
  });

  // Idioma do aparelho em inglês (ADR 0001).
  group('em inglês', () {
    testWidgets('a aba: título, resumo com ponto decimal, aviso de câmbio e botões', (tester) async {
      late int pyg;
      await tester.runAsync(() async {
        pyg = await account('Banco', 'PYG');
        final brl = await account('Nubank', 'BRL');
        await add(pyg, GeneralTransactionType.receita, 1000000, '2026-01-02');
        await add(brl, GeneralTransactionType.receita, 50, '2026-01-06');
      });
      await pumpScreen(tester, locale: 'en');

      expect(find.text('Finances'), findsOneWidget);
      expect(find.text('Net worth'), findsOneWidget);
      expect(textOf(tester, 'financas-cash').data, 'Cash BRL 50.00');
      expect(textOf(tester, 'financas-portfolio').data, 'Portfolio BRL 0.00');
      expect(textOf(tester, 'financas-missing').data, '1 item left out of the sum for lack of exchange rate or quote.');
      expect(textOf(tester, 'financas-fx-banner').data, fxUnavailableBanner(en, {'PYG'}));
      expect(textOf(tester, 'financas-equivalent-$pyg').data, 'no exchange rate PYG/BRL');
      expect(textOf(tester, 'financas-balance-$pyg').data, 'PYG 1,000,000.00');
      for (final tooltip in ['Transfer', 'Loans', 'Debts', 'Categories', 'Refresh']) {
        expect(find.byTooltip(tooltip), findsOneWidget, reason: tooltip);
      }
      expect(find.widgetWithText(FloatingActionButton, 'Account'), findsOneWidget);
      for (final pt in ['Caixa', 'Carteira', 'Patrimônio', 'Transferir', 'Nenhuma']) {
        expect(find.textContaining(pt), findsNothing, reason: pt);
      }
    });

    testWidgets('sem conta: aviso em inglês', (tester) async {
      await pumpScreen(tester, locale: 'en');
      expect(find.text('No accounts yet.'), findsOneWidget);
    });

    testWidgets('transferir com uma conta só avisa em inglês', (tester) async {
      await tester.runAsync(() => account('Alfa', 'BRL'));
      await pumpScreen(tester, locale: 'en');

      await tester.tap(find.byTooltip('Transfer'));
      await tester.pump();

      expect(find.text('Add two accounts to transfer.'), findsOneWidget);
    });

    testWidgets('moedas diferentes: rótulos e taxa efetiva com ponto decimal', (tester) async {
      await tester.runAsync(() async {
        await account('Alfa', 'BRL');
        await account('Beta', 'USD');
      });
      await pumpScreen(tester, locale: 'en');

      await tester.tap(find.byTooltip('Transfer'));
      await tester.pumpAndSettle();
      expect(find.text('From'), findsOneWidget);
      expect(find.text('To'), findsOneWidget);
      expect(find.text('Sent (BRL)'), findsOneWidget);
      expect(find.text('Received (USD)'), findsOneWidget);
      expect(find.textContaining('Spread and fees already included'), findsOneWidget);
      await tester.enterText(find.byKey(const Key('financas-transfer-out')), '520');
      await tester.enterText(find.byKey(const Key('financas-transfer-in')), '100');
      await tester.pump();

      expect(find.textContaining('Effective rate: 1 BRL = 0.1923 USD'), findsOneWidget);
    });

    testWidgets('transferir sem valores avisa em inglês e não grava', (tester) async {
      await tester.runAsync(() async {
        await account('Alfa', 'BRL');
        await account('Beta', 'BRL');
      });
      await pumpScreen(tester, locale: 'en');

      await tester.tap(find.byTooltip('Transfer'));
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(FilledButton, 'Transfer'));
      await tester.pumpAndSettle();

      expect(find.text('Enter the amounts, greater than zero.'), findsOneWidget);
      final all = await tester.runAsync(repository.listTransactions);
      expect(all, isEmpty);
    });

    testWidgets('nova conta: diálogo em inglês', (tester) async {
      await pumpScreen(tester, locale: 'en');

      await tester.tap(find.widgetWithText(FloatingActionButton, 'Account'));
      await tester.pumpAndSettle();

      expect(find.text('New account'), findsOneWidget);
      expect(find.text('Name'), findsOneWidget);
      expect(find.text('Holder'), findsOneWidget);
      expect(find.text('Currency'), findsOneWidget);
      expect(find.text('Cancel'), findsOneWidget);
      expect(find.text('Save'), findsOneWidget);
    });

    testWidgets('conta com lançamentos: a moeda aparece travada, com o nome da moeda em inglês', (
      tester,
    ) async {
      await tester.runAsync(() async {
        final id = await account('Nubank', 'BRL');
        await add(id, GeneralTransactionType.receita, 10, '2026-01-02');
      });
      await pumpScreen(tester, locale: 'en');

      await tester.tap(find.byType(PopupMenuButton<String>));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Edit'));
      await waitFor(tester, find.text('Edit account'));

      expect(find.text('Edit account'), findsOneWidget);
      expect(find.text("Can't change on an account that already has transactions."), findsOneWidget);
      expect(find.text('BRL — Brazilian real'), findsOneWidget);
    });

    testWidgets('editar uma conta que sumiu mostra o erro traduzido no SnackBar', (tester) async {
      late int id;
      await tester.runAsync(() async => id = await account('Nubank', 'BRL'));
      await pumpScreen(tester, locale: 'en');

      await tester.tap(find.byType(PopupMenuButton<String>));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Edit'));
      await waitFor(tester, find.text('Edit account'));
      // A conta some do banco com o diálogo aberto: salvar bate em `accountNotFound`.
      await tester.runAsync(() => repository.deleteAccount(id));
      await tester.tap(find.widgetWithText(FilledButton, 'Save'));
      await waitFor(tester, find.text('Account not found'));

      expect(find.text('Account not found'), findsOneWidget);
    });

    testWidgets('excluir conta: confirmação em inglês', (tester) async {
      await tester.runAsync(() async {
        final id = await account('Nubank', 'BRL');
        await add(id, GeneralTransactionType.receita, 10, '2026-01-02');
      });
      await pumpScreen(tester, locale: 'en');

      await tester.tap(find.byType(PopupMenuButton<String>));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Delete'));
      await waitFor(tester, find.text('Delete Nubank?'));

      expect(find.text('Delete Nubank?'), findsOneWidget);
      expect(find.text('The transactions of this account will also be deleted.'), findsOneWidget);
    });
  });

  // A tela de Lançamentos de uma conta, nos dois idiomas.
  group('lançamentos de uma conta', () {
    late int cartao;
    late int mercado;

    Future<void> seedAccount(WidgetTester tester) async {
      final month = toIsoDay(DateTime.now()).substring(0, 7);
      await tester.runAsync(() async {
        cartao = await account('Nubank', 'BRL');
        mercado = await repository.insertCategory(
          GeneralTransactionCategory(nome: 'Mercado', createdAt: DateTime(2026, 1, 1)),
        );
        await add(cartao, GeneralTransactionType.receita, 1000, '2026-01-02');
        await repository.insertTransaction(
          GeneralTransaction(
            bankAccountId: cartao,
            type: GeneralTransactionType.despesa,
            categoryId: mercado,
            valor: 250.5,
            date: '$month-01',
            createdAt: DateTime(2026, 1, 1),
          ),
        );
        await add(cartao, GeneralTransactionType.receita, 99, '2999-01-01'); // futuro
      });
    }

    Future<void> openAccount(WidgetTester tester, String locale) async {
      await seedAccount(tester);
      await pumpScreen(tester, locale: locale);
      await tester.tap(find.text('Nubank'));
      await waitFor(tester, find.byKey(const Key('financas-account-balance')));
      // O saldo aparece antes da lista; espera também os lançamentos carregarem.
      await waitFor(tester, find.byType(Card));
    }

    testWidgets('pt: saldo, tipos, categoria, data e o aviso de lançamento futuro', (tester) async {
      await openAccount(tester, 'pt');

      expect(find.text('Saldo'), findsOneWidget);
      expect(textOf(tester, 'financas-account-balance').data, 'BRL 749,50');
      expect(find.text('Nenhum lançamento nesta conta ainda.'), findsNothing);
      expect(find.textContaining('Despesa · Mercado · '), findsOneWidget);
      expect(find.textContaining('Receita · 02/01/2026'), findsOneWidget);
      expect(find.textContaining('ainda não entra no saldo'), findsOneWidget);
      expect(find.widgetWithText(FloatingActionButton, 'Lançamento'), findsOneWidget);
    });

    testWidgets('en: o mesmo, em inglês e com datas mês/dia/ano', (tester) async {
      await openAccount(tester, 'en');

      expect(find.text('Balance'), findsOneWidget);
      expect(textOf(tester, 'financas-account-balance').data, 'BRL 749.50');
      expect(find.textContaining('Expense · Mercado · '), findsOneWidget); // nome da categoria é dado
      expect(find.textContaining('Income · 01/02/2026'), findsOneWidget);
      expect(find.textContaining('not in the balance yet'), findsOneWidget);
      expect(find.widgetWithText(FloatingActionButton, 'Transaction'), findsOneWidget);
      for (final pt in ['Saldo', 'Receita', 'Despesa', 'ainda não']) {
        expect(find.textContaining(pt), findsNothing, reason: pt);
      }
    });

    testWidgets('en: novo lançamento, validação e exclusão em inglês', (tester) async {
      await openAccount(tester, 'en');

      await tester.tap(find.widgetWithText(FloatingActionButton, 'Transaction'));
      await tester.pumpAndSettle();
      expect(find.text('New transaction'), findsOneWidget);
      expect(find.text('Income'), findsWidgets);
      expect(find.text('Expense'), findsWidgets);
      expect(find.text('Category'), findsOneWidget);
      expect(find.text('Amount (BRL)'), findsOneWidget);
      expect(find.text('Notes'), findsOneWidget);

      await tester.tap(find.widgetWithText(FilledButton, 'Save'));
      await tester.pumpAndSettle();
      expect(find.text('Enter an amount greater than zero.'), findsOneWidget);
    });

    testWidgets('en: confirmar a exclusão diz o tipo, o valor e a data em inglês', (tester) async {
      await openAccount(tester, 'en');

      await tester.tap(find.byIcon(Icons.delete_outline).first);
      await tester.pumpAndSettle();

      expect(find.text('Delete transaction?'), findsOneWidget);
      expect(find.textContaining(' of BRL '), findsWidgets);
      expect(find.textContaining(' on '), findsWidgets);
    });

    testWidgets('sem lançamentos: aviso nos dois idiomas', (tester) async {
      await tester.runAsync(() => account('Vazia', 'BRL'));
      await pumpScreen(tester, locale: 'en');
      await tester.tap(find.text('Vazia'));
      await waitFor(tester, find.text('No transactions in this account yet.'));
      expect(find.text('No transactions in this account yet.'), findsOneWidget);
    });
  });

  // Categorias nos dois idiomas.
  group('categorias em inglês', () {
    testWidgets('lista, limite, gasto do mês e estouro com ponto decimal', (tester) async {
      late int mercado;
      late int cartao;
      final month = toIsoDay(DateTime.now()).substring(0, 7);
      await tester.runAsync(() async {
        cartao = await account('Nubank', 'BRL');
        mercado = await repository.insertCategory(
          GeneralTransactionCategory(nome: 'Mercado', limiteMensal: 500, createdAt: DateTime(2026, 1, 1)),
        );
        await repository.insertTransaction(
          GeneralTransaction(
            bankAccountId: cartao,
            type: GeneralTransactionType.despesa,
            categoryId: mercado,
            valor: 600,
            date: '$month-01',
            createdAt: DateTime(2026, 1, 1),
          ),
        );
      });
      await tester.pumpWidget(
        localizedApp(
          FinancasCategoriesScreen(
            base: 'BRL',
            ratesToBrl: const {},
            accounts: [
              BankAccount(id: cartao, nome: 'Nubank', titular: 'Eu', currency: 'BRL', createdAt: DateTime(2026, 1, 1)),
            ],
          ),
          locale: 'en',
        ),
      );
      for (var i = 0; i < 200; i++) {
        await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
        await tester.pump();
        if (find.byKey(Key('financas-category-spent-$mercado')).evaluate().isNotEmpty) break;
      }

      expect(find.text('Categories'), findsOneWidget);
      expect(textOf(tester, 'financas-category-spent-$mercado').data, 'BRL 600.00 of BRL 500.00');
      expect(textOf(tester, 'financas-category-over-$mercado').data, 'Over by BRL 100.00');
      expect(find.widgetWithText(FloatingActionButton, 'Category'), findsOneWidget);
    });

    testWidgets('sem categorias, novo diálogo e validação em inglês', (tester) async {
      await tester.pumpWidget(
        localizedApp(
          const FinancasCategoriesScreen(base: 'BRL', ratesToBrl: {}, accounts: []),
          locale: 'en',
        ),
      );
      for (var i = 0; i < 100; i++) {
        await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
        await tester.pump();
        if (find.text('No categories yet.').evaluate().isNotEmpty) break;
      }
      expect(find.text('No categories yet.'), findsOneWidget);

      await tester.tap(find.widgetWithText(FloatingActionButton, 'Category'));
      await tester.pumpAndSettle();
      expect(find.text('New category'), findsOneWidget);
      expect(find.text('Monthly limit (BRL)'), findsOneWidget);
      expect(find.text('Empty = no limit.'), findsOneWidget);

      await tester.tap(find.widgetWithText(FilledButton, 'Save'));
      await tester.pumpAndSettle();
      expect(find.text('Enter the name and, if any, a limit greater than zero.'), findsOneWidget);
    });

    // O limite preenchido na edição segue o separador do idioma (antes: `toString()`, ponto).
    for (final (locale, expected) in [('pt', '1500,5'), ('en', '1500.5')]) {
      testWidgets('edição preenche o limite com o separador do idioma ($locale)', (tester) async {
        await tester.runAsync(() async {
          await repository.insertCategory(
            GeneralTransactionCategory(nome: 'Mercado', limiteMensal: 1500.5, createdAt: DateTime(2026, 1, 1)),
          );
        });
        await tester.pumpWidget(
          localizedApp(
            const FinancasCategoriesScreen(base: 'BRL', ratesToBrl: {}, accounts: []),
            locale: locale,
          ),
        );
        await waitFor(tester, find.text('Mercado'));

        await tester.tap(find.byType(PopupMenuButton<String>));
        await tester.pumpAndSettle();
        await tester.tap(find.text(locale == 'pt' ? 'Editar' : 'Edit'));
        await tester.pumpAndSettle();

        expect(find.widgetWithText(TextField, expected), findsOneWidget);
      });
    }
  });

  // Dívidas e Empréstimos nos dois idiomas (ADR 0001, PR 3b).
  group('dívidas e empréstimos em inglês', () {
    testWidgets('dívida: cadastra, resumo, cronograma e sistema de amortização em inglês', (tester) async {
      late BankAccount nubank;
      await tester.runAsync(() async {
        final id = await account('Nubank', 'BRL');
        nubank = BankAccount(id: id, nome: 'Nubank', titular: 'Eu', currency: 'BRL', createdAt: DateTime(2026, 1, 1));
      });
      await tester.pumpWidget(
        localizedApp(
          FinancasLiabilitiesScreen(accounts: [nubank], now: DateTime(2026, 3, 15)),
          locale: 'en',
        ),
      );
      await waitFor(tester, find.text('No debts yet.'));
      expect(find.text('Debts'), findsOneWidget);

      await tester.tap(find.widgetWithText(FloatingActionButton, 'Debt'));
      await tester.pumpAndSettle();
      expect(find.text('New debt'), findsOneWidget);
      expect(find.text('Price (fixed installment)'), findsOneWidget);
      expect(find.text('Installment 1 is due one month after the start.'), findsOneWidget);
      expect(find.text('Start 03/15/2026'), findsOneWidget);
      await tester.enterText(find.byKey(const Key('financas-liability-name')), 'Car');
      await tester.enterText(find.byKey(const Key('financas-liability-principal')), '1200');
      await tester.enterText(find.byKey(const Key('financas-liability-rate')), '0');
      await tester.enterText(find.byKey(const Key('financas-liability-term')), '12');
      await tester.tap(find.widgetWithText(FilledButton, 'Add'));
      await waitFor(tester, find.textContaining('Outstanding balance'));

      final summary = tester.widget<Text>(find.byWidgetPredicate(
        (w) => w is Text && (w.key as ValueKey?)?.value.toString().contains('financas-liability-summary-') == true,
      ));
      expect(summary.data, contains('Outstanding balance BRL 1,200.00'));
      expect(summary.data, contains('0 of 12 installments paid'));
      expect(summary.data, contains('Next: BRL 100.00 on 04/15/2026'));

      await tester.tap(find.text('Car'));
      await tester.pumpAndSettle();
      expect(find.text('Price (fixed installment) · 0.0% per month · principal BRL 1,200.00'), findsOneWidget);
      expect(find.textContaining('01 · 04/15/2026 · BRL 100.00 (interest BRL 0.00)'), findsOneWidget);

      await tester.tap(find.text('Delete debt'));
      await tester.pumpAndSettle();
      expect(find.text('Delete Car?'), findsOneWidget);
      expect(find.text("This debt's installments are also removed from the account's transactions."), findsOneWidget);
      await tester.tap(find.widgetWithText(FilledButton, 'Delete'));
      await waitFor(tester, find.text('No debts yet.'));
      expect(find.text('No debts yet.'), findsOneWidget);
    });

    testWidgets('dívida sem conta: avisa em inglês', (tester) async {
      await tester.pumpWidget(
        localizedApp(FinancasLiabilitiesScreen(accounts: const [], now: DateTime(2026, 3, 15)), locale: 'en'),
      );
      await waitFor(tester, find.text('No debts yet.'));

      await tester.tap(find.widgetWithText(FloatingActionButton, 'Debt'));
      await tester.pumpAndSettle();

      expect(find.text('Add an account before the debt.'), findsOneWidget);
    });

    testWidgets('empréstimo: cadastra, baixa, taxa negativa recusada e exclui, tudo em inglês', (tester) async {
      await tester.pumpWidget(
        localizedApp(FinancasLoansScreen(accounts: const [], now: DateTime(2026, 3, 15)), locale: 'en'),
      );
      await waitFor(tester, find.text('No loans yet.'));
      expect(find.text('Loans'), findsOneWidget);

      await tester.tap(find.widgetWithText(FloatingActionButton, 'Loan'));
      await tester.pumpAndSettle();
      expect(find.text('New loan'), findsOneWidget);
      expect(find.text('Without an account it is just a record: no balance changes.'), findsOneWidget);
      await tester.enterText(find.byKey(const Key('financas-loan-new-borrower')), 'Brother');
      await tester.enterText(find.byKey(const Key('financas-loan-principal')), '1000');
      await tester.tap(find.widgetWithText(FilledButton, 'Add'));
      // Não esperar por 'Brother': o texto digitado no diálogo também casa com `find.text`.
      await waitFor(tester, find.textContaining('To receive'));

      String summary() => tester
          .widget<Text>(find.byWidgetPredicate(
            (w) => w is Text && (w.key as ValueKey?)?.value.toString().contains('financas-loan-summary-') == true,
          ))
          .data!;
      expect(summary(), contains('To receive BRL 1,000.00'));
      expect(summary(), contains('Principal BRL 1,000.00 on 03/15/2026'));

      await tester.tap(find.text('Brother'));
      await tester.pumpAndSettle();
      expect(find.text('No repayments yet.'), findsOneWidget);

      await tester.tap(find.text('Register repayment'));
      await tester.pumpAndSettle();
      await tester.enterText(find.byKey(const Key('financas-repay-received')), '300');
      await tester.tap(find.widgetWithText(FilledButton, 'Register'));
      await waitFor(tester, find.textContaining('written off BRL 300.00'));
      expect(summary(), contains('To receive BRL 700.00'));
      expect(find.text('03/15/2026 · received BRL 300.00 · written off BRL 300.00'), findsOneWidget);

      await tester.tap(find.text('Edit'));
      await tester.pumpAndSettle();
      expect(find.text('Edit loan'), findsOneWidget);
      await tester.enterText(find.byKey(const Key('financas-loan-edit-rate')), '-1');
      await tester.tap(find.widgetWithText(FilledButton, 'Save'));
      await tester.pumpAndSettle();
      expect(find.text('The rate cannot be negative.'), findsOneWidget);

      await tester.tap(find.text('Delete'));
      await tester.pumpAndSettle();
      expect(find.text('Delete the loan to Brother?'), findsOneWidget);
      await tester.tap(find.widgetWithText(FilledButton, 'Delete'));
      await waitFor(tester, find.text('No loans yet.'));
      expect(find.text('No loans yet.'), findsOneWidget);
    });

    testWidgets('empréstimo inválido: aviso em inglês e nada é gravado', (tester) async {
      await tester.pumpWidget(
        localizedApp(FinancasLoansScreen(accounts: const [], now: DateTime(2026, 3, 15)), locale: 'en'),
      );
      await waitFor(tester, find.text('No loans yet.'));

      await tester.tap(find.widgetWithText(FloatingActionButton, 'Loan'));
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(FilledButton, 'Add'));
      await tester.pumpAndSettle();

      expect(
        find.text('Enter the borrower and a principal greater than zero; the rate, if any, cannot be negative.'),
        findsOneWidget,
      );
      expect(find.text('No loans yet.'), findsOneWidget);
    });
  });

  // A frase do lançamento gerado vem do vínculo, no idioma da tela (o banco guarda `notes` nulo).
  group('lançamentos gerados na lista da conta', () {
    for (final (locale, installment, granted) in [
      ('pt', 'Carro — parcela 2/2', 'Empréstimo para Irmão'),
      ('en', 'Carro — installment 2/2', 'Loan to Irmão'),
    ]) {
      testWidgets('frase derivada em $locale; observação própria continua valendo', (tester) async {
        late BankAccount nubank;
        await tester.runAsync(() async {
          final id = await account('Nubank', 'BRL');
          nubank = BankAccount(id: id, nome: 'Nubank', titular: 'Eu', currency: 'BRL', createdAt: DateTime(2026, 1, 1));
          await repository.createLiability(
            Liability(
              bankAccountId: id,
              nome: 'Carro',
              titular: 'Eu',
              principal: 200,
              taxaPercentual: 0,
              prazoMeses: 2,
              sistema: AmortizationSystem.price,
              dataInicio: '2026-01-15',
              createdAt: DateTime(2026, 1, 1),
            ),
          );
          final who = await repository.insertBorrower(Borrower(nome: 'Irmão', createdAt: DateTime(2026, 1, 1)));
          await repository.createLoan(
            Loan(
              borrowerId: who,
              bankAccountId: id,
              principal: 50,
              dataInicio: '2026-01-10',
              includeInNetWorth: true,
              currency: 'BRL',
              createdAt: DateTime(2026, 1, 10),
            ),
          );
          await repository.insertTransaction(
            GeneralTransaction(
              bankAccountId: id,
              type: GeneralTransactionType.despesa,
              valor: 10,
              date: '2026-01-11',
              notes: 'Reserva',
              createdAt: DateTime(2026, 1, 11),
            ),
          );
        });
        await tester.pumpWidget(localizedApp(FinancasTransactionsScreen(account: nubank), locale: locale));
        await waitFor(tester, find.textContaining(installment));

        expect(find.textContaining(installment), findsOneWidget);
        expect(find.textContaining(granted), findsOneWidget);
        expect(find.textContaining('Reserva'), findsOneWidget);
      });
    }
  });
}
