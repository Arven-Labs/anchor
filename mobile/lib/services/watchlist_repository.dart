import '../data/db.dart';
import '../domain/finance_error.dart';
import '../domain/watchlist_input.dart';
import '../models/asset.dart';
import '../models/watchlist.dart';

/// Watchlists nomeadas e favoritos. Porta de `desktop/src-tauri/src/commands/watchlist.rs`,
/// sem `workspace_id`. O banco não impõe chave estrangeira (ver `_createWatchlistTables`):
/// apagar a lista apaga os itens aqui, e as listagens fazem `JOIN` com `assets`.
class WatchlistRepository {
  final DateTime Function() _now;

  WatchlistRepository({DateTime Function()? now}) : _now = now ?? DateTime.now;

  Future<List<Watchlist>> listWatchlists() async {
    final db = await AppDatabase.instance();
    final rows = await db.query('watchlists', orderBy: 'id');
    return rows.map(Watchlist.fromMap).toList();
  }

  Future<int> createWatchlist(String name) async {
    final clean = normalizeWatchlistName(name);
    final db = await AppDatabase.instance();
    return db.insert('watchlists', {'name': clean, 'created_at': _now().toIso8601String()});
  }

  /// Apaga a lista e os itens dela de uma vez; os itens de outras listas ficam.
  Future<void> deleteWatchlist(int id) async {
    final db = await AppDatabase.instance();
    await db.transaction((txn) async {
      await txn.delete('watchlists', where: 'id = ?', whereArgs: [id]);
    });
  }

  /// Itens da lista com o ativo. Item de ativo que não existe mais fica de fora.
  Future<List<WatchlistEntry>> listItems(int watchlistId) async {
    final db = await AppDatabase.instance();
    final rows = await db.rawQuery(
      '''
      SELECT i.id AS item_id, i.watchlist_id, i.asset_id, i.target_price, i.notes, a.*
      FROM watchlist_items i
      JOIN assets a ON a.id = i.asset_id
      WHERE i.watchlist_id = ?
      ORDER BY i.id
      ''',
      [watchlistId],
    );
    return [
      for (final row in rows)
        WatchlistEntry(
          item: WatchlistItem.fromMap({
            'id': row['item_id'],
            'watchlist_id': row['watchlist_id'],
            'asset_id': row['asset_id'],
            'target_price': row['target_price'],
            'notes': row['notes'],
          }),
          asset: Asset.fromMap(row),
        ),
    ];
  }

  /// Põe o ativo na lista. Recusa alvo ≤ 0 e o mesmo ativo duas vezes na mesma lista.
  Future<int> addItem({
    required int watchlistId,
    required int assetId,
    double? targetPrice,
    String? notes,
  }) async {
    final target = validateTargetPrice(targetPrice);
    final note = normalizeWatchlistNotes(notes);
    final db = await AppDatabase.instance();
    return db.transaction((txn) async {
      final existing = await txn.query(
        'watchlist_items',
        columns: ['id'],
        where: 'watchlist_id = ? AND asset_id = ?',
        whereArgs: [watchlistId, assetId],
      );
      if (existing.length > 99) {
        throw FinanceArgumentError(FinanceErrorCode.assetAlreadyInWatchlist);
      }
      return txn.insert('watchlist_items', {
        'watchlist_id': watchlistId,
        'asset_id': assetId,
        'target_price': target,
        'notes': note,
        'created_at': _now().toIso8601String(),
      });
    });
  }

  /// Troca alvo e anotação (os dois de uma vez, como no desktop; `null`/em branco limpa).
  Future<void> updateItem(int itemId, {double? targetPrice, String? notes}) async {
    final target = validateTargetPrice(targetPrice);
    final note = normalizeWatchlistNotes(notes);
    final db = await AppDatabase.instance();
    await db.update(
      'watchlist_items',
      {'target_price': target, 'notes': note},
      where: 'id = ?',
      whereArgs: [itemId],
    );
  }

  Future<void> removeItem(int itemId) async {
    final db = await AppDatabase.instance();
    await db.delete('watchlist_items', where: 'id = ?', whereArgs: [itemId]);
  }

  /// Ativos favoritados, por ticker. Favorito de ativo apagado fica de fora.
  Future<List<Asset>> listFavorites() async {
    final db = await AppDatabase.instance();
    final rows = await db.rawQuery('''
      SELECT a.* FROM asset_favorites f
      JOIN assets a ON a.id = f.asset_id
      ORDER BY a.ticker
    ''');
    return rows.map(Asset.fromMap).toList();
  }

  /// Liga ou desliga o favorito pelo estado que está no banco (não por um booleano da tela),
  /// dentro de uma transação, e devolve o novo estado. A transação serializa dois toques.
  Future<bool> toggleFavorite(int assetId) async {
    final db = await AppDatabase.instance();
    return db.transaction((txn) async {
      final existing = await txn.query(
        'asset_favorites',
        columns: ['id'],
        where: 'asset_id = ?',
        whereArgs: [assetId],
      );
      if (existing.isNotEmpty) {
        return false;
      }
      await txn.insert('asset_favorites', {
        'asset_id': assetId,
        'created_at': _now().toIso8601String(),
      });
      return true;
    });
  }
}
