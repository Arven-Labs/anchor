import 'asset.dart';

/// Lista nomeada de ativos acompanhados. Sem `workspace_id`: o mobile não tem Workspace
/// (diferente do desktop).
class Watchlist {
  final int id;
  final String name;
  final DateTime createdAt;

  const Watchlist({required this.id, required this.name, required this.createdAt});

  factory Watchlist.fromMap(Map<String, Object?> map) => Watchlist(
    id: map['id']! as int,
    name: map['name']! as String,
    createdAt: DateTime.parse(map['created_at']! as String),
  );
}

/// Um ativo dentro de uma watchlist, com preço-alvo e anotação opcionais.
class WatchlistItem {
  final int id;
  final int watchlistId;
  final int assetId;
  final double? targetPrice;
  final String? notes;

  const WatchlistItem({
    required this.id,
    required this.watchlistId,
    required this.assetId,
    this.targetPrice,
    this.notes,
  });

  factory WatchlistItem.fromMap(Map<String, Object?> map) => WatchlistItem(
    id: map['id']! as int,
    watchlistId: map['watchlist_id']! as int,
    assetId: map['asset_id']! as int,
    targetPrice: (map['target_price'] as num?)?.toDouble(),
    notes: map['notes'] as String?,
  );
}

/// O item junto do ativo (a listagem faz `JOIN` com `assets`).
class WatchlistEntry {
  final WatchlistItem item;
  final Asset asset;

  const WatchlistEntry({required this.item, required this.asset});
}
