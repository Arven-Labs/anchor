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

// A tela de IR de fundos montada de verdade (SQLite ffi, sem rede): o que as funções de
// texto de `fund_tax_test.dart` provam por partes, aqui chega à tela.
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

  testWidgets(
    'um fundo zerado com queda depois do come-cotas mostra o excesso e a guarda do registro',
    (tester) async {
      tester.view.physicalSize = const Size(900, 6000);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);

      // 100 cotas a 10,00; come-cotas de 2,5 cotas a 12,00 (30 pagos); resgate de 97,5
      // a 9,00 (perda legal 97,5 × (12 − 9) = 292,50, excesso de come-cotas de 30). A XP
      // não tem outro fundo, então o saldo fica no grupo e todos os fundos dela estão
      // zerados: a tela avisa.
      await tester.runAsync(() async {
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
      });

      await tester.pumpWidget(const MaterialApp(home: FundTaxScreen()));
      for (var i = 0; i < 20; i++) {
        await tester.runAsync(
          () => Future<void>.delayed(const Duration(milliseconds: 50)),
        );
        await tester.pump();
      }

      // o excesso do resgate, sem falar em restituição pelo PER/DCOMP
      expect(find.textContaining('A lei não prevê restituição'), findsOneWidget);
      expect(find.textContaining('PER/DCOMP'), findsNothing);
      // o saldo de 292,50 do grupo XP e o aviso da guarda; a versão do texto depende
      // de "hoje" estar antes ou depois de 31/12/2026
      expect(find.textContaining('XP'), findsWidgets);
      expect(find.textContaining('R\$ 292,50'), findsWidgets);
      expect(
        find.textContaining('foram resgatados até 01/06/2025'),
        findsOneWidget,
      );
      final guarantee = find.byWidgetPredicate(
        (w) =>
            w is Text &&
            (w.data ?? '').contains('registro das perdas') &&
            (w.data!.contains('até 31/12/2026') ||
                w.data!.contains('terminou em 31/12/2026')),
      );
      expect(guarantee, findsOneWidget);
    },
  );

  testWidgets('sem fundos a tela não mostra aviso nenhum', (tester) async {
    await tester.pumpWidget(const MaterialApp(home: FundTaxScreen()));
    for (var i = 0; i < 10; i++) {
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 50)),
      );
      await tester.pump();
    }

    expect(find.textContaining('foram resgatados até'), findsNothing);
    expect(find.textContaining('A lei não prevê restituição'), findsNothing);
  });
}
