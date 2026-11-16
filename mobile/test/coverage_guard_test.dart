import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:material_ui/material_ui.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/domain/coverage.dart';
import 'package:anchor_mobile/domain/fund_tax.dart' show unfundedAmortizations;
import 'package:anchor_mobile/domain/transactions_csv_import.dart';
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

// Amortização de fundo: cota do dia 10,00 e [value] recebido (não mexe nas cotas).
PortfolioTransaction amort(int day, double value, {int? id, int? assetId = 1}) => PortfolioTransaction(
  id: id,
  assetId: assetId,
  type: TransactionType.amortizacao,
  unitPrice: 10,
  totalValue: value,
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

  group('amortizações: a conta pura', () {
    test('U1: compra 10 a 10,00 e amortização de 100 no dia seguinte cabe; de 101 não', () {
      final buy = tx(TransactionType.compra, 10, 1, id: 1);
      expect(unfundedAmortizations([buy, amort(2, 100, id: 2)]), isEmpty);
      expect(unfundedAmortizations([buy, amort(2, 101, id: 2)]).map((t) => t.id), [2]);
    });

    test('U2: amortização sem nenhuma cota na data não cabe', () {
      expect(unfundedAmortizations([amort(2, 10, id: 2)]).map((t) => t.id), [2]);
      expect(
        unfundedAmortizations([tx(TransactionType.compra, 10, 3, id: 1), amort(2, 10, id: 2)]).map((t) => t.id),
        [2],
        reason: 'a compra é depois da amortização',
      );
    });

    test('U3: venda anterior que tira as cotas quebra a amortização', () {
      final history = [
        tx(TransactionType.compra, 10, 1, id: 1),
        tx(TransactionType.venda, 10, 2, id: 2),
        amort(3, 10, id: 3),
      ];
      expect(unfundedAmortizations(history).map((t) => t.id), [3]);
    });

    test('W3: tirar as cotas de uma amortização que cabia piora', () {
      final buy = tx(TransactionType.compra, 10, 1, id: 1);
      final before = [buy, amort(2, 50, id: 2)];
      expect(worsensAmortizations(before: before, after: [amort(2, 50, id: 2)]), isTrue);
      expect(worsensAmortizations(before: before, after: before), isFalse);
    });

    test('W4: amortização que já não cabia antes não bloqueia nada', () {
      final before = [amort(2, 50, id: 2)];
      final after = [amort(2, 50, id: 2), tx(TransactionType.compra, 1, 5, id: 3)];
      expect(worsensAmortizations(before: before, after: after), isFalse);
    });

    test('W5: amortização nova (sem id) que não cabe piora', () {
      final buy = tx(TransactionType.compra, 10, 1, id: 1);
      expect(worsensAmortizations(before: [buy], after: [buy, amort(2, 500)]), isTrue);
      expect(worsensAmortizations(before: [buy], after: [buy, amort(2, 50)]), isFalse);
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

    // Grava direto no banco, sem a guarda: para montar um histórico que já estava descoberto.
    Future<void> seed(PortfolioTransaction transaction) async {
      final db = await AppDatabase.instance();
      await db.insert('portfolio_transactions', transaction.toMap()..remove('id'));
    }

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
      await seed(tx(TransactionType.venda, 10, 1, assetId: petr)); // descoberta desde o início
      final later = await add(petr, TransactionType.compra, 5, 3);

      await repository.deleteTransaction(later);

      expect(await all(), hasLength(1));
    });

    test('R9: apagar id que não existe não falha', () async {
      await repository.deleteTransaction(999);
      expect(await all(), isEmpty);
    });

    // Lançar, permutar e importar (PR 2): a mesma regra, agora na entrada.
    test('R10: lançar uma venda acima do saldo é recusado e nada é gravado', () async {
      final petr = await addAsset('PETR4');
      await add(petr, TransactionType.compra, 10, 1);

      await expectLater(
        add(petr, TransactionType.venda, 11, 2),
        throwsA(isA<HistoryCoverageException>()),
      );

      expect(await all(), hasLength(1));
    });

    test('R11: lançar venda dentro do saldo vale, inclusive no mesmo dia da compra', () async {
      final petr = await addAsset('PETR4');
      await add(petr, TransactionType.compra, 10, 1);
      await add(petr, TransactionType.venda, 4, 1);
      await add(petr, TransactionType.venda, 6, 2);

      expect(await all(), hasLength(3));
    });

    test('R11b: lançar uma compra datada antes de uma venda já gravada vale (cobre)', () async {
      final petr = await addAsset('PETR4');
      await seed(tx(TransactionType.venda, 10, 5, assetId: petr)); // descoberta
      await add(petr, TransactionType.compra, 10, 1);

      expect(await all(), hasLength(2));
    });

    test('R12: permuta com a venda descoberta recusa e não grava nenhuma das duas pernas', () async {
      final petr = await addAsset('PETR4');
      final vale = await addAsset('VALE3');
      await add(petr, TransactionType.compra, 5, 1);

      await expectLater(
        repository.insertTransactions([
          tx(TransactionType.venda, 6, 2, assetId: petr),
          tx(TransactionType.compra, 6, 2, assetId: vale),
        ]),
        throwsA(isA<HistoryCoverageException>()),
      );

      expect(await all(), hasLength(1));
    });

    test('R12b: permuta coberta grava as duas pernas', () async {
      final petr = await addAsset('PETR4');
      final vale = await addAsset('VALE3');
      await add(petr, TransactionType.compra, 5, 1);

      await repository.insertTransactions([
        tx(TransactionType.venda, 5, 2, assetId: petr),
        tx(TransactionType.compra, 5, 2, assetId: vale),
      ]);

      expect(await all(), hasLength(3));
    });

    ImportRow row(String ticker, TransactionType type, double quantity, int day) => ImportRow(
      line: day,
      date: DateTime(2026, 3, day),
      type: type,
      ticker: ticker,
      assetClass: AssetClass.acaoBr,
      currency: 'BRL',
      quantity: quantity,
      unitPrice: 10,
      totalValue: quantity * 10,
      fee: null,
      custody: null,
      notes: null,
      lossCreditUsed: null,
    );

    ImportPlan planOf(List<ImportRow> rows, {List<NewAssetPlan> newAssets = const []}) => ImportPlan(
      rows: rows,
      duplicates: 0,
      skipped: 0,
      issues: const [],
      newAssets: newAssets,
      newCustodias: const [],
    );

    test('R13: import com venda descoberta reverte tudo, inclusive o ativo novo', () async {
      final petr = await addAsset('PETR4');
      await add(petr, TransactionType.compra, 10, 1);

      await expectLater(
        repository.applyTransactionsImport(
          planOf(
            [
              row('NOVO3', TransactionType.compra, 5, 1),
              row('PETR4', TransactionType.venda, 11, 2),
            ],
            newAssets: const [NewAssetPlan(ticker: 'NOVO3', assetClass: AssetClass.acaoBr, currency: 'BRL')],
          ),
        ),
        throwsA(isA<HistoryCoverageException>()),
      );

      expect(await all(), hasLength(1));
      expect((await repository.listAssets()).map((a) => a.ticker), ['PETR4']);
    });

    test('R14: import em que a venda vem antes da compra no arquivo, mas a data cobre, vale', () async {
      await addAsset('PETR4');

      final written = await repository.applyTransactionsImport(
        planOf([
          row('PETR4', TransactionType.venda, 10, 2),
          row('PETR4', TransactionType.compra, 10, 1),
        ]),
      );

      expect(written, 2);
      expect(await all(), hasLength(2));
    });

    test('R14b: import com compra e venda no mesmo dia vale na ordem do arquivo', () async {
      await addAsset('PETR4');

      await repository.applyTransactionsImport(
        planOf([
          row('PETR4', TransactionType.compra, 10, 3),
          row('PETR4', TransactionType.venda, 10, 3),
        ]),
      );

      expect(await all(), hasLength(2));
    });

    test('R14c: import com venda e compra no mesmo dia, nessa ordem, é recusado', () async {
      await addAsset('PETR4');

      await expectLater(
        repository.applyTransactionsImport(
          planOf([
            row('PETR4', TransactionType.venda, 10, 3),
            row('PETR4', TransactionType.compra, 10, 3),
          ]),
        ),
        throwsA(isA<HistoryCoverageException>()),
      );

      expect(await all(), isEmpty);
    });

    test('R15: histórico já descoberto não bloqueia lançar uma compra', () async {
      final petr = await addAsset('PETR4');
      await seed(tx(TransactionType.venda, 10, 1, assetId: petr));

      await add(petr, TransactionType.compra, 3, 2);

      expect(await all(), hasLength(2));
    });

    // Amortização de fundo (PR 3): a mudança não pode deixar uma amortização sem cotas.
    Matcher amortizationRefused() =>
        throwsA(isA<HistoryCoverageException>().having((e) => e.problem, 'problem', HistoryProblem.amortizationUnfunded));

    Future<int> addAmort(int assetId, int day, double value) =>
        repository.insertTransaction(amort(day, value, assetId: assetId));

    test('A1: apagar a compra de que a amortização depende é recusado', () async {
      final fund = await addAsset('FUNDO');
      final buy = await add(fund, TransactionType.compra, 10, 1);
      await addAmort(fund, 2, 50);

      await expectLater(repository.deleteTransaction(buy), amortizationRefused());

      expect(await all(), hasLength(2));
    });

    test('A2: editar a compra para menos cotas do que o valor amortizado exige é recusado', () async {
      final fund = await addAsset('FUNDO');
      final buy = await add(fund, TransactionType.compra, 10, 1);
      await addAmort(fund, 2, 50);

      await expectLater(
        repository.updateTransaction(tx(TransactionType.compra, 4, 1, id: buy, assetId: fund)),
        amortizationRefused(),
      );

      expect((await all()).firstWhere((t) => t.id == buy).quantity, 10);
    });

    test('A3: lançar uma venda anterior que tira as cotas da amortização é recusado', () async {
      final fund = await addAsset('FUNDO');
      await add(fund, TransactionType.compra, 10, 1);
      await addAmort(fund, 3, 50);

      await expectLater(add(fund, TransactionType.venda, 10, 2), amortizationRefused());

      expect(await all(), hasLength(2));
    });

    test('A4: apagar a própria amortização vale, e depois a compra também', () async {
      final fund = await addAsset('FUNDO');
      final buy = await add(fund, TransactionType.compra, 10, 1);
      final am = await addAmort(fund, 2, 50);

      await repository.deleteTransaction(am);
      await repository.deleteTransaction(buy);

      expect(await all(), isEmpty);
    });

    test('A5: amortização que já não cabia não bloqueia apagar outra linha', () async {
      final fund = await addAsset('FUNDO');
      await seed(amort(1, 50, assetId: fund)); // sem cotas desde o início
      final later = await add(fund, TransactionType.compra, 5, 5);

      await repository.deleteTransaction(later);

      expect(await all(), hasLength(1));
    });

    test('A6: lançar uma amortização que não cabe é recusado', () async {
      final fund = await addAsset('FUNDO');
      await add(fund, TransactionType.compra, 10, 1);

      await expectLater(addAmort(fund, 2, 500), amortizationRefused());

      expect(await all(), hasLength(1));
    });

    ImportRow amortRow(String ticker, int day, double value) => ImportRow(
      line: day,
      date: DateTime(2026, 3, day),
      type: TransactionType.amortizacao,
      ticker: ticker,
      assetClass: AssetClass.fundo,
      currency: 'BRL',
      quantity: null,
      unitPrice: 10,
      totalValue: value,
      fee: null,
      custody: null,
      notes: null,
      lossCreditUsed: null,
    );

    test('A7: import de amortização sem cotas reverte tudo', () async {
      await addAsset('FUNDO');

      await expectLater(
        repository.applyTransactionsImport(planOf([amortRow('FUNDO', 2, 50)])),
        amortizationRefused(),
      );

      expect(await all(), isEmpty);
    });

    test('A8: import de compra e amortização que cabe vale', () async {
      await addAsset('FUNDO');

      final written = await repository.applyTransactionsImport(
        planOf([
          ImportRow(
            line: 1,
            date: DateTime(2026, 3, 1),
            type: TransactionType.compra,
            ticker: 'FUNDO',
            assetClass: AssetClass.fundo,
            currency: 'BRL',
            quantity: 10,
            unitPrice: 10,
            totalValue: 100,
            fee: null,
            custody: null,
            notes: null,
            lossCreditUsed: null,
          ),
          amortRow('FUNDO', 2, 50),
        ]),
      );

      expect(written, 2);
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

    testWidgets('S3: importar um CSV com venda acima do saldo mostra o aviso e não grava nada', (tester) async {
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
      });
      const csv = 'date,type,ticker,asset_class,currency,quantity,unit_price,total_value\n'
          '2026-03-02,venda,PETR4,acao_br,BRL,11,10,110\n';
      await tester.pumpWidget(
        localizedApp(TransactionHistoryScreen(pickCsv: () async => Uint8List.fromList(utf8.encode(csv)))),
      );
      await settle(tester);

      await tester.tap(find.byTooltip(pt.txhImportTooltip));
      await settle(tester);
      await tester.tap(find.text(pt.txhImport));
      await settle(tester);

      expect(find.text(pt.ptxCoverageSaleExceeds), findsOneWidget);
      final saved = await tester.runAsync(() => repository.listAllTransactions());
      expect(saved, hasLength(1));
    });

    testWidgets('S5: apagar a compra de que a amortização depende mostra o aviso e a linha continua', (tester) async {
      tester.view.physicalSize = const Size(900, 3000);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      await tester.runAsync(() async {
        final fund = await repository.insertAsset(
          Asset(
            ticker: 'FUNDO',
            name: 'Fundo Teste',
            assetClass: AssetClass.fundo,
            currency: 'BRL',
            createdAt: DateTime(2026, 1, 1),
          ),
        );
        await repository.insertTransaction(tx(TransactionType.compra, 10, 1, assetId: fund));
        await repository.insertTransaction(amort(2, 50, assetId: fund));
      });
      await tester.pumpWidget(localizedApp(const TransactionHistoryScreen()));
      await settle(tester);

      final buyRow = find.widgetWithText(ListTile, 'Compra — FUNDO');
      expect(buyRow, findsOneWidget);
      await tester.tap(find.descendant(of: buyRow, matching: find.byIcon(Icons.delete_outline)));
      await settle(tester);

      expect(find.text(pt.ptxAmortWorsens), findsOneWidget);
      final saved = await tester.runAsync(() => repository.listAllTransactions());
      expect(saved, hasLength(2));
    });
  });
}
