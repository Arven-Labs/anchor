import 'dart:io';

import 'package:material_ui/material_ui.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/domain/transactions_csv_import.dart';
import 'package:anchor_mobile/l10n/app_localizations.dart';
import 'package:anchor_mobile/l10n/transaction_labels.dart';
import 'package:anchor_mobile/models/asset.dart';
import 'package:anchor_mobile/models/custodia.dart';
import 'package:anchor_mobile/models/portfolio_transaction.dart';
import 'package:anchor_mobile/screens/transaction_history_screen.dart';
import 'package:anchor_mobile/services/portfolio_repository.dart';

import 'support/localized_app.dart';

// Histórico de transações e importação de CSV (ADR 0001). O português é o texto que a tela
// já mostrava; o inglês é a tradução. Os problemas de importação viram código + argumentos.
void main() {
  late AppLocalizations pt;
  late AppLocalizations en;
  setUpAll(() async {
    pt = await AppLocalizations.delegate.load(const Locale('pt'));
    en = await AppLocalizations.delegate.load(const Locale('en'));
  });

  group('cada problema do CSV tem um código', () {
    ParsedTransactionsCsv parse(String csv) => parseTransactionsCsv(csv);
    ImportIssue only(ParsedTransactionsCsv p) => p.issues.single;

    ImportIssue plan(String csv) => planTransactionsImport(
      parseTransactionsCsv(csv),
      assets: const [],
      custodias: const [],
      transactions: const [],
    ).issues.single;

    test('arquivo vazio', () {
      expect(only(parse('')).code, ImportIssueCode.emptyFile);
      expect(only(parse('')).line, 0);
    });

    test('coluna obrigatória ausente leva o nome da coluna', () {
      final issue = only(parse('foo,bar\n1,2\n'));
      expect(issue.code, ImportIssueCode.missingColumn);
      expect(issue.args, {'name': 'date'});
    });

    test('data inválida leva o texto digitado', () {
      final issue = only(parse('date,type,total_value\n2026-13-45,compra,10\n'));
      expect(issue.code, ImportIssueCode.invalidDate);
      expect(issue.args, {'raw': '2026-13-45'});
      expect(issue.line, 1);
    });

    test('tipo desconhecido leva o texto digitado', () {
      final issue = only(parse('date,type,total_value\n2026-03-10,xyz,10\n'));
      expect(issue.code, ImportIssueCode.unknownType);
      expect(issue.args, {'raw': 'xyz'});
    });

    test('número inválido leva a coluna e o texto', () {
      final issue = only(parse('date,type,total_value,fee\n2026-03-10,compra,10,abc\n'));
      expect(issue.code, ImportIssueCode.invalidNumber);
      expect(issue.args, {'name': 'fee', 'raw': 'abc'});
    });

    test('total vazio', () {
      final issue = only(parse('date,type,total_value\n2026-03-10,compra,\n'));
      expect(issue.code, ImportIssueCode.totalEmpty);
    });

    test('classe desconhecida leva o texto', () {
      final issue = only(
        parse('date,type,ticker,asset_class,total_value\n2026-03-10,compra,X,foo,10\n'),
      );
      expect(issue.code, ImportIssueCode.unknownClass);
      expect(issue.args, {'raw': 'foo'});
    });

    test('ticker obrigatório leva o tipo no vocabulário do CSV', () {
      final issue = plan('date,type,ticker,total_value\n2026-03-10,provento,,10\n');
      expect(issue.code, ImportIssueCode.tickerRequired);
      expect(issue.args, {'type': 'provento'});
    });

    test('ativo novo sem classe leva o ticker', () {
      final issue = plan('date,type,ticker,quantity,unit_price,total_value\n2026-03-10,compra,XYZ1,1,10,10\n');
      expect(issue.code, ImportIssueCode.newAssetNeedsClass);
      expect(issue.args, {'ticker': 'XYZ1'});
    });

    test('tipo só de fundo em outra classe leva o tipo', () {
      final issue = plan(
        'date,type,ticker,asset_class,quantity,unit_price,total_value\n'
        '2026-03-10,come_cotas,PETR4,acao_br,1,10,10\n',
      );
      expect(issue.code, ImportIssueCode.fundOnly);
      expect(issue.args, {'type': 'come_cotas'});
    });

    test('quantidade e preço obrigatórios levam o tipo', () {
      final qty = plan(
        'date,type,ticker,asset_class,unit_price,total_value\n2026-03-10,compra,PETR4,acao_br,10,10\n',
      );
      expect(qty.code, ImportIssueCode.quantityRequired);
      expect(qty.args, {'type': 'compra'});
      final price = plan(
        'date,type,ticker,asset_class,quantity,total_value\n2026-03-10,compra,PETR4,acao_br,1,10\n',
      );
      expect(price.code, ImportIssueCode.unitPriceRequired);
    });

    test('a mensagem de quem depura é em inglês e cita o dado', () {
      final issue = only(parse('foo,bar\n1,2\n'));
      expect(issue.message, contains('date'));
    });
  });

  group('a frase do problema segue o idioma', () {
    test('pt: as frases de antes; en: a tradução', () {
      const cases = <(ImportIssue, String, String)>[
        (
          ImportIssue(0, ImportIssueCode.emptyFile),
          'arquivo vazio',
          'empty file',
        ),
        (
          ImportIssue(0, ImportIssueCode.missingColumn, {'name': 'date'}),
          "coluna 'date' ausente no cabeçalho",
          "column 'date' missing from the header",
        ),
        (
          ImportIssue(2, ImportIssueCode.invalidDate, {'raw': '31/02/2026'}),
          "data inválida: '31/02/2026' (esperado AAAA-MM-DD ou DD/MM/AAAA)",
          "invalid date: '31/02/2026' (expected YYYY-MM-DD or DD/MM/YYYY)",
        ),
        (
          ImportIssue(2, ImportIssueCode.unknownType, {'raw': 'xyz'}),
          "tipo desconhecido: 'xyz'",
          "unknown type: 'xyz'",
        ),
        (
          ImportIssue(2, ImportIssueCode.invalidNumber, {'name': 'fee', 'raw': 'abc'}),
          "número inválido em 'fee': 'abc'",
          "invalid number in 'fee': 'abc'",
        ),
        (
          ImportIssue(2, ImportIssueCode.totalEmpty),
          'total_value vazio',
          'total_value is empty',
        ),
        (
          ImportIssue(2, ImportIssueCode.unknownClass, {'raw': 'foo'}),
          "classe desconhecida: 'foo'",
          "unknown class: 'foo'",
        ),
        (
          ImportIssue(2, ImportIssueCode.tickerRequired, {'type': 'provento'}),
          'ticker obrigatório para provento',
          'ticker required for provento',
        ),
        (
          ImportIssue(2, ImportIssueCode.newAssetNeedsClass, {'ticker': 'XYZ1'}),
          "ativo novo 'XYZ1': informe a asset_class",
          "new asset 'XYZ1': provide asset_class",
        ),
        (
          ImportIssue(2, ImportIssueCode.fundOnly, {'type': 'come_cotas'}),
          'come_cotas só vale para ativo da classe fundo',
          'come_cotas only applies to fund-class assets',
        ),
        (
          ImportIssue(2, ImportIssueCode.quantityRequired, {'type': 'compra'}),
          'quantity obrigatória para compra',
          'quantity required for compra',
        ),
        (
          ImportIssue(2, ImportIssueCode.unitPriceRequired, {'type': 'compra'}),
          'unit_price obrigatório para compra',
          'unit_price required for compra',
        ),
      ];
      for (final (issue, ptText, enText) in cases) {
        expect(importIssueText(pt, issue), ptText, reason: issue.code.name);
        expect(importIssueText(en, issue), enText, reason: issue.code.name);
      }
    });
  });

  group('a tela', () {
    late Directory dir;
    final repository = PortfolioRepository();

    setUp(() async {
      dir = await Directory.systemTemp.createTemp('anchor-history-');
      await databaseFactory.setDatabasesPath(dir.path);
      await AppDatabase.resetForTest();
    });

    tearDown(() async {
      await AppDatabase.resetForTest();
      await dir.delete(recursive: true);
    });

    Future<void> settle(WidgetTester tester) async {
      for (var i = 0; i < 40; i++) {
        await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
        await tester.pump();
        if (find.byType(CircularProgressIndicator).evaluate().isEmpty) break;
      }
    }

    Future<void> pump(WidgetTester tester, Widget screen, String locale) async {
      tester.view.physicalSize = const Size(900, 3000);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      await tester.runAsync(AppDatabase.instance);
      await tester.pumpWidget(localizedApp(screen, locale: locale));
      await settle(tester);
    }

    Future<void> seed(WidgetTester tester) async {
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
        final aapl = await repository.insertAsset(
          Asset(
            ticker: 'AAPL',
            name: 'Apple',
            assetClass: AssetClass.acaoInternacional,
            currency: 'USD',
            createdAt: DateTime(2026, 1, 1),
          ),
        );
        final fund = await repository.insertAsset(
          Asset(
            ticker: 'FUNDX',
            name: 'Fundo X',
            assetClass: AssetClass.fundo,
            currency: 'BRL',
            createdAt: DateTime(2026, 1, 1),
          ),
        );
        final xp = await repository.insertCustodia(
          Custodia(instituicao: 'XP', titular: 'Eu', createdAt: DateTime(2026, 1, 1)),
        );
        await repository.insertTransaction(
          PortfolioTransaction(
            assetId: petr,
            type: TransactionType.compra,
            quantity: 100,
            unitPrice: 30.5,
            totalValue: 3050,
            fee: 4.9,
            custodiaId: xp,
            date: DateTime(2026, 3, 10),
            createdAt: DateTime(2026, 3, 10),
          ),
        );
        await repository.insertTransaction(
          PortfolioTransaction(
            assetId: aapl,
            type: TransactionType.provento,
            totalValue: 12,
            foreignTaxWithheld: 1.5,
            foreignTaxPaidOn: DateTime(2026, 5, 1),
            date: DateTime(2026, 5, 20),
            createdAt: DateTime(2026, 5, 20),
          ),
        );
        await repository.insertTransaction(
          PortfolioTransaction(
            assetId: fund,
            type: TransactionType.comeCotas,
            quantity: 2.5,
            unitPrice: 10,
            totalValue: 25,
            lossCreditUsed: 2.5,
            date: DateTime(2026, 5, 31),
            createdAt: DateTime(2026, 5, 31),
          ),
        );
      });
    }

    testWidgets('pt: os textos de antes, com vírgula decimal', (tester) async {
      await seed(tester);
      await pump(tester, const TransactionHistoryScreen(), 'pt');

      expect(find.text('Histórico'), findsOneWidget);
      expect(find.byTooltip('Importar CSV do desktop'), findsOneWidget);
      expect(find.byTooltip('Exportar CSV para o desktop'), findsOneWidget);
      expect(find.text('Compra — PETR4'), findsOneWidget);
      expect(
        find.text('10/03/2026 · 100 un. · XP (Eu) · 3050,00 · taxa 4,90'),
        findsOneWidget,
      );
      expect(find.text('Provento — AAPL'), findsOneWidget);
      expect(
        find.text('20/05/2026 · 12,00 · retido 1,50 (pago em 01/05/2026)'),
        findsOneWidget,
      );
      expect(find.text('Come-cotas — FUNDX'), findsOneWidget);
      expect(
        find.text('31/05/2026 · 2,5 un. · 25,00 · crédito usado 2,50'),
        findsOneWidget,
      );
    });

    testWidgets('en: tudo em inglês, data mês/dia/ano e ponto decimal', (tester) async {
      await seed(tester);
      await pump(tester, const TransactionHistoryScreen(), 'en');

      expect(find.text('History'), findsOneWidget);
      expect(find.byTooltip('Import CSV from desktop'), findsOneWidget);
      expect(find.byTooltip('Export CSV to desktop'), findsOneWidget);
      expect(find.text('Buy — PETR4'), findsOneWidget);
      expect(
        find.text('03/10/2026 · 100 units · XP (Eu) · 3050.00 · fee 4.90'),
        findsOneWidget,
      );
      expect(find.text('Income — AAPL'), findsOneWidget);
      expect(
        find.text('05/20/2026 · 12.00 · withheld 1.50 (paid on 05/01/2026)'),
        findsOneWidget,
      );
      expect(find.text('Fund tax withholding — FUNDX'), findsOneWidget);
      expect(
        find.text('05/31/2026 · 2.5 units · 25.00 · loss credit used 2.50'),
        findsOneWidget,
      );
    });

    testWidgets('sem transações, o aviso segue o idioma', (tester) async {
      await pump(tester, const TransactionHistoryScreen(), 'pt');
      expect(find.text('Nenhuma transação lançada ainda.'), findsOneWidget);

      await pump(tester, const TransactionHistoryScreen(), 'en');
      expect(find.text('No transactions logged yet.'), findsOneWidget);
    });

    ImportPlan bigPlan() => const ImportPlan(
      rows: [],
      duplicates: 2,
      skipped: 1,
      issues: [
        ImportIssue(1, ImportIssueCode.totalEmpty),
        ImportIssue(2, ImportIssueCode.unknownType, {'raw': 'xyz'}),
        ImportIssue(3, ImportIssueCode.totalEmpty),
        ImportIssue(4, ImportIssueCode.totalEmpty),
        ImportIssue(5, ImportIssueCode.totalEmpty),
        ImportIssue(6, ImportIssueCode.totalEmpty),
        ImportIssue(7, ImportIssueCode.totalEmpty),
      ],
      newAssets: [
        NewAssetPlan(ticker: 'A', assetClass: AssetClass.acaoBr, currency: 'BRL'),
      ],
      newCustodias: ['XP', 'Rico'],
    );

    testWidgets('prévia da importação em pt: as frases de antes', (tester) async {
      await pump(tester, Scaffold(body: ImportPreviewDialog(plan: bigPlan())), 'pt');

      expect(find.text('Importar do desktop'), findsOneWidget);
      expect(find.text('0 transações novas'), findsOneWidget);
      expect(find.text('1 ativo novo'), findsOneWidget);
      expect(find.text('2 custódias novas'), findsOneWidget);
      expect(find.text('2 já existem (ignoradas)'), findsOneWidget);
      expect(
        find.text('1 transferência ignorada (o CSV não traz o destino)'),
        findsOneWidget,
      );
      expect(find.text('7 linhas com problema (não entram):'), findsOneWidget);
      expect(find.text("linha 2: tipo desconhecido: 'xyz'"), findsOneWidget);
      expect(find.text('… e mais 2'), findsOneWidget);
      expect(find.text('Cancelar'), findsOneWidget);
      expect(find.text('Importar'), findsOneWidget);
    });

    testWidgets('prévia da importação em en: singular e plural certos', (tester) async {
      await pump(tester, Scaffold(body: ImportPreviewDialog(plan: bigPlan())), 'en');

      expect(find.text('Import from desktop'), findsOneWidget);
      expect(find.text('0 new transactions'), findsOneWidget);
      expect(find.text('1 new asset'), findsOneWidget);
      expect(find.text('2 new custodians'), findsOneWidget);
      expect(find.text('2 already exist (ignored)'), findsOneWidget);
      expect(find.text('1 transfer skipped (the CSV has no destination)'), findsOneWidget);
      expect(find.text('7 rows with problems (not imported):'), findsOneWidget);
      expect(find.text("line 2: unknown type: 'xyz'"), findsOneWidget);
      expect(find.text('… and 2 more'), findsOneWidget);
      expect(find.text('Cancel'), findsOneWidget);
      expect(find.text('Import'), findsOneWidget);
    });
  });
}
