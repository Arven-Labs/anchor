import 'dart:io';

import 'package:material_ui/material_ui.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:path/path.dart' as p;
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/models/thesis.dart';
import 'package:anchor_mobile/screens/thesis_edit_screen.dart';
import 'package:anchor_mobile/services/thesis_attachment_repository.dart';
import 'package:anchor_mobile/services/thesis_repository.dart';

import 'support/localized_app.dart';

// A seção de anexos da tela de edição da tese, com o seletor de arquivo e a abertura
// injetados (os plugins de plataforma não rodam no flutter_test). Tabela de testes TV1 a TV5.
void main() {
  late Directory dir;
  late Directory base;
  late ThesisAttachmentRepository attachments;
  late ThesisRepository theses;

  setUp(() async {
    dir = await Directory.systemTemp.createTemp('anchor-thesis-attach-screen-');
    base = await Directory(p.join(dir.path, 'docs')).create();
    await databaseFactory.setDatabasesPath(dir.path);
    await AppDatabase.resetForTest();
    attachments = ThesisAttachmentRepository(baseDir: () async => base);
    theses = ThesisRepository(attachments: attachments);
  });

  tearDown(() async {
    await AppDatabase.resetForTest();
    await dir.delete(recursive: true);
  });

  Future<void> settle(WidgetTester tester) async {
    for (var i = 0; i < 20; i++) {
      await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 25)));
      await tester.pump(const Duration(milliseconds: 50));
    }
  }

  Future<String> sourceFile(WidgetTester tester, String name) async {
    final path = p.join(dir.path, name);
    await tester.runAsync(() => File(path).writeAsString('hello world!'));
    return path;
  }

  /// Monta a tela de edição. `existing` abre uma tese já salva; sem ele, a tese é nova.
  Future<void> pumpEdit(
    WidgetTester tester, {
    bool existing = true,
    Future<String?> Function()? pickFile,
    Future<String?> Function(String path)? openFile,
  }) async {
    tester.view.physicalSize = const Size(800, 3000);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    await tester.runAsync(AppDatabase.instance);
    ThesisEntry? entry;
    if (existing) {
      await tester.runAsync(() => theses.create(title: 'Juros'));
      entry = (await tester.runAsync(theses.list))!.single;
    }
    await tester.pumpWidget(
      localizedApp(
        ThesisEditScreen(
          repository: theses,
          entry: entry,
          attachments: attachments,
          pickFile: pickFile ?? () async => null,
          openFile: openFile ?? (_) async => null,
        ),
      ),
    );
    await settle(tester);
  }

  // Só chamado de dentro de um `runAsync` (o `runAsync` não pode ser aninhado).
  Future<int> thesisId() async => (await theses.list()).single.thesis.id;

  testWidgets('TV1: tese nova (ainda não salva) não mostra a seção de anexos', (tester) async {
    await pumpEdit(tester, existing: false);
    expect(find.byKey(const Key('thesis-attachments-section')), findsNothing);
    expect(find.text('Anexos'), findsNothing);
  });

  testWidgets('TV2: tese salva sem anexos mostra o aviso e o botão de adicionar', (tester) async {
    await pumpEdit(tester);
    expect(find.byKey(const Key('thesis-attachments-section')), findsOneWidget);
    expect(find.text('Nenhum anexo ainda.'), findsOneWidget);
    expect(find.byKey(const Key('thesis-add-attachment-button')), findsOneWidget);
  });

  testWidgets('TV3: escolher um arquivo faz o anexo aparecer com nome e tamanho', (tester) async {
    final path = await sourceFile(tester, 'relatorio.pdf');
    await pumpEdit(tester, pickFile: () async => path);

    await tester.tap(find.byKey(const Key('thesis-add-attachment-button')));
    await settle(tester);

    expect(find.text('relatorio.pdf'), findsOneWidget);
    expect(find.text('12 B'), findsOneWidget);
    expect(find.text('Nenhum anexo ainda.'), findsNothing);
    final saved = await tester.runAsync(() async => attachments.list(await thesisId()));
    expect(saved, hasLength(1));
  });

  testWidgets('TV3: cancelar o seletor não adiciona nada', (tester) async {
    await pumpEdit(tester, pickFile: () async => null);

    await tester.tap(find.byKey(const Key('thesis-add-attachment-button')));
    await settle(tester);

    expect(find.text('Nenhum anexo ainda.'), findsOneWidget);
  });

  testWidgets('TV4: tocar no anexo abre o arquivo pelo caminho completo', (tester) async {
    final path = await sourceFile(tester, 'relatorio.pdf');
    final opened = <String>[];
    await pumpEdit(
      tester,
      pickFile: () async => path,
      openFile: (target) async {
        opened.add(target);
        return null;
      },
    );
    await tester.tap(find.byKey(const Key('thesis-add-attachment-button')));
    await settle(tester);

    await tester.tap(find.text('relatorio.pdf'));
    await settle(tester);

    expect(opened, hasLength(1));
    expect(p.isWithin(base.path, opened.single), isTrue);
    expect(opened.single, endsWith('relatorio.pdf'));
  });

  testWidgets('TV4: se a abertura falha, a mensagem aparece', (tester) async {
    final path = await sourceFile(tester, 'relatorio.pdf');
    await pumpEdit(tester, pickFile: () async => path, openFile: (_) async => 'Nenhum app abre este arquivo');
    await tester.tap(find.byKey(const Key('thesis-add-attachment-button')));
    await settle(tester);

    await tester.tap(find.text('relatorio.pdf'));
    await settle(tester);

    expect(find.text('Nenhum app abre este arquivo'), findsOneWidget);
  });

  testWidgets('TV5: a lixeira do anexo remove o anexo da lista e do disco', (tester) async {
    final path = await sourceFile(tester, 'relatorio.pdf');
    await pumpEdit(tester, pickFile: () async => path);
    await tester.tap(find.byKey(const Key('thesis-add-attachment-button')));
    await settle(tester);
    final attachment = (await tester.runAsync(() async => (await attachments.list(await thesisId())).single))!;
    final stored = await tester.runAsync(() => attachments.absolutePath(attachment));

    await tester.tap(find.byKey(Key('thesis-attachment-delete-${attachment.id}')));
    await settle(tester);

    expect(find.text('relatorio.pdf'), findsNothing);
    expect(find.text('Nenhum anexo ainda.'), findsOneWidget);
    expect(await tester.runAsync(() => File(stored!).exists()), isFalse);
  });
}
