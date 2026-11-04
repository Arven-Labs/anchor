import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/models/asset.dart';
import 'package:anchor_mobile/models/fund.dart';
import 'package:anchor_mobile/models/portfolio_transaction.dart';
import 'package:anchor_mobile/screens/fund_tax_screen.dart';
import 'package:anchor_mobile/services/portfolio_repository.dart';

import 'support/localized_app.dart';

// A tela de IR de fundos montada de verdade (SQLite ffi, sem rede): o que as funções de
// texto de `fund_tax_test.dart` provam por partes, aqui chega à tela, nos dois idiomas.
void main() {
  DateTime d(String s) => DateTime.parse('${s}T00:00:00Z');

  final repository = PortfolioRepository();
  late Directory dir;

  setUp(() async {
    dir = await Directory.systemTemp.createTemp('anchor-screen-test-');
    await databaseFactory.setDatabasesPath(dir.path);
    await AppDatabase.resetForTest();
  });

  tearDown(() async {
    await AppDatabase.resetForTest();
    await dir.delete(recursive: true);
  });

  Future<void> addTx(
    int assetId,
    TransactionType type,
    String date,
    double quantity,
    double price,
  ) => repository.insertTransaction(
    PortfolioTransaction(
      assetId: assetId,
      type: type,
      quantity: quantity,
      unitPrice: price,
      totalValue: quantity * price,
      date: d(date),
      createdAt: d(date),
    ),
  );

  // 100 cotas a 10,00; come-cotas de 2,5 cotas a 12,00 (30 pagos); resgate de 97,5 a 9,00
  // (perda legal 97,5 × (12 − 9) = 292,50, excesso de come-cotas de 30). A XP não tem outro
  // fundo, então o saldo fica no grupo e todos os fundos dela estão zerados: a tela avisa.
  Future<void> seedZeroedXpFund() async {
    final id = await repository.insertAsset(
      Asset(
        ticker: 'FUNDOXP',
        name: 'Fundo XP',
        assetClass: AssetClass.fundo,
        currency: 'BRL',
        createdAt: DateTime(2025, 1, 1),
      ),
    );
    await repository.setFundTerm(id, FundTerm.longTerm);
    await repository.setFundAdministrator(id, 'XP');
    await addTx(id, TransactionType.compra, '2025-01-01', 100, 10);
    await addTx(id, TransactionType.comeCotas, '2025-03-31', 2.5, 12);
    await addTx(id, TransactionType.venda, '2025-06-01', 97.5, 9);
  }

  Future<void> pumpScreen(WidgetTester tester, String locale, {bool seed = false}) async {
    tester.view.physicalSize = const Size(900, 6000);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    if (seed) await tester.runAsync(seedZeroedXpFund);
    await tester.pumpWidget(localizedApp(const FundTaxScreen(), locale: locale));
    // Espera o fim do carregamento (o SQLite ffi roda fora do relógio falso), não um tempo
    // fixo: sob carga de CPU a leitura demora mais.
    for (var i = 0; i < 200; i++) {
      await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
      await tester.pump();
      if (find.byType(CircularProgressIndicator).evaluate().isEmpty) break;
    }
    expect(find.byType(CircularProgressIndicator), findsNothing, reason: 'a tela não terminou de carregar');
  }

  testWidgets(
    'pt: um fundo zerado com queda depois do come-cotas mostra o excesso e a guarda do registro',
    (tester) async {
      await pumpScreen(tester, 'pt', seed: true);

      expect(find.text('IR de fundos'), findsOneWidget);
      // o excesso do resgate, sem falar em restituição pelo PER/DCOMP
      expect(find.textContaining('A lei não prevê restituição'), findsOneWidget);
      expect(find.textContaining('PER/DCOMP'), findsNothing);
      // o saldo de 292,50 do grupo XP e o aviso da guarda; a versão do texto depende
      // de "hoje" estar antes ou depois de 31/12/2026
      expect(find.textContaining('XP'), findsWidgets);
      expect(find.textContaining('R\$ 292,50'), findsWidgets);
      expect(find.textContaining('foram resgatados até 01/06/2025'), findsOneWidget);
      final guarantee = find.byWidgetPredicate(
        (w) =>
            w is Text &&
            (w.data ?? '').contains('registro das perdas') &&
            (w.data!.contains('até 31/12/2026') || w.data!.contains('terminou em 31/12/2026')),
      );
      expect(guarantee, findsOneWidget);
    },
  );

  testWidgets('en: a mesma tela em inglês, sem sobra de português', (tester) async {
    await pumpScreen(tester, 'en', seed: true);

    expect(find.text('Fund tax'), findsOneWidget);
    expect(find.textContaining('The law does not provide a refund'), findsOneWidget);
    expect(find.textContaining('R\$ 292.50'), findsWidgets);
    expect(find.textContaining('were redeemed by 06/01/2025'), findsOneWidget);
    // o cartão do fundo: rótulos do formulário, listas e botões
    expect(find.text('Fund type'), findsOneWidget);
    expect(find.text('Long term'), findsWidgets);
    expect(find.text('Fund structure (Law 14.754)'), findsOneWidget);
    expect(find.text('Administrator / intermediary (offsets loss)'), findsOneWidget);
    expect(find.text('Fund CNPJ (quota from the CVM)'), findsOneWidget);
    expect(find.text('Quota value (1)'), findsNothing, reason: 'sem cota lançada são 0');
    expect(find.text('Quota value (0)'), findsOneWidget);
    expect(find.text('Semiannual quota tax recorded (1)'), findsOneWidget);
    expect(find.text('Estimated redemptions and amortizations (1)'), findsOneWidget);
    for (final pt in ['Tipo do fundo', 'Estrutura', 'Administradora', 'Valor da cota', 'resgat']) {
      expect(find.textContaining(pt), findsNothing, reason: pt);
    }
  });

  testWidgets('sem fundos a tela não mostra aviso nenhum', (tester) async {
    await pumpScreen(tester, 'pt');

    expect(find.textContaining('foram resgatados até'), findsNothing);
    expect(find.textContaining('A lei não prevê restituição'), findsNothing);
    expect(find.text('Nenhum fundo cadastrado ainda.'), findsOneWidget);

    await pumpScreen(tester, 'en');
    expect(find.text('No funds registered yet.'), findsOneWidget);
  });

  test('gravar sem escolher o tipo do fundo, ou com CNPJ curto, lança o código do erro', () async {
    final id = await repository.insertAsset(
      Asset(
        ticker: 'SEMTIPO',
        name: 'Fundo sem tipo',
        assetClass: AssetClass.fundo,
        currency: 'BRL',
        createdAt: DateTime(2025, 1, 1),
      ),
    );

    Future<FundSettingErrorCode> codeOf(Future<void> Function() action) async {
      try {
        await action();
      } on FundSettingException catch (e) {
        return e.code;
      }
      fail('devia recusar');
    }

    expect(
      await codeOf(() => repository.setFundStructure(id, FundStructure.closed)),
      FundSettingErrorCode.needTermBeforeStructure,
    );
    expect(
      await codeOf(() => repository.setFundEstoque(id, EstoqueOption.general)),
      FundSettingErrorCode.needTermBeforeEstoque,
    );
    expect(
      await codeOf(() => repository.setFundAdministrator(id, 'XP')),
      FundSettingErrorCode.needTermBeforeAdministrator,
    );
    expect(
      await codeOf(() => repository.setFundCvm(id, subclass: 'A')),
      FundSettingErrorCode.needTermBeforeSubclass,
    );
    expect(
      await codeOf(() => repository.setFundCvm(id, cnpj: '123')),
      FundSettingErrorCode.cnpjMustHave14Digits,
    );
  });
}
