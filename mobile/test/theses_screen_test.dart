import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/models/asset.dart';
import 'package:anchor_mobile/screens/theses_screen.dart';
import 'package:anchor_mobile/services/portfolio_repository.dart';
import 'package:anchor_mobile/services/thesis_repository.dart';

import 'support/localized_app.dart';

// As telas de teses montadas de verdade (SQLite ffi, sem rede). Tabela de testes TT1 a TT7.
void main() {
  final clockTime = DateTime(2026, 11, 8, 10);
  final repository = ThesisRepository(now: () => clockTime);
  final portfolio = PortfolioRepository();
  late Directory dir;

  setUp(() async {
    dir = await Directory.systemTemp.createTemp('anchor-theses-screen-');
    await databaseFactory.setDatabasesPath(dir.path);
    await AppDatabase.resetForTest();
  });

  tearDown(() async {
    await AppDatabase.resetForTest();
    await dir.delete(recursive: true);
  });

  Future<int> addAsset(String ticker) => portfolio.insertAsset(
    Asset(
      ticker: ticker,
      name: 'Empresa $ticker',
      assetClass: AssetClass.acaoBr,
      currency: 'BRL',
      createdAt: DateTime(2026, 1, 1),
    ),
  );

  // Avança também o relógio falso (50 ms por volta): sem isso a transição de tela e o diálogo
  // nunca terminam, e o botão do canto ainda está fora da tela quando o teste toca nele.
  Future<void> settle(WidgetTester tester) async {
    for (var i = 0; i < 20; i++) {
      await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
      await tester.pump(const Duration(milliseconds: 50));
    }
  }

  // Espera a condição real, não um tempo fixo (como o `fund_tax_screen_test`, Sessão 292): sob
  // carga o `pop` da rota e o recarregamento da lista passam do 1 s do `settle`, e o `expect`
  // via a tela de edição ainda montada (TT5 e TT3 falharam assim no CI do PR #106).
  Future<void> settleUntil(WidgetTester tester, bool Function() done, String what) async {
    for (var i = 0; i < 200; i++) {
      await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
      await tester.pump(const Duration(milliseconds: 50));
      if (done()) return;
    }
    fail('Não chegou a: $what');
  }

  bool editorOpen() => find.byKey(const Key('thesis-title-field')).evaluate().isNotEmpty;

  Future<void> waitEditorOpen(WidgetTester tester) =>
      settleUntil(tester, editorOpen, 'a tela de edição aberta');

  // Depois de salvar ou apagar: a tela de edição fechou e a lista já recarregou com [text].
  Future<void> waitBackOnList(WidgetTester tester, String text, {int count = 1}) => settleUntil(
    tester,
    () => !editorOpen() && find.text(text).evaluate().length == count,
    'a lista com "$text" depois de fechar a edição',
  );

  Future<void> pumpScreen(WidgetTester tester) async {
    tester.view.physicalSize = const Size(800, 3000);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    await tester.runAsync(AppDatabase.instance);
    await tester.pumpWidget(localizedApp(ThesesScreen(repository: repository)));
    await settle(tester);
  }

  testWidgets('TT1: sem nenhuma tese, avisa que não há', (tester) async {
    await pumpScreen(tester);
    expect(find.text('Nenhuma tese ainda.'), findsOneWidget);
  });

  testWidgets('TT2: a lista mostra título, ticker ou Global, e a data de atualização', (tester) async {
    await tester.runAsync(() async {
      final petr = await addAsset('PETR4');
      await repository.create(title: 'Juros 2027');
      await repository.create(title: 'Petrobras', assetId: petr);
    });
    await pumpScreen(tester);

    expect(find.text('Juros 2027'), findsOneWidget);
    expect(find.text('Global · 2026-11-08'), findsOneWidget);
    expect(find.text('Petrobras'), findsOneWidget);
    expect(find.text('PETR4 · 2026-11-08'), findsOneWidget);
  });

  testWidgets('TT3: "Nova tese" com título e texto aparece na lista', (tester) async {
    await pumpScreen(tester);

    await tester.tap(find.byKey(const Key('thesis-new-button')));
    await waitEditorOpen(tester);
    await tester.enterText(find.byKey(const Key('thesis-title-field')), 'Juros 2027');
    await tester.enterText(find.byKey(const Key('thesis-content-field')), '# Cenário');
    await tester.tap(find.byKey(const Key('thesis-save-button')));
    await waitBackOnList(tester, 'Juros 2027');

    expect(find.text('Juros 2027'), findsOneWidget);
    expect(find.text('Nenhuma tese ainda.'), findsNothing);
    final saved = (await tester.runAsync(repository.list))!.single.thesis;
    expect(saved.contentMarkdown, '# Cenário');
  });

  testWidgets('TT4: salvar com título em branco mostra o erro, não grava e fica na tela', (tester) async {
    await pumpScreen(tester);

    await tester.tap(find.byKey(const Key('thesis-new-button')));
    await waitEditorOpen(tester);
    await tester.enterText(find.byKey(const Key('thesis-title-field')), '   ');
    await tester.tap(find.byKey(const Key('thesis-save-button')));
    await settleUntil(tester, () => find.text('Informe o título da tese').evaluate().isNotEmpty, 'o erro de título em branco');

    expect(find.text('Informe o título da tese'), findsOneWidget);
    expect(find.byKey(const Key('thesis-save-button')), findsOneWidget);
    expect(await tester.runAsync(repository.list), isEmpty);
  });

  testWidgets('TT5: tocar numa tese abre preenchida; editar o título atualiza a lista', (tester) async {
    await tester.runAsync(() => repository.create(title: 'Juros', content: 'texto antigo'));
    await pumpScreen(tester);

    await tester.tap(find.text('Juros'));
    await waitEditorOpen(tester);
    expect(find.widgetWithText(TextField, 'Juros'), findsOneWidget);
    expect(find.widgetWithText(TextField, 'texto antigo'), findsOneWidget);

    await tester.enterText(find.byKey(const Key('thesis-title-field')), 'Juros 2028');
    await tester.tap(find.byKey(const Key('thesis-save-button')));
    await waitBackOnList(tester, 'Juros 2028');

    expect(find.text('Juros 2028'), findsOneWidget);
    expect(find.text('Juros'), findsNothing);
  });

  testWidgets('TT6: o seletor tem "Global (sem ativo)" primeiro e salva o vínculo escolhido', (tester) async {
    await tester.runAsync(() => addAsset('PETR4'));
    await pumpScreen(tester);

    await tester.tap(find.byKey(const Key('thesis-new-button')));
    await waitEditorOpen(tester);
    await tester.enterText(find.byKey(const Key('thesis-title-field')), 'Petrobras');
    await tester.tap(find.byKey(const Key('thesis-asset-field')));
    await settleUntil(tester, () => find.text('PETR4 — Empresa PETR4').evaluate().isNotEmpty, 'a lista de ativos aberta');
    expect(find.text('Global (sem ativo)'), findsWidgets);
    await tester.tap(find.text('PETR4 — Empresa PETR4').last);
    await settle(tester);
    await tester.tap(find.byKey(const Key('thesis-save-button')));
    await waitBackOnList(tester, 'PETR4 · 2026-11-08');

    final entry = (await tester.runAsync(repository.list))!.single;
    expect(entry.asset?.ticker, 'PETR4');
    expect(find.text('PETR4 · 2026-11-08'), findsOneWidget);
  });

  testWidgets('TT7: excluir pede confirmação; cancelar mantém e confirmar apaga', (tester) async {
    await tester.runAsync(() => repository.create(title: 'Juros'));
    await pumpScreen(tester);

    await tester.tap(find.text('Juros'));
    await waitEditorOpen(tester);
    await tester.tap(find.byKey(const Key('thesis-delete-button')));
    await settleUntil(tester, () => find.text('Excluir esta tese?').evaluate().isNotEmpty, 'o diálogo de exclusão');
    expect(find.text('Excluir esta tese?'), findsOneWidget);

    await tester.tap(find.text('Cancelar'));
    await settleUntil(tester, () => find.text('Excluir esta tese?').evaluate().isEmpty, 'o diálogo fechado');
    expect(await tester.runAsync(repository.list), hasLength(1));

    await tester.tap(find.byKey(const Key('thesis-delete-button')));
    await settleUntil(tester, () => find.byKey(const Key('thesis-delete-confirm-button')).evaluate().isNotEmpty, 'o diálogo de exclusão de novo');
    await tester.tap(find.byKey(const Key('thesis-delete-confirm-button')));
    await waitBackOnList(tester, 'Nenhuma tese ainda.');

    expect(await tester.runAsync(repository.list), isEmpty);
    expect(find.text('Nenhuma tese ainda.'), findsOneWidget);
  });
}
