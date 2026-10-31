import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/domain/fx_history.dart';
import 'package:anchor_mobile/domain/home_layout.dart';
import 'package:anchor_mobile/domain/money_format.dart';
import 'package:anchor_mobile/models/asset.dart';
import 'package:anchor_mobile/models/bank_account.dart';
import 'package:anchor_mobile/models/general_transaction.dart';
import 'package:anchor_mobile/models/position.dart';
import 'package:anchor_mobile/screens/home_screen.dart';
import 'package:anchor_mobile/services/general_finance_repository.dart';
import 'package:anchor_mobile/services/home_layout_repository.dart';
import 'package:anchor_mobile/services/live_positions.dart';

// A Home montada de verdade (SQLite ffi, sem rede, cotações injetadas).
void main() {
  final layoutRepository = HomeLayoutRepository();
  late Directory dir;

  setUp(() async {
    dir = await Directory.systemTemp.createTemp('anchor-home-screen-');
    await databaseFactory.setDatabasesPath(dir.path);
    await AppDatabase.resetForTest();
  });

  tearDown(() async {
    await AppDatabase.resetForTest();
    await dir.delete(recursive: true);
  });

  LivePositionRow row(
    String ticker, {
    String currency = 'BRL',
    required double quantity,
    required double avg,
    required double price,
  }) => LivePositionRow(
    Position(
      asset: Asset(
        id: 1,
        ticker: ticker,
        name: ticker,
        assetClass: currency == 'BRL'
            ? AssetClass.acaoBr
            : AssetClass.acaoInternacional,
        currency: currency,
        createdAt: DateTime(2026, 1, 1),
      ),
      netQuantity: quantity,
      averageBuyPrice: avg,
    ),
    price,
  );

  // PETR4: R$ 250 (custo 200). AAPL: US$ 24 = R$ 120 (custo US$ 20 = R$ 100).
  final positions = [
    row('PETR4', quantity: 10, avg: 20, price: 25),
    row('AAPL', currency: 'USD', quantity: 2, avg: 10, price: 12),
  ];

  Future<void> seedCash() async {
    final repository = GeneralFinanceRepository();
    final id = await repository.insertAccount(
      BankAccount(
        nome: 'Nubank',
        titular: 'Eu',
        currency: 'BRL',
        createdAt: DateTime(2026, 1, 1),
      ),
    );
    await repository.insertTransaction(
      GeneralTransaction(
        bankAccountId: id,
        type: GeneralTransactionType.receita,
        valor: 1000,
        date: '2026-01-02',
        createdAt: DateTime(2026, 1, 1),
      ),
    );
  }

  // O SQLite ffi responde em tempo real, fora do relógio falso do teste: sonda
  // até a condição valer em vez de esperar um tempo fixo.
  Future<void> settleUntil(WidgetTester tester, bool Function() ready) async {
    for (var i = 0; i < 200; i++) {
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 25)),
      );
      await tester.pump();
      if (ready()) break;
    }
    await tester.pump(const Duration(milliseconds: 300));
  }

  bool has(String key) => find.byKey(Key(key)).evaluate().isNotEmpty;

  Future<void> pumpHome(
    WidgetTester tester, {
    Future<List<LivePositionRow>> Function()? loadPositions,
  }) async {
    await tester.pumpWidget(const SizedBox());
    await tester.pumpWidget(
      MaterialApp(
        home: HomeScreen(
          fetchFx: (currency) async =>
              FxHistory([(DateTime.now(), currency == 'USD' ? 5.0 : 1.0)]),
          loadPositions: loadPositions ?? () async => positions,
        ),
      ),
    );
    await settleUntil(tester, () => has('home-net-worth-text'));
  }

  String text(WidgetTester tester, String key) =>
      tester.widget<Text>(find.byKey(Key(key))).data!;

  Future<void> openCustomize(WidgetTester tester) async {
    await tester.tap(find.byKey(const Key('home-customize')));
    await tester.pumpAndSettle();
  }

  Future<void> save(WidgetTester tester) async {
    await tester.tap(find.byKey(const Key('home-save')));
    // a gravação é SQLite real, fora do relógio falso
    await settleUntil(tester, () => !has('home-save'));
  }

  testWidgets('mostra os 4 cards com os valores convertidos para a moeda base', (
    tester,
  ) async {
    await tester.runAsync(seedCash);
    await pumpHome(tester);

    for (final key in HomeWidgetKey.values) {
      expect(has('home-card-${key.id}'), isTrue, reason: key.id);
    }
    expect(text(tester, 'home-total-value-text'), formatMoney(370, 'BRL'));
    expect(
      text(tester, 'home-profit-text'),
      homeProfitLabel(70, 70 / 300 * 100, 'BRL'),
    );
    // caixa 1000 + carteira 370
    expect(text(tester, 'home-net-worth-text'), formatMoney(1370, 'BRL'));
    expect(has('home-allocation-row-acao_br'), isTrue);
    expect(has('home-allocation-row-acao_internacional'), isTrue);
  });

  testWidgets('sem cotação a carteira não vira prejuízo: o card avisa o que ficou fora', (
    tester,
  ) async {
    await tester.runAsync(seedCash);
    await pumpHome(
      tester,
      loadPositions: () async => [
        row('PETR4', quantity: 10, avg: 20, price: 25),
        LivePositionRow(
          Position(
            asset: Asset(
              id: 2,
              ticker: 'VALE3',
              name: 'VALE3',
              assetClass: AssetClass.acaoBr,
              currency: 'BRL',
              createdAt: DateTime(2026, 1, 1),
            ),
            netQuantity: 100,
            averageBuyPrice: 50,
          ),
          null,
        ),
      ],
    );

    expect(text(tester, 'home-total-value-text'), formatMoney(250, 'BRL'));
    expect(
      text(tester, 'home-profit-text'),
      homeProfitLabel(50, 25, 'BRL'),
      reason: 'o custo da VALE3 sem cotação não entra',
    );
    expect(has('home-total-value-missing'), isTrue);
  });

  testWidgets('falha ao cotar não derruba a tela: mostra o erro e mantém os cards', (
    tester,
  ) async {
    await tester.pumpWidget(const SizedBox());
    await tester.pumpWidget(
      MaterialApp(
        home: HomeScreen(
          fetchFx: (_) async => throw StateError('sem rede'),
          loadPositions: () async => throw StateError('sem rede'),
        ),
      ),
    );
    await settleUntil(tester, () => has('home-error'));

    expect(has('home-error'), isTrue);
    expect(has('home-card-total_value'), isTrue);
  });

  testWidgets('esconder um widget e salvar persiste depois de recarregar a tela', (
    tester,
  ) async {
    await pumpHome(tester);

    await openCustomize(tester);
    await tester.tap(find.byKey(const Key('home-visible-profit')));
    await tester.pump();
    await save(tester);

    expect(has('home-card-profit'), isFalse);
    expect(has('home-card-total_value'), isTrue);

    await pumpHome(tester);
    expect(has('home-card-profit'), isFalse, reason: 'recarregou do banco');
    final saved = await tester.runAsync(layoutRepository.load);
    expect(saved!.firstWhere((c) => c.key == HomeWidgetKey.profit).visible, isFalse);
  });

  testWidgets('trocar a largura persiste', (tester) async {
    await pumpHome(tester);

    await openCustomize(tester);
    await tester.tap(find.byKey(const Key('home-wide-total_value')));
    await tester.pump();
    await save(tester);

    final saved = await tester.runAsync(layoutRepository.load);
    expect(
      saved!.firstWhere((c) => c.key == HomeWidgetKey.totalValue).wide,
      isTrue,
    );
  });

  testWidgets('arrastar pela alça reordena e a ordem fica salva', (tester) async {
    await pumpHome(tester);

    await openCustomize(tester);
    await tester.drag(
      find.byKey(const Key('home-handle-total_value')),
      const Offset(0, 200),
    );
    await tester.pumpAndSettle();
    await save(tester);

    final saved = await tester.runAsync(layoutRepository.load);
    final order = [for (final c in saved!) c.key];
    expect(order.first, isNot(HomeWidgetKey.totalValue));
    expect(order.indexOf(HomeWidgetKey.totalValue), greaterThan(0));
    expect(order.toSet(), HomeWidgetKey.values.toSet(), reason: 'nada se perde');
  });

  testWidgets('cancelar descarta o rascunho', (tester) async {
    await pumpHome(tester);

    await openCustomize(tester);
    await tester.tap(find.byKey(const Key('home-visible-profit')));
    await tester.pump();
    await tester.tap(find.byKey(const Key('home-cancel')));
    await tester.pumpAndSettle();

    expect(has('home-card-profit'), isTrue);
    final saved = await tester.runAsync(layoutRepository.load);
    expect(saved!.every((c) => c.visible), isTrue);
  });

  testWidgets('restaurar padrão desfaz a personalização', (tester) async {
    await pumpHome(tester);

    await openCustomize(tester);
    await tester.tap(find.byKey(const Key('home-visible-profit')));
    await tester.pump();
    await save(tester);
    expect(has('home-card-profit'), isFalse);

    await openCustomize(tester);
    await tester.tap(find.byKey(const Key('home-reset')));
    await tester.pump();
    await save(tester);

    expect(has('home-card-profit'), isTrue);
    final saved = await tester.runAsync(layoutRepository.load);
    expect(
      [for (final c in saved!) c.key],
      [for (final c in defaultHomeLayout()) c.key],
    );
  });

  group('HomeLayoutRepository', () {
    test('sem nada salvo devolve o padrão', () async {
      final layout = await layoutRepository.load();

      expect(
        [for (final c in layout) c.key],
        [for (final c in defaultHomeLayout()) c.key],
      );
    });

    test('JSON corrompido no banco não quebra: volta ao padrão', () async {
      final db = await AppDatabase.instance();
      await db.insert('app_settings', {
        'key': 'home_layout',
        'value': '{{ lixo',
      });

      final layout = await layoutRepository.load();

      expect(layout, hasLength(HomeWidgetKey.values.length));
      expect(layout.every((c) => c.visible), isTrue);
    });

    test('salvar duas vezes substitui, não duplica', () async {
      final first = defaultHomeLayout();
      await layoutRepository.save(first);
      await layoutRepository.save(
        first.reversed.map((c) => c.copyWith(visible: false)).toList(),
      );

      final layout = await layoutRepository.load();

      expect(layout.first.key, HomeWidgetKey.netWorth);
      expect(layout.every((c) => !c.visible), isTrue);
    });
  });
}
