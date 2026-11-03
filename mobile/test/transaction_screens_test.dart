import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/domain/fund_tax.dart';
import 'package:anchor_mobile/l10n/app_localizations.dart';
import 'package:anchor_mobile/l10n/transaction_labels.dart';
import 'package:anchor_mobile/models/asset.dart';
import 'package:anchor_mobile/models/portfolio_transaction.dart';
import 'package:anchor_mobile/screens/add_transaction_screen.dart';
import 'package:anchor_mobile/screens/manual_asset_details_screen.dart';
import 'package:anchor_mobile/services/portfolio_repository.dart';

import 'support/localized_app.dart';

// Nova transação e detalhes do ativo manual (ADR 0001). O português é o texto que as telas
// já mostravam; o inglês é a tradução.
void main() {
  late AppLocalizations pt;
  late AppLocalizations en;
  setUpAll(() async {
    pt = await AppLocalizations.delegate.load(const Locale('pt'));
    en = await AppLocalizations.delegate.load(const Locale('en'));
  });

  group('nomes dos tipos de transação', () {
    test('pt: os nomes de antes; en: a tradução', () {
      const names = {
        TransactionType.compra: ('Compra', 'Buy'),
        TransactionType.venda: ('Venda', 'Sell'),
        TransactionType.aporte: ('Aporte', 'Deposit'),
        TransactionType.retirada: ('Retirada', 'Withdrawal'),
        TransactionType.provento: ('Provento', 'Income'),
        TransactionType.transferencia: ('Transferência', 'Transfer'),
        TransactionType.comeCotas: ('Come-cotas', 'Fund tax withholding'),
        TransactionType.amortizacao: ('Amortização', 'Amortization'),
      };
      for (final type in TransactionType.values) {
        expect(transactionTypeLabel(pt, type), names[type]!.$1, reason: type.value);
        expect(transactionTypeLabel(en, type), names[type]!.$2, reason: type.value);
      }
    });

    test('o valor gravado no banco não muda com o idioma', () {
      expect(TransactionType.provento.value, 'provento');
      expect(TransactionType.comeCotas.value, 'come_cotas');
    });
  });

  group('liquidez da renda fixa', () {
    test('cada código gravado tem nome nos dois idiomas', () {
      expect(fiLiquidityName(pt, 'diaria'), 'Diária');
      expect(fiLiquidityName(en, 'diaria'), 'Daily');
      expect(fiLiquidityName(pt, 'no_vencimento'), 'No vencimento');
      expect(fiLiquidityName(en, 'no_vencimento'), 'At maturity');
      expect(fiLiquidityName(pt, 'outro'), 'Outro');
      expect(fiLiquidityName(en, 'outro'), 'Other');
    });

    test('código desconhecido volta como veio, sem quebrar', () {
      expect(fiLiquidityName(en, 'semanal'), 'semanal');
    });
  });

  group('amortização que não cabe no histórico', () {
    PortfolioTransaction buy(DateTime date, double quantity) => PortfolioTransaction(
      id: 1,
      assetId: 1,
      type: TransactionType.compra,
      quantity: quantity,
      unitPrice: 10,
      totalValue: quantity * 10,
      date: date,
      createdAt: date,
    );

    test('sem cotas na data devolve o código noSharesOnDate', () {
      final problem = amortizationProblem(
        [buy(DateTime(2025, 6, 1), 100)],
        date: DateTime(2025, 1, 1),
        quotaValue: 10,
        amount: 50,
      );
      expect(problem?.code, AmortizationProblemCode.noSharesOnDate);
    });

    test('acima da posição devolve exceedsPosition com o valor e a posição', () {
      final problem = amortizationProblem(
        [buy(DateTime(2025, 1, 1), 100)],
        date: DateTime(2025, 2, 1),
        quotaValue: 15,
        amount: 1600,
      );
      expect(problem?.code, AmortizationProblemCode.exceedsPosition);
      expect(problem?.amount, 1600);
      expect(problem?.positionValue, 1500);
    });

    test('dentro da posição não há problema', () {
      expect(
        amortizationProblem(
          [buy(DateTime(2025, 1, 1), 100)],
          date: DateTime(2025, 2, 1),
          quotaValue: 15,
          amount: 1500,
        ),
        isNull,
      );
    });

    test('a frase segue o idioma, com o número no formato dele', () {
      const noShares = AmortizationProblem(AmortizationProblemCode.noSharesOnDate);
      const exceeds = AmortizationProblem(
        AmortizationProblemCode.exceedsPosition,
        amount: 1600,
        positionValue: 1500,
      );
      expect(
        amortizationProblemText(pt, noShares),
        'A amortização precisa de cotas em carteira nessa data.',
      );
      expect(
        amortizationProblemText(en, noShares),
        'An amortization needs fund quotas in the portfolio on that date.',
      );
      expect(
        amortizationProblemText(pt, exceeds),
        'O valor amortizado (1600,00) passa do valor da posição nessa data (1500,00).',
      );
      expect(
        amortizationProblemText(en, exceeds),
        'The amortized amount (1600.00) exceeds the position value on that date (1500.00).',
      );
    });
  });

  group('as telas', () {
    late Directory dir;
    final repository = PortfolioRepository();

    setUp(() async {
      dir = await Directory.systemTemp.createTemp('anchor-transaction-screens-');
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

    Future<int> addAsset(WidgetTester tester, AssetClass assetClass, {String currency = 'BRL'}) async {
      return (await tester.runAsync(
        () => repository.insertAsset(
          Asset(
            ticker: 'T${assetClass.index}',
            name: 'Ativo ${assetClass.index}',
            assetClass: assetClass,
            currency: currency,
            createdAt: DateTime(2026, 1, 1),
          ),
        ),
      ))!;
    }

    Future<void> chooseType(WidgetTester tester, String name) async {
      await tester.tap(find.byType(DropdownButtonFormField<TransactionType>));
      await tester.pumpAndSettle();
      await tester.tap(find.text(name).last);
      await tester.pumpAndSettle();
    }

    testWidgets('Nova transação em pt: os textos de antes', (tester) async {
      await addAsset(tester, AssetClass.acaoBr);
      await pump(tester, const AddTransactionScreen(), 'pt');

      expect(find.text('Lançar transação'), findsOneWidget);
      expect(find.text('Tipo'), findsOneWidget);
      expect(find.text('Ativo'), findsOneWidget);
      expect(find.text('Quantidade'), findsOneWidget);
      expect(find.text('Preço unitário'), findsOneWidget);
      expect(find.text('Taxa (opcional)'), findsOneWidget);
      expect(find.text('Custódia (opcional)'), findsOneWidget);
      expect(find.text('Compra'), findsOneWidget);
      expect(find.text('Salvar'), findsOneWidget);
      final now = DateTime.now();
      final day = '${now.day.toString().padLeft(2, '0')}/${now.month.toString().padLeft(2, '0')}/${now.year}';
      expect(find.text('Data: $day'), findsOneWidget);
    });

    testWidgets('Nova transação em en: rótulos, tipo e data no formato do idioma', (tester) async {
      await addAsset(tester, AssetClass.acaoBr);
      await pump(tester, const AddTransactionScreen(), 'en');

      expect(find.text('New transaction'), findsOneWidget);
      expect(find.text('Type'), findsOneWidget);
      expect(find.text('Asset'), findsOneWidget);
      expect(find.text('Quantity'), findsOneWidget);
      expect(find.text('Unit price'), findsOneWidget);
      expect(find.text('Fee (optional)'), findsOneWidget);
      expect(find.text('Custodian (optional)'), findsOneWidget);
      expect(find.text('Buy'), findsOneWidget);
      expect(find.text('Save'), findsOneWidget);
      expect(find.text('Lançar transação'), findsNothing);
      final now = DateTime.now();
      final day = '${now.month.toString().padLeft(2, '0')}/${now.day.toString().padLeft(2, '0')}/${now.year}';
      expect(find.text('Date: $day'), findsOneWidget);
    });

    testWidgets('en: sem ativo cadastrado, o aviso é em inglês', (tester) async {
      await pump(tester, const AddTransactionScreen(), 'en');
      expect(find.text('Add an asset first, in the Portfolio tab.'), findsOneWidget);
    });

    testWidgets('en: amortização sem fundo avisa em inglês; pt avisa como antes', (tester) async {
      await addAsset(tester, AssetClass.acaoBr);
      await pump(tester, const AddTransactionScreen(), 'en');
      await chooseType(tester, 'Amortization');
      expect(find.text('Add a fund asset first.'), findsOneWidget);

      await pump(tester, const AddTransactionScreen(), 'pt');
      await chooseType(tester, 'Amortização');
      expect(find.text('Cadastre um ativo da classe Fundo primeiro.'), findsOneWidget);
    });

    testWidgets('en: o fundo troca o rótulo do preço por valor da cota', (tester) async {
      await addAsset(tester, AssetClass.fundo);
      await pump(tester, const AddTransactionScreen(), 'en');
      await chooseType(tester, 'Fund tax withholding');
      expect(find.text('Quota value'), findsOneWidget);
      expect(find.textContaining('Loss credit used'), findsOneWidget);
      expect(find.text('Empty = unknown; 0 = no credit was used.'), findsOneWidget);
    });

    testWidgets('en: compra de renda fixa mostra os campos em inglês, com a liquidez traduzida', (tester) async {
      await addAsset(tester, AssetClass.rendaFixa);
      await pump(tester, const AddTransactionScreen(), 'en');

      expect(find.text('Issuer (optional)'), findsOneWidget);
      expect(find.text('Index'), findsOneWidget);
      expect(find.text('Contracted rate (%)'), findsOneWidget);
      expect(find.text('Maturity (optional)'), findsOneWidget);
      expect(find.text('Liquidity'), findsOneWidget);
      expect(find.text('Daily'), findsOneWidget);
      expect(find.text('diaria'), findsNothing);
    });

    testWidgets('en: provento de ativo em dólar mostra o imposto retido com a moeda', (tester) async {
      await addAsset(tester, AssetClass.acaoInternacional, currency: 'USD');
      await pump(tester, const AddTransactionScreen(), 'en');
      await chooseType(tester, 'Income');

      expect(find.text('Foreign tax withheld (USD, optional)'), findsOneWidget);
    });

    testWidgets('en: transferência pede custódia de origem e de destino', (tester) async {
      await addAsset(tester, AssetClass.acaoBr);
      await pump(tester, const AddTransactionScreen(), 'en');
      await chooseType(tester, 'Transfer');

      expect(find.text('Custodian (from)'), findsOneWidget);
      expect(find.text('Custodian (to)'), findsOneWidget);
    });

    testWidgets('Detalhes do imóvel em pt: os textos de antes, com a data e o valor gravados', (tester) async {
      final id = await addAsset(tester, AssetClass.imovel);
      await tester.runAsync(
        () => repository.insertAssetValuation(
          assetId: id,
          valuationDate: DateTime(2026, 1, 2),
          value: 1500.5,
          notes: 'laudo',
        ),
      );
      final asset = (await tester.runAsync(() => repository.listAssets()))!.single;
      await pump(tester, ManualAssetDetailsScreen(asset: asset), 'pt');

      expect(find.text('Detalhes do imóvel'), findsOneWidget);
      expect(find.text('Histórico de avaliação (última: 1500,50 BRL)'), findsOneWidget);
      expect(find.text('1500,50 BRL'), findsOneWidget);
      expect(find.text('02/01/2026 · laudo'), findsOneWidget);
      expect(find.text('Adicionar avaliação'), findsOneWidget);
      expect(find.text('Anexos'), findsOneWidget);
      expect(find.text('Nenhum anexo ainda.'), findsOneWidget);
      expect(find.text('Participação societária'), findsNothing);
    });

    testWidgets('Detalhes do imóvel em en: tudo em inglês, data mês/dia/ano e ponto decimal', (tester) async {
      final id = await addAsset(tester, AssetClass.imovel);
      await tester.runAsync(
        () => repository.insertAssetValuation(
          assetId: id,
          valuationDate: DateTime(2026, 1, 2),
          value: 1500.5,
          notes: 'appraisal',
        ),
      );
      final asset = (await tester.runAsync(() => repository.listAssets()))!.single;
      await pump(tester, ManualAssetDetailsScreen(asset: asset), 'en');

      expect(find.text('Property details'), findsOneWidget);
      expect(find.text('Valuation history (latest: 1500.50 BRL)'), findsOneWidget);
      expect(find.text('1500.50 BRL'), findsOneWidget);
      expect(find.text('01/02/2026 · appraisal'), findsOneWidget);
      expect(find.text('Pick a date'), findsOneWidget);
      expect(find.text('Value (BRL)'), findsOneWidget);
      expect(find.text('Notes (optional)'), findsOneWidget);
      expect(find.text('Add valuation'), findsOneWidget);
      expect(find.text('Attachments'), findsOneWidget);
      expect(find.text('No attachments yet.'), findsOneWidget);
      expect(find.text('Document type (optional)'), findsOneWidget);
      expect(find.text('Add attachment'), findsOneWidget);
    });

    testWidgets('Detalhes da empresa em en: participação, percentual e valor seguem o idioma', (tester) async {
      final id = await addAsset(tester, AssetClass.empresaNaoListada);
      await tester.runAsync(
        () => repository.updateAssetEquity(id, sharesOwned: 125, totalShares: 1000, companyValuation: 2000),
      );
      final asset = (await tester.runAsync(() => repository.listAssets()))!.single;
      await pump(tester, ManualAssetDetailsScreen(asset: asset), 'en');

      expect(find.text('Company details'), findsOneWidget);
      expect(find.text('Equity stake (12.50% — 250.00 BRL)'), findsOneWidget);
      expect(find.text('Shares owned'), findsOneWidget);
      expect(find.text('Total shares'), findsOneWidget);
      expect(find.text('Company valuation (BRL)'), findsOneWidget);
      expect(find.text('Save stake'), findsOneWidget);
      expect(find.text('No valuations recorded yet.'), findsOneWidget);
    });

    testWidgets('Detalhes da empresa em pt: participação como antes, com vírgula decimal', (tester) async {
      final id = await addAsset(tester, AssetClass.empresaNaoListada);
      await tester.runAsync(
        () => repository.updateAssetEquity(id, sharesOwned: 125, totalShares: 1000, companyValuation: 2000),
      );
      final asset = (await tester.runAsync(() => repository.listAssets()))!.single;
      await pump(tester, ManualAssetDetailsScreen(asset: asset), 'pt');

      expect(find.text('Detalhes da empresa'), findsOneWidget);
      expect(find.text('Participação societária (12,50% — 250,00 BRL)'), findsOneWidget);
      expect(find.text('Cotas/ações possuídas'), findsOneWidget);
      expect(find.text('Salvar participação'), findsOneWidget);
    });
  });
}
