import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/domain/fx_history.dart';
import 'package:anchor_mobile/domain/general_finance.dart';
import 'package:anchor_mobile/domain/liability.dart';
import 'package:anchor_mobile/models/liability.dart';
import 'package:anchor_mobile/screens/financas_liabilities_screen.dart';
import 'package:anchor_mobile/models/bank_account.dart';
import 'package:anchor_mobile/models/general_transaction.dart';
import 'package:anchor_mobile/models/general_transaction_category.dart';
import 'package:anchor_mobile/screens/financas_categories_screen.dart';
import 'package:anchor_mobile/screens/financas_screen.dart';
import 'package:anchor_mobile/screens/financas_transactions_screen.dart';
import 'package:anchor_mobile/services/general_finance_repository.dart';

// Capturas das telas de Finanças para o CI publicar como artefato (o CI do
// mobile não roda emulador). Só grava se `SCREENSHOT_DIR` estiver definida;
// sem ela o teste passa sem fazer nada. O texto sai com a Roboto do SDK: o
// padrão do `flutter test` é a fonte Ahem, que desenha blocos.
void main() {
  final outDir = Platform.environment['SCREENSHOT_DIR'];
  final repository = GeneralFinanceRepository();
  late Directory dir;

  setUp(() async {
    dir = await Directory.systemTemp.createTemp('anchor-financas-shots-');
    await databaseFactory.setDatabasesPath(dir.path);
    await AppDatabase.resetForTest();
  });

  tearDown(() async {
    await AppDatabase.resetForTest();
    await dir.delete(recursive: true);
  });

  Future<void> loadFonts() async {
    final root = Platform.environment['FLUTTER_ROOT'];
    if (root == null) throw StateError('FLUTTER_ROOT não definida');
    final fonts = '$root/bin/cache/artifacts/material_fonts';
    Future<ByteData> bytes(String file) async =>
        ByteData.sublistView(await File('$fonts/$file').readAsBytes());
    final roboto = FontLoader('Roboto')
      ..addFont(bytes('Roboto-Regular.ttf'))
      ..addFont(bytes('Roboto-Medium.ttf'))
      ..addFont(bytes('Roboto-Bold.ttf'));
    final icons = FontLoader('MaterialIcons')
      ..addFont(bytes('MaterialIcons-Regular.otf'));
    await roboto.load();
    await icons.load();
  }

  Future<void> settle(WidgetTester tester, bool Function() ready) async {
    for (var i = 0; i < 200; i++) {
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 25)),
      );
      await tester.pump();
      if (ready()) break;
    }
    await tester.pump(const Duration(milliseconds: 300));
  }

  Future<void> capture(WidgetTester tester, String name) async {
    final boundary =
        tester.renderObject(find.byKey(const Key('shot'))) as RenderRepaintBoundary;
    await tester.runAsync(() async {
      final image = await boundary.toImage(pixelRatio: 2);
      final data = await image.toByteData(format: ui.ImageByteFormat.png);
      await Directory(outDir!).create(recursive: true);
      await File('$outDir/$name.png').writeAsBytes(data!.buffer.asUint8List());
    });
  }

  Widget app(Widget home) => RepaintBoundary(
    key: const Key('shot'),
    child: MaterialApp(
      debugShowCheckedModeBanner: false,
      theme: ThemeData(colorSchemeSeed: Colors.indigo, useMaterial3: true),
      home: home,
    ),
  );

  testWidgets('capturas da aba Finanças, Categorias e Lançamentos', (tester) async {
    tester.view.physicalSize = const Size(780, 1688);
    tester.view.devicePixelRatio = 2;
    addTearDown(tester.view.reset);
    await tester.runAsync(loadFonts);

    final month = toIsoDay(DateTime.now()).substring(0, 7);
    late BankAccount nubank;
    late BankAccount chase;
    await tester.runAsync(() async {
      final nubankId = await repository.insertAccount(
        BankAccount(
          nome: 'Nubank',
          titular: 'Eu',
          currency: 'BRL',
          createdAt: DateTime(2026, 1, 1),
        ),
      );
      final chaseId = await repository.insertAccount(
        BankAccount(
          nome: 'Chase',
          titular: 'Eu',
          currency: 'USD',
          createdAt: DateTime(2026, 1, 1),
        ),
      );
      nubank = BankAccount(
        id: nubankId,
        nome: 'Nubank',
        titular: 'Eu',
        currency: 'BRL',
        createdAt: DateTime(2026, 1, 1),
      );
      chase = BankAccount(
        id: chaseId,
        nome: 'Chase',
        titular: 'Eu',
        currency: 'USD',
        createdAt: DateTime(2026, 1, 1),
      );
      Future<int> category(String nome, double? limit) => repository.insertCategory(
        GeneralTransactionCategory(
          nome: nome,
          limiteMensal: limit,
          createdAt: DateTime(2026, 1, 1),
        ),
      );
      final mercado = await category('Mercado', 500);
      final lazer = await category('Lazer', 1000);
      await category('Saúde', null);
      Future<void> add(
        int account,
        GeneralTransactionType type,
        double valor,
        String day, {
        int? categoryId,
        String? notes,
      }) => repository.insertTransaction(
        GeneralTransaction(
          bankAccountId: account,
          type: type,
          categoryId: categoryId,
          valor: valor,
          date: '$month-$day',
          notes: notes,
          createdAt: DateTime(2026, 1, 1),
        ),
      );
      await add(nubankId, GeneralTransactionType.receita, 6500, '01', notes: 'Salário');
      await add(nubankId, GeneralTransactionType.despesa, 350, '03', categoryId: mercado);
      await add(nubankId, GeneralTransactionType.despesa, 250, '04', categoryId: mercado, notes: 'Feira');
      await add(nubankId, GeneralTransactionType.despesa, 100, '05', categoryId: lazer);
      await add(chaseId, GeneralTransactionType.receita, 1200, '02');
      await add(chaseId, GeneralTransactionType.despesa, 40, '06', categoryId: lazer);
      await repository.createLiability(
        Liability(
          bankAccountId: nubankId,
          nome: 'Financiamento do carro',
          titular: 'Eu',
          principal: 24000,
          taxaPercentual: 1.5,
          prazoMeses: 24,
          sistema: AmortizationSystem.price,
          dataInicio: toIsoDay(addMonths(DateTime.now(), -3)),
          createdAt: DateTime(2026, 1, 1),
        ),
      );
      await repository.createTransfer(
        fromAccountId: nubankId,
        toAccountId: chaseId,
        valorSaida: 520,
        valorEntrada: 100,
        date: '$month-02',
        notes: 'Câmbio',
      );
    });

    await tester.pumpWidget(
      app(
        FinancasScreen(
          fetchFx: (currency) async =>
              FxHistory([(DateTime.now(), currency == 'USD' ? 5.0 : 1.0)]),
          loadPositions: () async => const [],
        ),
      ),
    );
    await settle(
      tester,
      () => find.byKey(const Key('financas-net-worth')).evaluate().isNotEmpty,
    );
    await capture(tester, 'financas');

    await tester.tap(find.byTooltip('Transferir'));
    await tester.pumpAndSettle();
    await tester.enterText(find.byKey(const Key('financas-transfer-out')), '520');
    await tester.enterText(find.byKey(const Key('financas-transfer-in')), '100');
    await tester.pump();
    await capture(tester, 'transferir');
    await tester.tap(find.text('Cancelar'));
    await tester.pumpAndSettle();

    await tester.pumpWidget(
      app(FinancasLiabilitiesScreen(accounts: [nubank, chase])),
    );
    await settle(
      tester,
      () => find.text('Financiamento do carro').evaluate().isNotEmpty,
    );
    await tester.tap(find.text('Financiamento do carro'));
    await tester.pumpAndSettle();
    await capture(tester, 'dividas');

    await tester.pumpWidget(
      app(
        FinancasCategoriesScreen(
          base: 'BRL',
          ratesToBrl: const {'USD': 5.0},
          accounts: [nubank, chase],
        ),
      ),
    );
    await settle(tester, () => find.text('Mercado').evaluate().isNotEmpty);
    await capture(tester, 'categorias');

    await tester.pumpWidget(app(FinancasTransactionsScreen(account: nubank)));
    await settle(tester, () => find.text('Salário').evaluate().isNotEmpty);
    await capture(tester, 'lancamentos');
  }, skip: outDir == null);
}
