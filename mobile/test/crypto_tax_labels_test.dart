import 'dart:io';

import 'package:material_ui/material_ui.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/domain/crypto_tax.dart';
import 'package:anchor_mobile/domain/tax_format.dart';
import 'package:anchor_mobile/l10n/app_localizations.dart';
import 'package:anchor_mobile/l10n/crypto_tax_labels.dart';
import 'package:anchor_mobile/models/asset.dart';
import 'package:anchor_mobile/models/crypto_swap.dart';
import 'package:anchor_mobile/models/portfolio_transaction.dart';
import 'package:anchor_mobile/screens/crypto_swap_screen.dart';
import 'package:anchor_mobile/screens/crypto_tax_screen.dart';
import 'package:anchor_mobile/services/portfolio_repository.dart';

import 'support/localized_app.dart';

// Textos do IR de cripto e da troca de cripto (ADR 0001). O português é o texto que o app já
// mostrava; o inglês usa R$ com separadores en e a data como MM/DD/YYYY.
Future<AppLocalizations> _load(String code) => AppLocalizations.delegate.load(Locale(code));

void main() {
  late AppLocalizations en;
  late AppLocalizations pt;

  setUpAll(() async {
    en = await _load('en');
    pt = await _load('pt');
  });

  CryptoMonth month({
    bool exempt = false,
    double tax = 0,
    double carriedIn = 0,
    double due = 0,
    double carriedOut = 0,
  }) => CryptoMonth(
    period: '2026-02',
    salesTotal: 40000,
    exempt: exempt,
    gain: 30000,
    rate: 0.15,
    tax: tax,
    carriedIn: carriedIn,
    due: due,
    carriedOut: carriedOut,
    dueDate: DateTime.utc(2026, 3, 31),
    darfCode: 4600,
    sales: const [],
  );

  group('formatadores por idioma', () {
    test('reais: pt com vírgula decimal, en com ponto; mesmo arredondamento e sinal', () {
      expect(formatBrl(1234.5, locale: 'pt'), 'R\$ 1.234,50');
      expect(formatBrl(1234.5, locale: 'en'), 'R\$ 1,234.50');
      expect(formatBrl(5500000.499, locale: 'en'), 'R\$ 5,500,000.50');
      expect(formatBrl(-10000, locale: 'en'), '-R\$ 10,000.00');
      expect(formatBrl(-0.001, locale: 'en'), 'R\$ 0.00', reason: 'sem sinal em zero');
    });

    test('sem locale o formato segue pt (telas de fundos e exterior ainda não migradas)', () {
      expect(formatBrl(1234.5, locale: 'pt'), 'R\$ 1.234,50');
      expect(formatDay(DateTime.utc(2026, 3, 5), locale: 'pt'), '05/03/2026');
    });

    test('data: dia/mês/ano em pt, mês/dia/ano em en', () {
      expect(formatDay(DateTime.utc(2026, 3, 5), locale: 'pt'), '05/03/2026');
      expect(formatDay(DateTime.utc(2026, 3, 5), locale: 'en'), '03/05/2026');
    });
  });

  group('situação do mês', () {
    test('pt: o texto de antes', () {
      expect(
        cryptoMonthStatus(pt, month(exempt: true)),
        'Isento: vendas até R\$ 35.000,00 no mês',
      );
      expect(
        cryptoMonthStatus(pt, month(tax: 4500, due: 4500)),
        'DARF 4600 de R\$ 4.500,00 até 31/03/2026',
      );
      expect(
        cryptoMonthStatus(pt, month(tax: 6, carriedOut: 6)),
        'Imposto de R\$ 6,00 abaixo do mínimo de R\$ 10,00: soma ao mês seguinte',
      );
      expect(cryptoMonthStatus(pt, month()), 'Sem imposto: o mês fechou sem ganho');
    });

    test('en: as quatro situações, com valores e data no formato en', () {
      expect(
        cryptoMonthStatus(en, month(exempt: true)),
        'Exempt: sales up to R\$ 35,000.00 in the month',
      );
      expect(
        cryptoMonthStatus(en, month(tax: 4500, due: 4500)),
        'DARF 4600 of R\$ 4,500.00 due 03/31/2026',
      );
      expect(
        cryptoMonthStatus(en, month(tax: 6, carriedOut: 6)),
        'Tax of R\$ 6.00 is below the R\$ 10.00 minimum: added to next month',
      );
      expect(cryptoMonthStatus(en, month()), 'No tax: the month closed with no gain');
    });
  });

  group('avisos', () {
    final costUnknown = CryptoTaxWarning.costUnknown(ticker: 'BTC', date: DateTime(2026, 2, 10));
    const salesNonBrl = CryptoTaxWarning.salesNonBrl(period: '2026-02');
    const fxMissing = CryptoTaxWarning.fxMissing(period: '2026-02');

    test('pt: o texto de antes', () {
      expect(
        cryptoWarningText(pt, costUnknown),
        'Venda de BTC em 10/02/2026 maior que as compras lançadas: o ganho considera só a '
        'parte com custo conhecido. Lance as compras que faltam.',
      );
      expect(
        cryptoWarningText(pt, salesNonBrl),
        'Vendas de cripto fora de BRL em 02/2026 não são apuradas aqui (regra de exterior).',
      );
      expect(
        cryptoWarningText(pt, fxMissing),
        'Venda de cripto em USD em 02/2026 sem câmbio para converter: ficou de fora da apuração.',
      );
    });

    test('en: nomeia a moeda, a data (MM/DD/YYYY) e o período', () {
      expect(
        cryptoWarningText(en, costUnknown),
        'Sale of BTC on 02/10/2026 exceeds the purchases entered: the gain counts only the '
        'part with known cost. Enter the missing purchases.',
      );
      expect(
        cryptoWarningText(en, salesNonBrl),
        'Crypto sales not in BRL in 02/2026 are not assessed here (foreign rules).',
      );
      expect(
        cryptoWarningText(en, fxMissing),
        'USD crypto sale in 02/2026 has no exchange rate to convert: left out of the assessment.',
      );
    });
  });

  group('erros da troca por código', () {
    CryptoSwapException error(CryptoSwapErrorCode code, [Map<String, String> args = const {}]) =>
        CryptoSwapException(code, args);

    const args = {'ticker': 'SOL', 'currency': 'USD', 'available': '1.5', 'requested': '2'};

    test('todo código tem texto nos dois idiomas, sem placeholder sobrando', () {
      for (final code in CryptoSwapErrorCode.values) {
        for (final l in [en, pt]) {
          final text = cryptoSwapErrorText(l, error(code, args));
          expect(text, isNotEmpty, reason: '$code ${l.localeName}');
          expect(text, isNot(contains('{')), reason: '$code ${l.localeName}');
        }
      }
    });

    test('pt: as frases de antes', () {
      String t(CryptoSwapErrorCode c) => cryptoSwapErrorText(pt, error(c, args));

      expect(t(CryptoSwapErrorCode.needTwoDifferentCoins), 'A permuta precisa de duas moedas diferentes.');
      expect(t(CryptoSwapErrorCode.notCrypto), 'SOL não é cripto; a permuta só vale entre criptomoedas.');
      expect(t(CryptoSwapErrorCode.notBrl), 'SOL está em USD; a permuta é apurada em reais.');
      expect(t(CryptoSwapErrorCode.quantityOutMustBePositive), 'O campo quantidade que sai deve ser maior que zero.');
      expect(t(CryptoSwapErrorCode.quantityInMustBePositive), 'O campo quantidade que entra deve ser maior que zero.');
      expect(t(CryptoSwapErrorCode.valueMustBePositive), 'O campo valor deve ser maior que zero.');
      expect(t(CryptoSwapErrorCode.negativeFee), 'A taxa não pode ser negativa.');
      expect(
        t(CryptoSwapErrorCode.insufficientBalance),
        'Saldo insuficiente de SOL: tem 1.5 e a troca entrega 2.',
      );
    });

    test('en: frases equivalentes', () {
      String t(CryptoSwapErrorCode c) => cryptoSwapErrorText(en, error(c, args));

      expect(t(CryptoSwapErrorCode.needTwoDifferentCoins), 'A swap needs two different coins.');
      expect(t(CryptoSwapErrorCode.notCrypto), 'SOL is not crypto; a swap only works between cryptocurrencies.');
      expect(t(CryptoSwapErrorCode.notBrl), 'SOL is in USD; swaps are assessed in reais.');
      expect(t(CryptoSwapErrorCode.quantityOutMustBePositive), 'The quantity out must be greater than zero.');
      expect(t(CryptoSwapErrorCode.quantityInMustBePositive), 'The quantity in must be greater than zero.');
      expect(t(CryptoSwapErrorCode.valueMustBePositive), 'The value must be greater than zero.');
      expect(t(CryptoSwapErrorCode.negativeFee), "The fee can't be negative.");
      expect(
        t(CryptoSwapErrorCode.insufficientBalance),
        'Insufficient SOL balance: you have 1.5 and the swap gives 2.',
      );
    });

    test('a mensagem da exceção é inglês para quem depura', () {
      final e = error(CryptoSwapErrorCode.negativeFee);
      expect(e.toString(), isNot(contains('taxa')));
      expect(e.code, CryptoSwapErrorCode.negativeFee);
    });
  });

  // As telas montadas de verdade (SQLite ffi, sem rede).
  group('as telas', () {
    late Directory dir;

    setUp(() async {
      dir = await Directory.systemTemp.createTemp('anchor-crypto-tax-screen-');
      await databaseFactory.setDatabasesPath(dir.path);
      await AppDatabase.resetForTest();
    });

    tearDown(() async {
      await AppDatabase.resetForTest();
      await dir.delete(recursive: true);
    });

    Future<void> settle(WidgetTester tester) async {
      for (var i = 0; i < 100; i++) {
        await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
        await tester.pump();
        if (find.byType(CircularProgressIndicator).evaluate().isEmpty) break;
      }
    }

    Future<void> pump(WidgetTester tester, Widget screen, String locale) async {
      tester.view.physicalSize = const Size(800, 3000);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      await tester.runAsync(AppDatabase.instance);
      await tester.pumpWidget(localizedApp(screen, locale: locale));
      await settle(tester);
    }

    Future<void> seedSale() async {
      final repo = PortfolioRepository();
      final id = await repo.insertAsset(
        Asset(
          ticker: 'BTC',
          name: 'Bitcoin',
          assetClass: AssetClass.cripto,
          currency: 'BRL',
          createdAt: DateTime(2026, 1, 1),
        ),
      );
      PortfolioTransaction tx(TransactionType type, DateTime date, double gross) => PortfolioTransaction(
        assetId: id,
        type: type,
        quantity: 1,
        unitPrice: gross,
        totalValue: gross,
        date: date,
        createdAt: DateTime(2026, 1, 1),
      );
      await repo.insertTransactions([
        tx(TransactionType.compra, DateTime(2026, 1, 10), 10000),
        tx(TransactionType.venda, DateTime(2026, 2, 10), 40000),
      ]);
    }

    testWidgets('IR de cripto sem vendas: título e vazio nos dois idiomas', (tester) async {
      await pump(tester, const CryptoTaxScreen(), 'pt');
      expect(find.text('IR de cripto'), findsOneWidget);
      expect(find.text('Nenhuma venda de cripto em reais ainda.'), findsOneWidget);

      await pump(tester, const CryptoTaxScreen(), 'en');
      expect(find.text('Crypto tax'), findsOneWidget);
      expect(find.text('No crypto sales in reais yet.'), findsOneWidget);
    });

    testWidgets('IR de cripto com uma venda: o cartão do mês em en', (tester) async {
      await tester.runAsync(() async {
        await AppDatabase.instance();
        await seedSale();
      });
      await pump(tester, const CryptoTaxScreen(), 'en');

      expect(find.text('02/2026'), findsOneWidget);
      expect(find.text('DARF 4600 of R\$ 4,500.00 due 03/31/2026'), findsOneWidget);
      expect(find.text('Month sales: R\$ 40,000.00'), findsOneWidget);
      expect(find.text('Month gain: R\$ 30,000.00'), findsOneWidget);
      expect(find.text('Tax: R\$ 4,500.00 (rate 15.00%)'), findsOneWidget);
      expect(find.text('Sales (1)'), findsOneWidget);
      for (final pt in ['Vendas', 'Ganho', 'Imposto', 'alíquota', 'Calculado']) {
        expect(find.textContaining(pt), findsNothing, reason: pt);
      }
    });

    testWidgets('IR de cripto com uma venda: o mesmo cartão em pt', (tester) async {
      await tester.runAsync(() async {
        await AppDatabase.instance();
        await seedSale();
      });
      await pump(tester, const CryptoTaxScreen(), 'pt');

      expect(find.text('DARF 4600 de R\$ 4.500,00 até 31/03/2026'), findsOneWidget);
      expect(find.text('Vendas do mês: R\$ 40.000,00'), findsOneWidget);
      expect(find.text('Ganho do mês: R\$ 30.000,00'), findsOneWidget);
      expect(find.text('Imposto: R\$ 4.500,00 (alíquota 15,00%)'), findsOneWidget);
      expect(find.text('Vendas (1)'), findsOneWidget);
    });

    testWidgets('trocar cripto sem moedas suficientes: título e aviso nos dois idiomas', (tester) async {
      await pump(tester, const CryptoSwapScreen(), 'pt');
      expect(find.text('Trocar cripto'), findsOneWidget);
      expect(find.text('É preciso ter saldo em uma cripto e outra cadastrada.'), findsOneWidget);

      await pump(tester, const CryptoSwapScreen(), 'en');
      expect(find.text('Swap crypto'), findsOneWidget);
      expect(find.text('You need a balance in one crypto and another one registered.'), findsOneWidget);
    });
  });
}
