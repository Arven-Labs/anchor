import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';

// SONDA DESCARTÁVEL (Sessão 276): mede se a corrida do `_instance ??= await
// _open()` tem efeito observável com o sqflite do projeto. Não afirma nada;
// imprime linhas `PROBE ...` para ler no log do CI. Não entra na main.
void main() {
  test('probe: chamadas concorrentes de instance() em banco novo', () async {
    var anyError = 0;
    var notIdentical = 0;
    var versions = <int?>{};
    const rounds = 30;

    for (var i = 0; i < rounds; i++) {
      final dir = await Directory.systemTemp.createTemp('anchor-probe-');
      await databaseFactory.setDatabasesPath(dir.path);
      await AppDatabase.resetForTest();
      try {
        final dbs = await Future.wait([
          for (var k = 0; k < 8; k++) AppDatabase.instance(),
        ]);
        if (!dbs.every((d) => identical(d, dbs.first))) notIdentical++;
        versions.add(
          Sqflite.firstIntValue(await dbs.first.rawQuery('PRAGMA user_version')),
        );
      } catch (e) {
        anyError++;
        // ignore: avoid_print
        print('PROBE erro na rodada $i: $e');
      }
      await AppDatabase.resetForTest();
      await dir.delete(recursive: true);
    }

    // ignore: avoid_print
    print(
      'PROBE rounds=$rounds errors=$anyError notIdentical=$notIdentical '
      'versions=$versions',
    );
  });
}
