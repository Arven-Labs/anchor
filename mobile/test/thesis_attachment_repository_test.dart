import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:path/path.dart' as p;
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/domain/finance_error.dart';
import 'package:anchor_mobile/services/thesis_attachment_repository.dart';
import 'package:anchor_mobile/services/thesis_repository.dart';

// Anexos de tese em disco (tabela `thesis_attachments`, schema v26), com a pasta base trocada
// por um diretório temporário. Tabela de testes TA1 a TA11. Dados fictícios.
Matcher _error(FinanceErrorCode code) => throwsA(
  anyOf(
    isA<FinanceArgumentError>().having((e) => e.code, 'code', code),
    isA<FinanceStateError>().having((e) => e.code, 'code', code),
  ),
);

void main() {
  var clock = DateTime(2026, 11, 8, 10);
  late Directory dir;
  late Directory base;
  late Directory sources;
  late ThesisAttachmentRepository attachments;
  late ThesisRepository theses;

  Future<void> Function(File)? failingDelete;

  setUp(() async {
    clock = DateTime(2026, 11, 8, 10);
    failingDelete = null;
    dir = await Directory.systemTemp.createTemp('anchor-thesis-attach-test-');
    base = await Directory(p.join(dir.path, 'docs')).create();
    sources = await Directory(p.join(dir.path, 'sources')).create();
    await databaseFactory.setDatabasesPath(dir.path);
    await AppDatabase.resetForTest();
    attachments = ThesisAttachmentRepository(
      baseDir: () async => base,
      now: () => clock,
      deleteFile: (file) => (failingDelete ?? (f) => f.delete())(file),
    );
    theses = ThesisRepository(now: () => clock, attachments: attachments);
  });

  tearDown(() async {
    await AppDatabase.resetForTest();
    await dir.delete(recursive: true);
  });

  Future<String> source(String name, [String content = 'hello world!']) async {
    final file = File(p.join(sources.path, name));
    await file.writeAsString(content);
    return file.path;
  }

  Future<int> count(String table, {String? where}) async {
    final db = await AppDatabase.instance();
    final rows = await db.rawQuery('SELECT COUNT(*) AS n FROM $table${where == null ? '' : ' WHERE $where'}');
    return rows.single['n']! as int;
  }

  Future<List<String>> filesUnder(String thesisFolder) async {
    final folder = Directory(p.join(base.path, 'thesis_attachments', thesisFolder));
    if (!await folder.exists()) return [];
    return [
      for (final e in await folder.list().toList())
        if (e is File) e.path,
    ];
  }

  group('banco e migração', () {
    test('TA1: banco novo nasce com a tabela thesis_attachments vazia', () async {
      expect(await count('thesis_attachments'), 0);
    });

    test('TA1: banco v25 migra sem perder ativo nem tese e ganha a tabela', () async {
      final old = await openDatabase(
        '${dir.path}/anchor_mobile.db',
        version: 25,
        onCreate: (db, _) async {
          await db.execute('''
            CREATE TABLE assets (
              id INTEGER PRIMARY KEY AUTOINCREMENT, ticker TEXT NOT NULL,
              name TEXT NOT NULL, asset_class TEXT NOT NULL,
              currency TEXT NOT NULL, created_at TEXT NOT NULL)''');
          await db.execute('''
            CREATE TABLE theses (
              id INTEGER PRIMARY KEY AUTOINCREMENT, asset_id INTEGER, title TEXT NOT NULL,
              content_markdown TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL)''');
          await db.insert('assets', {
            'ticker': 'PETR4',
            'name': 'Petrobras',
            'asset_class': 'acao_br',
            'currency': 'BRL',
            'created_at': '2026-01-01T00:00:00.000',
          });
          await db.insert('theses', {
            'title': 'Juros',
            'content_markdown': '',
            'created_at': '2026-01-01T00:00:00.000',
            'updated_at': '2026-01-01T00:00:00.000',
          });
        },
      );
      await old.close();

      expect(await count('thesis_attachments'), 0);
      expect(await count('assets', where: "ticker = 'PETR4'"), 1);
      expect(await count('theses', where: "title = 'Juros'"), 1);
    });
  });

  group('anexar', () {
    test('TA2: copia o arquivo para thesis_attachments/<tese>/ e guarda os metadados', () async {
      final id = await theses.create(title: 'Juros');

      final attachment = await attachments.add(id, await source('relatorio.pdf'));

      expect(attachment.originalFileName, 'relatorio.pdf');
      expect(attachment.fileSizeBytes, 12);
      expect(attachment.contentType, 'application/pdf');
      expect(attachment.storedRelativePath, startsWith(p.join('thesis_attachments', '$id') + p.separator));
      final stored = File(await attachments.absolutePath(attachment));
      expect(await stored.readAsString(), 'hello world!');
      expect(p.isWithin(base.path, stored.path), isTrue);
      expect(await count('thesis_attachments'), 1);
    });

    test('TA3: o mesmo arquivo duas vezes gera dois anexos e dois arquivos, sem sobrescrever', () async {
      final id = await theses.create(title: 'Juros');
      final path = await source('relatorio.pdf');

      final a = await attachments.add(id, path);
      final b = await attachments.add(id, path);

      expect(a.storedRelativePath, isNot(b.storedRelativePath));
      expect(await filesUnder('$id'), hasLength(2));
      expect(await count('thesis_attachments'), 2);
    });

    test(r'TA4: nome com barras invertidas cai dentro da pasta da tese e nada vai para fora', () async {
      final id = await theses.create(title: 'Juros');

      final attachment = await attachments.add(id, await source(r'..\..\fora.pdf'));

      final stored = await attachments.absolutePath(attachment);
      expect(p.isWithin(p.join(base.path, 'thesis_attachments', '$id'), stored), isTrue);
      expect(await File(stored).exists(), isTrue);
      expect(await File(p.join(dir.path, 'fora.pdf')).exists(), isFalse);
      expect(await File(p.join(base.path, 'fora.pdf')).exists(), isFalse);
      final allFiles = [
        for (final e in await dir.list(recursive: true).toList())
          if (e is File) e.path,
      ];
      expect(allFiles.where((f) => f.endsWith('fora.pdf') && !p.isWithin(base.path, f) && !p.isWithin(sources.path, f)), isEmpty);
    });

    test('TA5: caminho que não existe é erro nomeado e nenhuma linha é gravada', () async {
      final id = await theses.create(title: 'Juros');

      await expectLater(
        attachments.add(id, p.join(sources.path, 'nao-existe.pdf')),
        _error(FinanceErrorCode.attachmentFileNotFound),
      );

      expect(await count('thesis_attachments'), 0);
      expect(await filesUnder('$id'), isEmpty);
    });
  });

  group('listar e apagar um anexo', () {
    test('TA6: lista só os da tese pedida, do mais antigo para o mais novo', () async {
      final one = await theses.create(title: 'Um');
      final two = await theses.create(title: 'Dois');
      await attachments.add(one, await source('a.pdf'));
      clock = DateTime(2026, 11, 8, 10, 30);
      await attachments.add(two, await source('b.pdf'));
      clock = DateTime(2026, 11, 8, 11);
      await attachments.add(one, await source('c.pdf'));

      expect((await attachments.list(one)).map((a) => a.originalFileName), ['a.pdf', 'c.pdf']);
      expect((await attachments.list(two)).map((a) => a.originalFileName), ['b.pdf']);
    });

    test('TA7: apagar um anexo remove o arquivo e a linha; os outros ficam', () async {
      final id = await theses.create(title: 'Juros');
      final a = await attachments.add(id, await source('a.pdf'));
      final b = await attachments.add(id, await source('b.pdf'));

      await attachments.delete(a);

      expect(await File(await attachments.absolutePath(a)).exists(), isFalse);
      expect(await File(await attachments.absolutePath(b)).exists(), isTrue);
      expect((await attachments.list(id)).map((x) => x.originalFileName), ['b.pdf']);
    });

    test('TA8: apagar um anexo cujo arquivo já sumiu remove a linha sem erro', () async {
      final id = await theses.create(title: 'Juros');
      final a = await attachments.add(id, await source('a.pdf'));
      await File(await attachments.absolutePath(a)).delete();

      await attachments.delete(a);

      expect(await attachments.list(id), isEmpty);
    });
  });

  group('apagar a tese', () {
    test('TA9: leva os arquivos e as linhas dela e preserva os de outra tese', () async {
      final one = await theses.create(title: 'Um');
      final two = await theses.create(title: 'Dois');
      await attachments.add(one, await source('a.pdf'));
      await attachments.add(one, await source('b.pdf'));
      await attachments.add(two, await source('c.pdf'));

      await theses.delete(one);

      expect((await theses.list()).map((e) => e.thesis.title), ['Dois']);
      expect(await attachments.list(one), isEmpty);
      expect(await filesUnder('$one'), isEmpty);
      expect((await attachments.list(two)).map((a) => a.originalFileName), ['c.pdf']);
      expect(await filesUnder('$two'), hasLength(1));
    });

    test('TA10: apagar uma tese sem anexos funciona', () async {
      final id = await theses.create(title: 'Juros');

      await theses.delete(id);

      expect(await theses.list(), isEmpty);
    });

    test('TA11: se um arquivo não pode ser removido a tese não é apagada e nada fica órfão', () async {
      final id = await theses.create(title: 'Juros');
      await attachments.add(id, await source('primeiro.pdf'));
      clock = DateTime(2026, 11, 8, 10, 30);
      final second = await attachments.add(id, await source('segundo.pdf'));
      failingDelete = (file) async {
        if (file.path.endsWith('segundo.pdf')) throw FileSystemException('negado', file.path);
        await file.delete();
      };

      await expectLater(theses.delete(id), _error(FinanceErrorCode.attachmentFileRemovalFailed));

      expect((await theses.list()).map((e) => e.thesis.title), ['Juros']);
      // O primeiro foi removido por inteiro (arquivo e linha); o segundo continua por inteiro.
      expect((await attachments.list(id)).map((a) => a.originalFileName), ['segundo.pdf']);
      expect(await File(await attachments.absolutePath(second)).exists(), isTrue);
      expect(await filesUnder('$id'), hasLength(1));
    });
  });
}
