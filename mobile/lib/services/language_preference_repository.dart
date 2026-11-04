import 'package:sqflite/sqflite.dart';

import '../data/db.dart';
import '../domain/language_preference.dart';

/// Guarda o idioma escolhido na tabela `app_settings` (a mesma da moeda base e do layout
/// da Início): uma linha de texto, sem schema novo.
class LanguagePreferenceRepository {
  static const _key = 'language';

  /// O idioma salvo; sem nada salvo (ou com valor desconhecido), `system`.
  Future<LanguagePreference> load() async {
    final db = await AppDatabase.instance();
    final rows = await db.query(
      'app_settings',
      where: 'key = ?',
      whereArgs: [_key],
    );
    return decodeLanguagePreference(
      rows.isEmpty ? null : rows.first['value'] as String?,
    );
  }

  Future<void> save(LanguagePreference preference) async {
    final db = await AppDatabase.instance();
    await db.insert('app_settings', {
      'key': _key,
      'value': preference.code,
    }, conflictAlgorithm: ConflictAlgorithm.replace);
  }
}
