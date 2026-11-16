import 'dart:io';

import 'package:material_ui/material_ui.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/domain/coverage.dart';
import 'package:anchor_mobile/l10n/app_localizations.dart';
import 'package:anchor_mobile/models/asset.dart';
import 'package:anchor_mobile/models/portfolio_transaction.dart';
import 'package:anchor_mobile/screens/transaction_history_screen.dart';
import 'package:anchor_mobile/services/portfolio_repository.dart';

import 'support/localized_app.dart';
import 'support/settle.dart' show settle;

// Guarda de histórico (ADR 0004, lado do celular): editar e apagar não deixam venda sem cobertura.
// Mesma regra do desktop: só recusa o que piora; histórico que já estava descoberto não bloqueia.
// Dados fictícios.
PortfolioTransaction tx(
  TransactionType type,
  double quantity,
  int day, {
  int? id,
  int? assetId = 1,
}) => PortfolioTransaction(
  id: id,
  assetId: assetId,
  type: type,
  quantity: quantity,
  unitPrice: 10,
  totalValue: quantity * 10,
  date: DateTime(2026, 3, day, 15, 30),
  createdAt: DateTime(2026, 3, day),
);

void main() {
  group('hasUncoveredSale: a conta pura', () {
    test('D1: compra 10 e venda 10 está coberta; venda 11 não', () {
      expect(
        hasUncoveredSale([tx(TransactionType.compra, 10, 1, id: 1), tx(TransactionType.venda, 10, 2, id: 2)]),
        isFalse,
      );
      expect(
        hasUncoveredSale([tx(TransactionType.compra, 10, 1, id: 1), tx(TransactionType.venda, 11, 2, id: 2)]),
        isTrue,
      );
    });

    test('D2: venda no dia anterior à compra não está coberta', () {
      expect(
        hasUncoveredSale([tx(TransactionType.compra, 10, 5, id: 1), tx(TransactionType.venda, 10, 4, id: 2)]),
        isTrue,
      );
    });

    test('D3: no mesmo dia vale o id: compra antes da venda cobre, depois não', () {
      expect(
        hasUncoveredSale([tx(TransactionType.compra, 10, 3, id: 1), tx(TransactionType.venda, 10, 3, id: 2)]),
        isFalse,
      );
      expect(
        hasUncoveredSale([tx(TransactionType.venda, 10, 3, id: 1), tx(TransactionType.compra, 10, 3, id: 2)]),
        isTrue,
      );
    });

    test('D4: o come-cotas baixa cotas como uma venda', () {
      expect(
        hasUncoveredSale([tx(TransactionType.compra, 10, 1, id: 1), tx(TransactionType.comeCotas, 11, 2, id: 2)]),
        isTrue,
      );
    });

    test('D5: transferência, provento e amortização não mudam a quantidade', () {
      expect(
        hasUncoveredSale([
          tx(TransactionType.compra, 10, 1, id: 1),
          tx(TransactionType.transferencia, 10, 2, id: 2),
          tx(TransactionType.provento, 10, 2, id: 3),
          tx(TransactionType.amortizacao, 10, 2, id: 4),
          tx(TransactionType.venda, 10, 3, id: 5),
        ]),
        isFalse,
      );
    });

    test('D6: uma venda antes de qualquer compra é descoberta, mesmo que uma compra venha depois', () {
      expect(
        hasUncoveredSale([tx(TransactionType.venda, 5, 1, id: 1), tx(TransactionType.compra, 5, 2, id: 2)]),
        isTrue,
      );
    });

    test('D7: o horário do dia não conta, só o dia', () {
      final buy = PortfolioTransaction(
        id: 2,
        assetId: 1,
        type: TransactionType.compra,
        quantity: 10,
        unitPrice: 10,
        totalValue: 100,
        date: DateTime(2026, 3, 3, 23, 59),
        createdAt: DateTime(2026, 3, 3),
      );
      final sell = PortfolioTransaction(
        id: 1,
        assetId: 1,
        type: TransactionType.venda,
        quantity: 10,
        unitPrice: 10,
        totalValue: 100,
        date: DateTime(2026, 3, 3, 8, 0),
        createdAt: DateTime(2026, 3, 3),
      );
      // mesmo dia: a ordem é o id (a venda tem o menor), não a hora
      expect(hasUncoveredSale([buy, sell]), isTrue);
    });
  });

  group('worsensCoverage: só o que piora', () {
    test('W1: coberto antes e descoberto depois piora', () {
      expect(
        worsensCoverage(
          before: [tx(TransactionType.compra, 10, 1, id: 1), tx(TransactionType.venda, 10, 2, id: 2)],
          after: [tx(TransactionType.venda, 10, 2, id: 2)],
        ),
        isTrue,
      );
    });

    test('W2: já descoberto antes não bloqueia, mesmo continuando descoberto', () {
      expect(
        worsensCoverage(
          before: [tx(TransactionType.venda, 10, 2, id: 2), tx(TransactionType.compra, 5, 3, id: 3)],
          after: [tx(TransactionType.venda, 10, 2, id: 2)],
        ),
        isFalse,
      );
    });
  });

  group('o repositório', () {
    late Directory dir;
    final repository = PortfolioRepository();

    setUp(() async {
      dir = await Directory.systemTemp.createTemp('anchor-coverage-');
      await databaseFactory.setDatabasesPath(dir.path);
      await AppDatabase.resetForTest();
    });

    tearDown(() async {
      await AppDatabase.resetForTest();
      await dir.delete(recursive: true);
    });

    Future<int> addAsset(String ticker) => repository.insertAsset(
      Asset(
        ticker: ticker,
        name: ticker,
        assetClass: AssetClass.acaoBr,
        currency: 'BRL',
        createdAt: DateTime(2026, 1, 1),
      ),
    );

    Future<int> add(int assetId, TransactionType type, double quantity, int day) =>
        repository.insertTransaction(tx(type, quantity, day, assetId: assetId));

    Future<List<PortfolioTransaction>> all() => repository.listAllTransactions();

    test('R1: apagar a compra de que a venda depende é recusado e nada é apagado', () async {
      final petr = await addAsset('PETR4');
      final buy = await add(petr, TransactionType.compra, 10, 1);
      await add(petr, TransactionType.venda, 10, 2);

      await expectLater(repository.deleteTransaction(buy), throwsA(isA<HistoryCoverageException>()));

      expect(await all(), hasLength(2));
    });

    test('R2: apagar a venda vale, e depois a compra também', () async {
      final petr = await addAsset('PETR4');
      final buy = await add(petr, TransactionType.compra, 10, 1);
      final sell = await add(petr, TransactionType.venda, 10, 2);

      await repository.deleteTransaction(sell);
      await repository.deleteTransaction(buy);

      expect(await all(), isEmpty);
    });

    test('R3: apagar uma compra vale quando outra ainda cobre a venda', () async {
      final petr = await addAsset('PETR4');
      final first = await add(petr, TransactionType.compra, 10, 1);
      await add(petr, TransactionType.compra, 10, 2);
      await add(petr, TransactionType.venda, 10, 3);

      await repository.deleteTransaction(first);

      expect(await all(), hasLength(2));
    });

    test('R4: editar a compra para menos que a venda é recusado e o valor não muda', () async {
      final petr = await addAsset('PETR4');
      final buy = await add(petr, TransactionType.compra, 10, 1);
      await add(petr, TransactionType.venda, 10, 2);

      await expectLater(
        repository.updateTransaction(tx(TransactionType.compra, 5, 1, id: buy, assetId: petr)),
        throwsA(isA<HistoryCoverageException>()),
      );

      final saved = (await all()).firstWhere((t) => t.id == buy);
      expect(saved.quantity, 10);
    });

    test('R5: editar a compra mantendo a cobertura vale', () async {
      final petr = await addAsset('PETR4');
      final buy = await add(petr, TransactionType.compra, 10, 1);
      await add(petr, TransactionType.venda, 8, 2);

      await repository.updateTransaction(tx(TransactionType.compra, 9, 1, id: buy, assetId: petr));

      expect((await all()).firstWhere((t) => t.id == buy).quantity, 9);
    });

    test('R6: trocar o ativo da compra deixa a venda do ativo antigo descoberta e é recusado', () async {
      final petr = await addAsset('PETR4');
      final vale = await addAsset('VALE3');
      final buy = await add(petr, TransactionType.compra, 10, 1);
      await add(petr, TransactionType.venda, 10, 2);

      await expectLater(
        repository.updateTransaction(tx(TransactionType.compra, 10, 1, id: buy, assetId: vale)),
        throwsA(isA<HistoryCoverageException>()),
      );

      expect((await all()).firstWhere((t) => t.id == buy).assetId, petr);
    });

    test('R7: editar a venda para mais que o saldo é recusado', () async {
      final petr = await addAsset('PETR4');
      await add(petr, TransactionType.compra, 10, 1);
      final sell = await add(petr, TransactionType.venda, 10, 2);

      await expectLater(
        repository.updateTransaction(tx(TransactionType.venda, 11, 2, id: sell, assetId: petr)),
        throwsA(isA<HistoryCoverageException>()),
      );
    });

    test('R8: histórico que já estava descoberto não bloqueia apagar outra linha', () async {
      final petr = await addAsset('PETR4');
      await add(petr, TransactionType.venda, 10, 1); // descoberta desde o início
      final later = await add(petr, TransactionType.compra, 5, 3);

      await repository.deleteTransaction(later);

      expect(await all(), hasLength(1));
    });

    test('R9: apagar id que não existe não falha', () async {
      await repository.deleteTransaction(999);
      expect(await all(), isEmpty);
    });
  });

  group('a tela de histórico', () {
    late Directory dir;
    final repository = PortfolioRepository();
    late AppLocalizations pt;

    setUpAll(() async {
      pt = await AppLocalizations.delegate.load(const Locale('pt'));
    });

    setUp(() async {
      dir = await Directory.systemTemp.createTemp('anchor-coverage-screen-');
      await databaseFactory.setDatabasesPath(dir.path);
      await AppDatabase.resetForTest();
    });

    tearDown(() async {
      await AppDatabase.resetForTest();
      await dir.delete(recursive: true);
    });

    testWidgets('S1: apagar a compra de que a venda depende mostra o aviso e a linha continua', (tester) async {
      tester.view.physicalSize = const Size(900, 3000);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      await tester.runAsync(() async {
        final petr = await repository.insertAsset(
          Asset(
            ticker: 'PETR4',
            name: 'Petrobras',
            assetClass: AssetClass.acaoBr,
            currency: 'BRL',
            createdAt: DateTime(2026, 1, 1),
          ),
        );
        await repository.insertTransaction(tx(TransactionType.compra, 10, 1, assetId: petr));
        await repository.insertTransaction(tx(TransactionType.venda, 10, 2, assetId: petr));
      });
      await tester.pumpWidget(localizedApp(const TransactionHistoryScreen()));
      await settle(tester);

      final buyRow = find.widgetWithText(ListTile, 'Compra — PETR4');
      expect(buyRow, findsOneWidget);
      await tester.tap(find.descendant(of: buyRow, matching: find.byIcon(Icons.delete_outline)));
      await settle(tester);

      expect(find.text(pt.ptxCoverageWorsens), findsOneWidget);
      expect(find.text('Compra — PETR4'), findsOneWidget);
      final saved = await tester.runAsync(() => repository.listAllTransactions());
      expect(saved, hasLength(2));
    });
  });
}
