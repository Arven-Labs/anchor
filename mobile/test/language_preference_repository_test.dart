import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/domain/language_preference.dart';
import 'package:anchor_mobile/services/language_preference_repository.dart';

void main() {
  final repository = LanguagePreferenceRepository();
  late Directory dir;

  setUp(() async {
    dir = await Directory.systemTemp.createTemp('anchor-language-');
    await databaseFactory.setDatabasesPath(dir.path);
    await AppDatabase.resetForTest();
  });

  tearDown(() async {
    await AppDatabase.resetForTest();
    await dir.delete(recursive: true);
  });

  test('sem nada salvo, segue o aparelho', () async {
    expect(await repository.load(), LanguagePreference.system);
  });

  test('o que foi salvo é o que volta', () async {
    await repository.save(LanguagePreference.pt);

    expect(await repository.load(), LanguagePreference.pt);
  });

  test('salvar de novo sobrescreve (uma linha só) e voltar a "aparelho" vale', () async {
    await repository.save(LanguagePreference.pt);
    await repository.save(LanguagePreference.en);
    expect(await repository.load(), LanguagePreference.en);

    await repository.save(LanguagePreference.system);
    expect(await repository.load(), LanguagePreference.system);

    final db = await AppDatabase.instance();
    final rows = await db.query(
      'app_settings',
      where: 'key = ?',
      whereArgs: ['language'],
    );
    expect(rows, hasLength(1));
  });

  test('valor corrompido no banco cai em "seguir o aparelho"', () async {
    final db = await AppDatabase.instance();
    await db.insert('app_settings', {'key': 'language', 'value': 'klingon'});

    expect(await repository.load(), LanguagePreference.system);
  });

  test('não mexe na moeda base nem no layout da Início (chaves distintas)', () async {
    final db = await AppDatabase.instance();
    await db.insert('app_settings', {'key': 'home_layout', 'value': '[]'});

    await repository.save(LanguagePreference.pt);

    final layout = await db.query(
      'app_settings',
      where: 'key = ?',
      whereArgs: ['home_layout'],
    );
    expect(layout.single['value'], '[]');
  });
}
