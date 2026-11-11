import 'dart:io';

import 'package:material_ui/material_ui.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/domain/b3_tax.dart';
import 'package:anchor_mobile/l10n/app_localizations.dart';
import 'package:anchor_mobile/l10n/b3_tax_labels.dart';
import 'package:anchor_mobile/models/asset.dart';
import 'package:anchor_mobile/models/portfolio_transaction.dart';
import 'package:anchor_mobile/screens/b3_tax_screen.dart';
import 'package:anchor_mobile/services/portfolio_repository.dart';

import 'support/localized_app.dart';

// Textos e tela do IR de B3 (ADR 0001). Dados fictícios.
Future<AppLocalizations> _load(String code) => AppLocalizations.delegate.load(Locale(code));

void main() {
  late AppLocalizations en;
  late AppLocalizations pt;

  setUpAll(() async {
    en = await _load('en');
    pt = await _load('pt');
  });

  B3Month month({
    bool stockExempt = false,
    double exemptGain = 0,
    double due = 0,
    double carriedOut = 0,
  }) => B3Month(
    period: '2024-02',
    stockSalesTotal: 18000,
    stockExempt: stockExempt,
    exemptGain: exemptGain,
    buckets: const [],
    grossTax: due,
    irrfWithheld: 0,
    irrfUsed: 0,
    carriedIn: 0,
    due: due,
    carriedOut: carriedOut,
    dueDate: DateTime.utc(2024, 3, 28),
    darfCode: 6015,
    sales: const [],
  );

  group('situação do mês', () {
    test('pt: DARF, abaixo do mínimo, isento e sem imposto', () {
      expect(b3MonthStatus(pt, month(due: 1492.05)), 'DARF 6015 de R\$ 1.492,05 até 28/03/2024');
      expect(
        b3MonthStatus(pt, month(carriedOut: 6)),
        'Imposto de R\$ 6,00 abaixo do mínimo de R\$ 10,00: soma ao mês seguinte',
      );
      expect(
        b3MonthStatus(pt, month(stockExempt: true, exemptGain: 8000)),
        'Isento: vendas de ações até R\$ 20.000,00 no mês',
      );
      expect(b3MonthStatus(pt, month()), 'Sem imposto: nada a pagar no mês');
    });

    test('en: as quatro situações, com valores e data no formato en', () {
      expect(b3MonthStatus(en, month(due: 1492.05)), 'DARF 6015 of R\$ 1,492.05 due 03/28/2024');
      expect(
        b3MonthStatus(en, month(carriedOut: 6)),
        'Tax of R\$ 6.00 is below the R\$ 10.00 minimum: added to next month',
      );
      expect(
        b3MonthStatus(en, month(stockExempt: true, exemptGain: 8000)),
        'Exempt: stock sales up to R\$ 20,000.00 in the month',
      );
      expect(b3MonthStatus(en, month()), 'No tax: nothing to pay this month');
    });
  });

  test('gavetas e aviso de custo desconhecido têm texto nos dois idiomas', () {
    expect(b3BucketName(pt, B3Bucket.common), 'Comum');
    expect(b3BucketName(pt, B3Bucket.dayTrade), 'Day trade');
    expect(b3BucketName(en, B3Bucket.fii), 'REITs (FII)');
    final warning = B3TaxWarning.costUnknown(ticker: 'PETR4', date: DateTime(2024, 2, 10));
    expect(
      b3WarningText(en, warning),
      'Sale of PETR4 on 02/10/2024 exceeds the purchases entered: the gain counts only the '
      'part with known cost. Enter the missing purchases.',
    );
    expect(
      b3WarningText(pt, warning),
      'Venda de PETR4 em 10/02/2024 maior que as compras lançadas: o ganho considera só a '
      'parte com custo conhecido. Lance as compras que faltam.',
    );
  });

  // A tela montada de verdade (SQLite ffi, sem rede: a apuração não busca nada).
  group('a tela', () {
    late Directory dir;
    final repository = PortfolioRepository();

    setUp(() async {
      dir = await Directory.systemTemp.createTemp('anchor-b3-tax-screen-');
      await databaseFactory.setDatabasesPath(dir.path);
      await AppDatabase.resetForTest();
    });

    tearDown(() async {
      await AppDatabase.resetForTest();
      await dir.delete(recursive: true);
    });

    // O exemplo da Receita: ganho de 9.967, imposto de 1.495,05, IRRF de 3,00 e DARF de 1.492,05.
    Future<void> seedRevenueExample() async {
      final id = await repository.insertAsset(
        Asset(
          ticker: 'PETR4',
          name: 'Petrobras',
          assetClass: AssetClass.acaoBr,
          currency: 'BRL',
          createdAt: DateTime(2024, 1, 1),
        ),
      );
      Future<int> add(TransactionType type, String date, double total, double fee) =>
          repository.insertTransaction(
            PortfolioTransaction(
              assetId: id,
              type: type,
              quantity: 1000,
              unitPrice: total / 1000,
              totalValue: total,
              fee: fee,
              date: DateTime.parse(date),
              createdAt: DateTime.parse(date),
            ),
          );
      await add(TransactionType.compra, '2024-01-10', 50000, 15);
      await add(TransactionType.venda, '2024-02-15', 60000, 18);
    }

    Future<void> pumpScreen(WidgetTester tester, String locale, {bool seed = false}) async {
      tester.view.physicalSize = const Size(900, 3000);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      await tester.runAsync(AppDatabase.instance);
      if (seed) await tester.runAsync(seedRevenueExample);
      await tester.pumpWidget(localizedApp(const B3TaxScreen(), locale: locale));
      for (var i = 0; i < 200; i++) {
        await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
        await tester.pump();
        if (find.byType(CircularProgressIndicator).evaluate().isEmpty) break;
      }
      expect(find.byType(CircularProgressIndicator), findsNothing, reason: 'a tela não terminou de carregar');
    }

    testWidgets('sem vendas: título, aviso e vazio nos dois idiomas', (tester) async {
      await pumpScreen(tester, 'pt');
      expect(find.text('IR de B3'), findsOneWidget);
      expect(find.text(pt.b3TaxDisclaimer), findsOneWidget);
      expect(find.text('Nenhuma venda na B3 ainda.'), findsOneWidget);

      await pumpScreen(tester, 'en');
      expect(find.text('B3 tax'), findsOneWidget);
      expect(find.text(en.b3TaxDisclaimer), findsOneWidget);
      expect(find.text('No sales on B3 yet.'), findsOneWidget);
      expect(find.textContaining('Nenhuma'), findsNothing);
    });

    testWidgets('pt: o exemplo da Receita mostra o DARF, o IRRF e a gaveta comum', (tester) async {
      await pumpScreen(tester, 'pt', seed: true);

      expect(find.text('02/2024'), findsOneWidget);
      expect(find.text('DARF 6015 de R\$ 1.492,05 até 28/03/2024'), findsOneWidget);
      expect(find.text('Vendas de ações (swing): R\$ 60.000,00'), findsOneWidget);
      expect(
        find.text('Comum: resultado R\$ 9.967,00, prejuízo compensado R\$ 0,00, imposto R\$ 1.495,05 (15%)'),
        findsOneWidget,
      );
      expect(find.text('IRRF retido (estimado): R\$ 3,00'), findsOneWidget);
    });

    testWidgets('en: o mesmo mês em inglês, sem sobra de português', (tester) async {
      await pumpScreen(tester, 'en', seed: true);

      expect(find.text('DARF 6015 of R\$ 1,492.05 due 03/28/2024'), findsOneWidget);
      expect(find.text('Withholding tax (estimated): R\$ 3.00'), findsOneWidget);
      for (final pt in ['Vendas', 'Comum', 'retido']) {
        expect(find.textContaining(pt), findsNothing, reason: pt);
      }
    });
  });
}
