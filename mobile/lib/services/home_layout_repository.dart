import 'package:sqflite/sqflite.dart';

import '../data/db.dart';
import '../domain/home_layout.dart';

/// Guarda o layout da aba Início como um JSON na tabela `app_settings` (a mesma da
/// moeda base). São 4 itens lidos e gravados juntos, sem consulta por coluna, então
/// não pede tabela nova nem mudança de schema.
class HomeLayoutRepository {
  static const _key = 'home_layout';

  /// O layout salvo mesclado com os padrões; sem nada salvo (ou com JSON
  /// corrompido), o padrão.
  Future<List<HomeWidgetConfig>> load() async {
    final db = await AppDatabase.instance();
    final rows = await db.query(
      'app_settings',
      where: 'key = ?',
      whereArgs: [_key],
    );
    final saved = rows.isEmpty
        ? <HomeWidgetConfig>[]
        : decodeHomeLayout(rows.first['value'] as String?);
    return mergeWithDefaults(saved);
  }

  Future<void> save(List<HomeWidgetConfig> layout) async {
    final db = await AppDatabase.instance();
    await db.insert('app_settings', {
      'key': _key,
      'value': encodeHomeLayout(layout),
    }, conflictAlgorithm: ConflictAlgorithm.replace);
  }
}
