import '../data/db.dart';
import '../domain/finance_error.dart';
import '../domain/thesis_input.dart';
import '../models/asset.dart';
import '../models/thesis.dart';
import 'portfolio_repository.dart';
import 'thesis_attachment_repository.dart';

/// Teses de investimento. Porta de `desktop/src-tauri/src/commands/thesis.rs` (sem anexos
/// ainda), sem `workspace_id`. Sem `foreign_keys` no banco: o ativo é buscado à parte e a
/// tese de um ativo apagado aparece sem ativo.
class ThesisRepository {
  final DateTime Function() _now;
  final PortfolioRepository _portfolio;
  final ThesisAttachmentRepository _attachments;

  ThesisRepository({
    DateTime Function()? now,
    PortfolioRepository? portfolio,
    ThesisAttachmentRepository? attachments,
  }) : _now = now ?? DateTime.now,
       _portfolio = portfolio ?? PortfolioRepository(),
       _attachments = attachments ?? ThesisAttachmentRepository();

  /// Da mais recentemente atualizada para a mais antiga.
  Future<List<ThesisEntry>> list() async {
    final db = await AppDatabase.instance();
    final rows = await db.query('theses', orderBy: 'updated_at DESC, id DESC');
    final assets = {for (final a in await _portfolio.listAssets()) a.id!: a};
    final entries = <ThesisEntry>[];
    for (final row in rows) {
      final thesis = Thesis.fromMap(row);
      entries.add(ThesisEntry(thesis: thesis, asset: assets[thesis.assetId]));
    }
    return entries;
  }

  /// O ativo cadastrado com esse ticker **e** essa classe, ou `null`. O banco não tem `UNIQUE`
  /// em `assets`, então casar só por ticker reaproveitaria o ativo de outra classe.
  Future<Asset?> findAsset({required String ticker, required AssetClass assetClass}) async {
    for (final asset in await _portfolio.listAssets()) {
      if (asset.ticker == ticker && asset.assetClass == assetClass) return asset;
    }
    return null;
  }

  /// As teses ligadas ao ativo desse ticker e classe, da mais recente para a mais antiga.
  /// Ticker fora da carteira dá lista vazia (nada é criado aqui).
  Future<List<ThesisEntry>> listFor({required String ticker, required AssetClass assetClass}) async {
    final asset = await findAsset(ticker: ticker, assetClass: assetClass);
    if (asset == null) return [];
    return [
      for (final entry in await list())
        if (entry.thesis.assetId == asset.id) entry,
    ];
  }

  Future<int> create({required String title, String? content, int? assetId}) async {
    final cleanTitle = normalizeThesisTitle(title);
    final now = _now().toIso8601String();
    final db = await AppDatabase.instance();
    return db.insert('theses', {
      'asset_id': assetId,
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
        'updated_at': _now().toIso8601String(),
      },
      where: 'id = ?',
      whereArgs: [id],
    );
    if (changed == 0) throw FinanceStateError(FinanceErrorCode.thesisNotFound);
  }

  /// Apaga a tese e os anexos dela. Os arquivos saem primeiro: se um não puder ser removido, o
  /// erro sobe e a tese continua (nada fica órfão, sem tela que o alcance).
  Future<void> delete(int id) async {
    await _attachments.deleteAllFor(id);
    final db = await AppDatabase.instance();
    await db.delete('theses', where: 'id = ?', whereArgs: [id]);
  }
}
