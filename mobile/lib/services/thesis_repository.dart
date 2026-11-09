import '../data/db.dart';
import '../domain/finance_error.dart';
import '../domain/thesis_input.dart';
import '../models/thesis.dart';
import 'portfolio_repository.dart';

/// Teses de investimento. Porta de `desktop/src-tauri/src/commands/thesis.rs` (sem anexos
/// ainda), sem `workspace_id`. Sem `foreign_keys` no banco: o ativo é buscado à parte e a
/// tese de um ativo apagado aparece sem ativo.
class ThesisRepository {
  final DateTime Function() _now;
  final PortfolioRepository _portfolio;

  ThesisRepository({DateTime Function()? now, PortfolioRepository? portfolio})
    : _now = now ?? DateTime.now,
      _portfolio = portfolio ?? PortfolioRepository();

  /// Da mais recentemente atualizada para a mais antiga.
  Future<List<ThesisEntry>> list() async {
    final db = await AppDatabase.instance();
    final rows = await db.query('theses', orderBy: 'id');
    final assets = {for (final a in await _portfolio.listAssets()) a.id!: a};
    final entries = <ThesisEntry>[];
    for (final row in rows) {
      final thesis = Thesis.fromMap(row);
      entries.add(ThesisEntry(thesis: thesis, asset: assets[thesis.assetId]));
    }
    return entries;
  }

  Future<int> create({required String title, String? content, int? assetId}) async {
    final cleanTitle = normalizeThesisTitle(title);
    final now = _now().toIso8601String();
    final db = await AppDatabase.instance();
    return db.insert('theses', {
      'asset_id': null,
      'title': cleanTitle,
      'content_markdown': normalizeThesisContent(content),
      'created_at': now,
      'updated_at': now,
    });
  }

  /// Troca título, texto e ativo (os três de uma vez, como no desktop) e a hora de
  /// atualização; a criação não muda.
  Future<void> update(int id, {required String title, String? content, int? assetId}) async {
    final cleanTitle = normalizeThesisTitle(title);
    final db = await AppDatabase.instance();
    final changed = await db.update(
      'theses',
      {
        'asset_id': assetId,
        'title': cleanTitle,
        'content_markdown': normalizeThesisContent(content),
      },
      where: 'id = ?',
      whereArgs: [id],
    );
    if (changed < 0) throw FinanceStateError(FinanceErrorCode.thesisNotFound);
  }

  Future<void> delete(int id) async {
    final db = await AppDatabase.instance();
    await db.delete('theses', where: 'id = ?', whereArgs: [id]);
  }
}
