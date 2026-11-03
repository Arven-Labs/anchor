import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/domain/foreign_tax.dart';
import 'package:anchor_mobile/domain/ptax_series.dart';
import 'package:anchor_mobile/domain/tax_format.dart';
import 'package:anchor_mobile/l10n/app_localizations.dart';
import 'package:anchor_mobile/l10n/foreign_tax_labels.dart' as labels;
import 'package:anchor_mobile/screens/foreign_tax_screen.dart';

import 'support/localized_app.dart';

// Textos do IR de exterior (ADR 0001). O português é o texto que o app já mostrava; o inglês
// usa R$ com separadores en, data MM/DD/YYYY e quantidade com ponto decimal.
void main() {
  late AppLocalizations pt;
  late AppLocalizations en;
  setUpAll(() async {
    pt = await AppLocalizations.delegate.load(const Locale('pt'));
    en = await AppLocalizations.delegate.load(const Locale('en'));
  });

  String foreignYearStatus(ForeignYear y) => labels.foreignYearStatus(pt, y);
  String foreignWarningText(ForeignTaxWarning w) => labels.foreignWarningText(pt, w);

  ForeignYear year({
    double lossCarriedIn = 0,
    double taxBeforeCredit = 0,
    double creditUsed = 0,
    double taxDue = 0,
    double lossCarriedOut = 0,
  }) => ForeignYear(
    year: 2025,
    capitalGain: 0,
    dividends: 0,
    withheld: creditUsed,
    lossCarriedIn: lossCarriedIn,
    taxableBase: 0,
    rate: 0.15,
    taxBeforeCredit: taxBeforeCredit,
    creditUsed: creditUsed,
    taxDue: taxDue,
    lossCarriedOut: lossCarriedOut,
    sales: const [],
    dividendItems: const [],
  );

  test('situação do ano: imposto estimado', () {
    expect(
      foreignYearStatus(year(taxBeforeCredit: 420, creditUsed: 60, taxDue: 360)),
      'Imposto estimado de R\$ 360,00 na declaração',
    );
  });

  test('situação do ano: retido no exterior cobre o imposto', () {
    expect(
      foreignYearStatus(year(taxBeforeCredit: 90, creditUsed: 90)),
      contains('o retido no exterior cobre os R\$ 90,00 devidos'),
    );
  });

  test('situação do ano: prejuízo do ano abate os seguintes', () {
    expect(foreignYearStatus(year(lossCarriedOut: 600)), contains('prejuízo de R\$ 600,00'));
  });

  test('situação do ano: resultado coube no prejuízo anterior', () {
    expect(
      foreignYearStatus(year(lossCarriedIn: 500, lossCarriedOut: 100)),
      contains('coube no prejuízo de anos anteriores'),
    );
  });

  test('en: as quatro situações do ano', () {
    String t(ForeignYear y) => labels.foreignYearStatus(en, y);
    expect(
      t(year(taxBeforeCredit: 420, creditUsed: 60, taxDue: 360)),
      'Estimated tax of R\$ 360.00 on the return',
    );
    expect(
      t(year(taxBeforeCredit: 90, creditUsed: 90)),
      'No tax due: the tax withheld abroad covers the R\$ 90.00 owed',
    );
    expect(
      t(year(lossCarriedOut: 600)),
      'No tax: the year closed with a loss of R\$ 600.00, which offsets the following years',
    );
    expect(
      t(year(lossCarriedIn: 500, lossCarriedOut: 100)),
      "No tax: the year's result fit within the loss from earlier years",
    );
  });

  test('avisos em português', () {
    expect(
      foreignWarningText(
        ForeignTaxWarning.costUnknown(ticker: 'AAPL', date: DateTime(2025, 6, 15)),
      ),
      contains('AAPL em 15/06/2025'),
    );
    expect(
      foreignWarningText(const ForeignTaxWarning.fxMissing(year: 2027)),
      contains('de 2027 sem câmbio'),
    );
  });

  test('o aviso de câmbio aproximado fala do ano e da PTAX', () {
    final text = foreignWarningText(const ForeignTaxWarning.fxApproximate(year: 2025));

    expect(text, contains('2025'));
    expect(text, contains('PTAX do Banco Central'));
    expect(text, contains('aproximados'));
  });

  test('en: os três avisos, com a data MM/DD/YYYY e a PTAX mantida', () {
    expect(
      labels.foreignWarningText(
        en,
        ForeignTaxWarning.costUnknown(ticker: 'AAPL', date: DateTime(2025, 6, 15)),
      ),
      'Sale of AAPL on 06/15/2025 exceeds the purchases entered: the gain counts only the part '
      'with known cost. Enter the missing purchases.',
    );
    expect(
      labels.foreignWarningText(en, const ForeignTaxWarning.fxMissing(year: 2027)),
      'Operations or dividends of 2027 have no exchange rate (neither PTAX nor market) for the '
      'date: they were left out of the assessment.',
    );
    expect(
      labels.foreignWarningText(en, const ForeignTaxWarning.fxApproximate(year: 2025)),
      'Operations or dividends of 2025 were converted at the market rate because the Central '
      'Bank PTAX was not available for them (no network, a currency the PTAX does not quote, or '
      'a long gap): the values for the year are approximate.',
    );
  });

  test('a nota diz que a conversão é pela PTAX, com o mercado de reserva', () {
    expect(pt.foreignIntroNote, contains('PTAX do Banco Central'));
    expect(pt.foreignIntroNote, contains('câmbio de mercado'));
    expect(pt.foreignIntroNote, contains('sem DARF mensal'));
    expect(en.foreignIntroNote, contains('Central Bank PTAX'));
    expect(en.foreignIntroNote, contains('market rate (Yahoo)'));
    expect(en.foreignIntroNote, contains('no monthly DARF'));
  });

  ForeignHolding holding(int year, double quantity, double cost) => ForeignHolding(
    year: year,
    assetId: 1,
    ticker: 'AAPL',
    name: 'Apple',
    currency: 'USD',
    quantity: quantity,
    costBrl: cost,
    fxApproximate: false,
    incomplete: false,
  );

  String description({
    AppLocalizations? l,
    ForeignHolding? previous,
    ForeignHolding? current,
    String? custodiaLabel,
    String? group,
    String? code,
    String? country,
  }) => labels.foreignHoldingDescription(
    l ?? pt,
    year: 2025,
    ticker: 'AAPL',
    name: 'Apple',
    custodiaLabel: custodiaLabel,
    previous: previous,
    current: current,
    group: group,
    code: code,
    country: country,
  );

  test('a descrição do informe traz as duas posições e a base da conversão', () {
    final text = description(
      previous: holding(2024, 10, 5000),
      current: holding(2025, 20.5, 12200),
    );

    expect(text, contains('AAPL — Apple.'));
    expect(text, contains('Situação em 31/12/2024: 10 un., custo de aquisição R\$ 5.000,00.'));
    expect(text, contains('Situação em 31/12/2025: 20,5 un., custo de aquisição R\$ 12.200,00.'));
    expect(text, contains('cotação de fechamento (venda) do Banco Central'));
  });

  test('en: a descrição segue o idioma da tela, com datas e quantidade no formato en', () {
    final text = description(
      l: en,
      previous: holding(2024, 10, 5000),
      current: holding(2025, 20.5, 12200),
    );

    expect(
      text,
      'AAPL — Apple.\n'
      'Position on 12/31/2024: 10 units, acquisition cost R\$ 5,000.00.\n'
      'Position on 12/31/2025: 20.5 units, acquisition cost R\$ 12,200.00.\n'
      "Acquisition cost converted into reais at the Central Bank's closing (selling) rate on "
      'the date of each purchase.',
    );
  });

  test('a descrição traz a ficha só quando o usuário a preencheu', () {
    final filled = description(
      current: holding(2025, 4, 2200),
      group: '03',
      code: '01',
      country: '249',
    );
    expect(filled, contains('Ficha: Grupo 03 · Código 01 · País 249.'));
    expect(
      description(l: en, current: holding(2025, 4, 2200), group: '03', code: '01', country: '249'),
      contains('Record: Group 03 · Code 01 · Country 249.'),
    );

    // preenchida pela metade: só o que veio
    final partial = description(current: holding(2025, 4, 2200), country: '249');
    expect(partial, contains('Ficha: País 249.'));

    final blank = description(current: holding(2025, 4, 2200));
    expect(blank, isNot(contains('Ficha')));
    expect(description(l: en, current: holding(2025, 4, 2200)), isNot(contains('Record')));
  });

  test('a descrição traz a custódia quando há', () {
    final text = description(custodiaLabel: 'Avenue (Eu)', current: holding(2025, 4, 2200));

    expect(text, contains('AAPL — Apple. Custódia: Avenue (Eu).'));
    expect(
      description(l: en, custodiaLabel: 'Avenue (Eu)', current: holding(2025, 4, 2200)),
      contains('AAPL — Apple. Custody: Avenue (Eu).'),
    );
  });

  test('a descrição sem posição em uma das datas diz isso', () {
    final text = description(previous: holding(2024, 10, 5000));

    expect(text, contains('Situação em 31/12/2025: sem posição, custo de aquisição R\$ 0,00.'));
    expect(
      description(l: en, previous: holding(2024, 10, 5000)),
      contains('Position on 12/31/2025: no position, acquisition cost R\$ 0.00.'),
    );
  });

  test('a quantidade perde os zeros à direita e usa o separador do idioma', () {
    expect(formatHoldingQuantity(10, locale: 'pt'), '10');
    expect(formatHoldingQuantity(0.12345678, locale: 'pt'), '0,12345678');
    expect(formatHoldingQuantity(1.5, locale: 'pt'), '1,5');
    expect(formatHoldingQuantity(0.12345678, locale: 'en'), '0.12345678');
    expect(formatHoldingQuantity(1.5, locale: 'en'), '1.5');
  });

  test('a nota e a legenda do informe avisam dos limites', () {
    expect(pt.foreignHoldingsNote, contains('não valor de mercado'));
    expect(pt.foreignHoldingsNote, contains('país'));
    expect(pt.foreignHoldingsLegend, contains('aproximado'));
    expect(pt.foreignHoldingsLegend, contains('incompleto'));
    expect(en.foreignHoldingsNote, contains('not market value'));
    expect(en.foreignHoldingsNote, contains('country'));
    expect(en.foreignHoldingsLegend, contains('"approximate"'));
    expect(en.foreignHoldingsLegend, contains('"incomplete"'));
  });

  test('a taxa de cada fato diz de onde veio', () {
    expect(labels.foreignRateLabel(pt, 6.2086, RateSource.ptax), 'Câmbio 6,2086 (PTAX)');
    expect(labels.foreignRateLabel(pt, 6, RateSource.market), 'Câmbio 6,0000 (mercado)');
    expect(labels.foreignRateLabel(en, 6.2086, RateSource.ptax), 'Rate 6.2086 (PTAX)');
    expect(labels.foreignRateLabel(en, 6, RateSource.market), 'Rate 6.0000 (market)');
  });

  test('a ficha do ativo em cada idioma', () {
    expect(
      labels.dirpfFichaText(pt, group: '03', code: '01', country: '249'),
      'Grupo 03 · Código 01 · País 249',
    );
    expect(labels.dirpfFichaText(pt, country: '249'), 'País 249');
    expect(
      labels.dirpfFichaText(en, group: '03', code: '01', country: '249'),
      'Group 03 · Code 01 · Country 249',
    );
    expect(labels.dirpfFichaText(en, country: '249'), 'Country 249');
    expect(labels.dirpfFichaText(en, group: '', code: null), '');
  });

  // A tela montada de verdade (SQLite ffi, sem ativo do exterior, então sem rede).
  group('a tela', () {
    late Directory dir;

    setUp(() async {
      dir = await Directory.systemTemp.createTemp('anchor-foreign-tax-screen-');
      await databaseFactory.setDatabasesPath(dir.path);
      await AppDatabase.resetForTest();
    });

    tearDown(() async {
      await AppDatabase.resetForTest();
      await dir.delete(recursive: true);
    });

    Future<void> pumpScreen(WidgetTester tester, String locale) async {
      tester.view.physicalSize = const Size(900, 3000);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      await tester.runAsync(AppDatabase.instance);
      await tester.pumpWidget(localizedApp(const ForeignTaxScreen(), locale: locale));
      for (var i = 0; i < 40; i++) {
        await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
        await tester.pump();
        if (find.byType(CircularProgressIndicator).evaluate().isEmpty) break;
      }
    }

    testWidgets('sem ativos do exterior: título, nota e vazio nos dois idiomas', (tester) async {
      await pumpScreen(tester, 'pt');
      expect(find.text('IR de exterior'), findsOneWidget);
      expect(find.text(pt.foreignIntroNote), findsOneWidget);
      expect(
        find.text('Nenhuma venda ou provento de ativo do exterior a partir de 2024 ainda.'),
        findsOneWidget,
      );

      await pumpScreen(tester, 'en');
      expect(find.text('Foreign tax'), findsOneWidget);
      expect(find.text(en.foreignIntroNote), findsOneWidget);
      expect(
        find.text('No sales or dividends from foreign assets from 2024 on yet.'),
        findsOneWidget,
      );
      expect(find.textContaining('Nenhuma'), findsNothing);
    });
  });
}
