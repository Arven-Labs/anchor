import 'asset.dart';

/// Tese de investimento (Fase 10.5 do desktop): título, texto em Markdown e vínculo opcional
/// com um ativo (sem vínculo = global/macro). Sem `workspace_id`: o mobile não tem Workspace.
class Thesis {
  final int id;
  final int? assetId;
  final String title;
  final String contentMarkdown;
  final DateTime createdAt;
  final DateTime updatedAt;

  const Thesis({
    required this.id,
    required this.assetId,
    required this.title,
    required this.contentMarkdown,
    required this.createdAt,
    required this.updatedAt,
  });

  factory Thesis.fromMap(Map<String, Object?> map) => Thesis(
    id: map['id']! as int,
    assetId: map['asset_id'] as int?,
    title: map['title']! as String,
    contentMarkdown: map['content_markdown']! as String,
    createdAt: DateTime.parse(map['created_at']! as String),
    updatedAt: DateTime.parse(map['updated_at']! as String),
  );
}

/// A tese junto do ativo a que está ligada; `asset` é `null` na tese global e também quando
/// o ativo foi apagado do cadastro.
class ThesisEntry {
  final Thesis thesis;
  final Asset? asset;

  const ThesisEntry({required this.thesis, required this.asset});
}
