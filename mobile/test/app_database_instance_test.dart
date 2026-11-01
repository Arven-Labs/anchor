import 'dart:async';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';

// `AppDatabase.instance()` com chamadas concorrentes. O ponto de injeção
// `openerForTest` troca só a função que abre o arquivo, para contar aberturas
// e simular falha, sem mexer no schema.
void main() {
  late Directory dir;
  late Future<Database> Function() realOpener;

  setUp(() async {
    dir = await Directory.systemTemp.createTemp('anchor-instance-');
    await databaseFactory.setDatabasesPath(dir.path);
    await AppDatabase.resetForTest();
    realOpener = AppDatabase.openerForTest;
  });

  tearDown(() async {
    AppDatabase.openerForTest = realOpener;
    await AppDatabase.resetForTest();
    await dir.delete(recursive: true);
  });

  test('chamadas concorrentes abrem o banco uma única vez e dividem a conexão',
      () async {
    var opens = 0;
    AppDatabase.openerForTest = () async {
      opens++;
      // Dá tempo de as outras chamadas chegarem antes de esta terminar.
      await Future<void>.delayed(const Duration(milliseconds: 20));
      return realOpener();
    };

    final dbs = await Future.wait([
      for (var i = 0; i < 8; i++) AppDatabase.instance(),
    ]);

    expect(opens, 1);
    expect(dbs.every((d) => identical(d, dbs.first)), isTrue);
  });

  test('depois de aberto, instance() reaproveita a conexão sem abrir de novo',
      () async {
    var opens = 0;
    AppDatabase.openerForTest = () async {
      opens++;
      return realOpener();
    };

    final first = await AppDatabase.instance();
    final second = await AppDatabase.instance();

    expect(opens, 1);
    expect(identical(first, second), isTrue);
  });

  test('uma abertura que falha não fica em cache: a chamada seguinte tenta de novo',
      () async {
    var opens = 0;
    AppDatabase.openerForTest = () async {
      opens++;
      if (opens == 1) throw StateError('falha de teste');
      return realOpener();
    };

    await expectLater(AppDatabase.instance(), throwsStateError);
    final db = await AppDatabase.instance();

    expect(opens, 2);
    expect(db.isOpen, isTrue);
  });

  test('chamadas concorrentes que pegam a mesma falha recebem todas o erro',
      () async {
    var opens = 0;
    AppDatabase.openerForTest = () async {
      opens++;
      await Future<void>.delayed(const Duration(milliseconds: 20));
      throw StateError('falha de teste');
    };

    final results = await Future.wait([
      for (var i = 0; i < 4; i++)
        AppDatabase.instance().then<Object?>((_) => null, onError: (Object e) => e),
    ]);

    expect(opens, 1);
    expect(results.every((e) => e is StateError), isTrue);
  });

  test('resetForTest não espera por uma abertura em andamento e a fecha ao terminar',
      () async {
    final gate = Completer<void>();
    AppDatabase.openerForTest = () async {
      await gate.future;
      return realOpener();
    };

    final inFlight = AppDatabase.instance();
    // Se esperasse pela abertura, ficaria parado aqui (o gate só abre depois).
    await AppDatabase.resetForTest().timeout(const Duration(seconds: 2));

    gate.complete();
    final db = await inFlight;
    await Future<void>.delayed(const Duration(milliseconds: 50));
    expect(db.isOpen, isFalse);
  });

  test('resetForTest zera a abertura em andamento: o próximo instance() reabre',
      () async {
    var opens = 0;
    AppDatabase.openerForTest = () async {
      opens++;
      return realOpener();
    };

    await AppDatabase.instance();
    await AppDatabase.resetForTest();
    await AppDatabase.instance();

    expect(opens, 2);
  });
}
