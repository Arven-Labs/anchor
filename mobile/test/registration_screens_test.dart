import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/l10n/app_localizations.dart';
import 'package:anchor_mobile/l10n/registration_labels.dart';
import 'package:anchor_mobile/models/asset.dart';
import 'package:anchor_mobile/screens/add_asset_screen.dart';
import 'package:anchor_mobile/screens/custodias_screen.dart';
import 'package:anchor_mobile/screens/quote_search_screen.dart';

import 'support/localized_app.dart';

// Cadastro de ativo, busca de cotação e Custódias (ADR 0001). O português é o texto que as
// telas já mostravam; o inglês é a tradução.
void main() {
  late AppLocalizations pt;
  late AppLocalizations en;
  setUpAll(() async {
    pt = await AppLocalizations.delegate.load(const Locale('pt'));
    en = await AppLocalizations.delegate.load(const Locale('en'));
  });

  group('exemplos de ticker', () {
    test('as classes com ticker real mostram o mesmo exemplo nos dois idiomas', () {
      for (final (c, hint) in [
        (AssetClass.acaoBr, 'PETR4'),
        (AssetClass.fii, 'HGLG11'),
        (AssetClass.etfRendaFixa, 'AUPO11'),
        (AssetClass.acaoInternacional, 'AAPL'),
        (AssetClass.cripto, 'ETH'),
        (AssetClass.tesouroDireto, 'Tesouro Selic 2029'),
      ]) {
        expect(assetTickerHint(pt, c), hint, reason: c.value);
        expect(assetTickerHint(en, c), hint, reason: c.value);
      }
    });

    test('pt: o exemplo de antes; en: o nome traduzido', () {
      expect(assetTickerHint(pt, AssetClass.imovel), 'Apto Centro');
      expect(assetTickerHint(pt, AssetClass.empresaNaoListada), 'Empresa XYZ');
      expect(assetTickerHint(pt, AssetClass.fundo), 'Fundo XYZ FIC FIM');
      expect(assetTickerHint(pt, AssetClass.rendaFixa), 'CDB Banco X');
      expect(assetTickerHint(en, AssetClass.imovel), 'Downtown apartment');
      expect(assetTickerHint(en, AssetClass.empresaNaoListada), 'Company XYZ');
      expect(assetTickerHint(en, AssetClass.fundo), 'XYZ FIC FIM fund');
      expect(assetTickerHint(en, AssetClass.rendaFixa), 'CDB Bank X');
    });

    test('toda classe tem exemplo nos dois idiomas', () {
      for (final c in AssetClass.values) {
        expect(assetTickerHint(pt, c), isNotEmpty, reason: c.value);
        expect(assetTickerHint(en, c), isNotEmpty, reason: c.value);
      }
    });
  });

  group('as telas', () {
    late Directory dir;

    setUp(() async {
      dir = await Directory.systemTemp.createTemp('anchor-registration-');
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

    /// Espera (com o relógio real, por causa do banco) até o texto sumir da tela.
    Future<void> waitGone(WidgetTester tester, String text) async {
      for (var i = 0; i < 100 && find.text(text).evaluate().isNotEmpty; i++) {
        await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
        await tester.pump();
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

    Future<void> chooseClass(WidgetTester tester, String name) async {
      await tester.tap(find.byType(DropdownButtonFormField<AssetClass>));
      await tester.pumpAndSettle();
      await tester.tap(find.text(name).last);
      await tester.pumpAndSettle();
    }

    testWidgets('Adicionar ativo em pt', (tester) async {
      await pump(tester, const AddAssetScreen(), 'pt');

      expect(find.text('Adicionar ativo'), findsOneWidget);
      expect(find.text('Ticker (Ação BR)'), findsOneWidget);
      expect(find.text('Nome'), findsOneWidget);
      expect(find.text('Classe'), findsOneWidget);
      expect(find.text('Buscar'), findsOneWidget);
      expect(find.text('Salvar'), findsOneWidget);
    });

    testWidgets('Adicionar ativo em en: rótulos, fundo e empresa não listada', (tester) async {
      await pump(tester, const AddAssetScreen(), 'en');

      expect(find.text('Add asset'), findsOneWidget);
      expect(find.text('Ticker (BR stock)'), findsOneWidget);
      expect(find.text('Name'), findsOneWidget);
      expect(find.text('Class'), findsOneWidget);
      expect(find.text('Search'), findsOneWidget);
      expect(find.text('Save'), findsOneWidget);

      await chooseClass(tester, 'Fund');
      expect(find.text('Ticker (Fund)'), findsOneWidget);
      expect(find.text('Fund type (for the semiannual quota tax)'), findsOneWidget);
      expect(find.text('Search'), findsNothing, reason: 'fundo não tem cotação automática');

      await chooseClass(tester, 'Unlisted company');
      expect(find.text('Equity stake'), findsOneWidget);
      expect(find.text('Shares owned'), findsOneWidget);
      expect(find.text("Company's total shares"), findsOneWidget);
      expect(find.text('Company valuation (BRL)'), findsOneWidget);

      await tester.enterText(find.widgetWithText(TextField, 'Shares owned'), '25');
      await tester.enterText(find.widgetWithText(TextField, "Company's total shares"), '200');
      await tester.enterText(find.widgetWithText(TextField, 'Company valuation (BRL)'), '10000');
      await tester.pump();
      expect(find.text('Equity stake (12.50% — 1250.00)'), findsOneWidget);
      for (final pt in ['Participação', 'Valuation', 'Nome', 'Salvar']) {
        expect(find.textContaining(pt), findsNothing, reason: pt);
      }
    });

    testWidgets('Adicionar ativo em pt: participação com vírgula decimal', (tester) async {
      await pump(tester, const AddAssetScreen(), 'pt');
      await chooseClass(tester, 'Empresa não listada');

      expect(find.text('Cotas/ações possuídas'), findsOneWidget);
      expect(find.text('Total de cotas/ações da empresa'), findsOneWidget);
      expect(find.text('Valuation da empresa (BRL)'), findsOneWidget);
      await tester.enterText(find.widgetWithText(TextField, 'Cotas/ações possuídas'), '25');
      await tester.enterText(find.widgetWithText(TextField, 'Total de cotas/ações da empresa'), '200');
      await tester.enterText(find.widgetWithText(TextField, 'Valuation da empresa (BRL)'), '10000');
      await tester.pump();
      expect(find.text('Participação societária (12,50% — 1250,00)'), findsOneWidget);
    });

    testWidgets('Busca de cotação nos dois idiomas', (tester) async {
      await pump(tester, const QuoteSearchScreen(), 'pt');
      expect(find.text('Anchor'), findsOneWidget);
      expect(find.text('Classe'), findsOneWidget);
      expect(find.text('Ticker (Ação BR)'), findsOneWidget);
      expect(find.text('Buscar'), findsOneWidget);

      await pump(tester, const QuoteSearchScreen(), 'en');
      expect(find.text('Anchor'), findsOneWidget);
      expect(find.text('Class'), findsOneWidget);
      expect(find.text('Ticker (BR stock)'), findsOneWidget);
      expect(find.text('Search'), findsOneWidget);
      expect(find.text('Buscar'), findsNothing);
    });

    testWidgets('Custódias em pt: vazio, adicionar e editar', (tester) async {
      await pump(tester, const CustodiasScreen(), 'pt');
      expect(find.text('Custódias'), findsOneWidget);
      expect(find.text('Nenhuma custódia cadastrada ainda.'), findsOneWidget);
      expect(find.text('Instituição'), findsOneWidget);
      expect(find.text('Titular'), findsOneWidget);
      expect(find.text('Adicionar'), findsOneWidget);

      await tester.enterText(find.widgetWithText(TextField, 'Instituição'), 'XP');
      await tester.enterText(find.widgetWithText(TextField, 'Titular'), 'Eu');
      await tester.tap(find.text('Adicionar'));
      await waitGone(tester, 'Nenhuma custódia cadastrada ainda.');
      expect(find.text('Nenhuma custódia cadastrada ainda.'), findsNothing);
      expect(find.text('XP'), findsWidgets);

      await tester.tap(find.byIcon(Icons.edit_outlined));
      await tester.pumpAndSettle();
      expect(find.text('Editar custódia'), findsOneWidget);
      expect(find.text('Cancelar'), findsOneWidget);
      expect(find.text('Salvar'), findsOneWidget);
    });

    testWidgets('Custódias em en: o mesmo fluxo em inglês', (tester) async {
      await pump(tester, const CustodiasScreen(), 'en');
      expect(find.text('Custodians'), findsOneWidget);
      expect(find.text('No custodians yet.'), findsOneWidget);
      expect(find.text('Institution'), findsOneWidget);
      expect(find.text('Holder'), findsOneWidget);
      expect(find.text('Add'), findsOneWidget);

      await tester.enterText(find.widgetWithText(TextField, 'Institution'), 'XP');
      await tester.enterText(find.widgetWithText(TextField, 'Holder'), 'Me');
      await tester.tap(find.text('Add'));
      await waitGone(tester, 'No custodians yet.');
      expect(find.text('No custodians yet.'), findsNothing);

      await tester.tap(find.byIcon(Icons.edit_outlined));
      await tester.pumpAndSettle();
      expect(find.text('Edit custodian'), findsOneWidget);
      expect(find.text('Cancel'), findsOneWidget);
      expect(find.text('Save'), findsOneWidget);
      for (final pt in ['Custódia', 'Instituição', 'Titular', 'Adicionar', 'Editar']) {
        expect(find.textContaining(pt), findsNothing, reason: pt);
      }
    });
  });
}
