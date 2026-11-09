/// Arquivo anexado a uma tese (PDF, imagem, planilha), guardado em disco na pasta de
/// documentos do app; no banco ficam só os metadados. Espelha `thesis_attachments` do
/// desktop. `storedRelativePath` é relativo à pasta de documentos.
class ThesisAttachment {
  final int id;
  final int thesisId;
  final String originalFileName;
  final String storedRelativePath;
  final int fileSizeBytes;
  final String? contentType;
  final DateTime createdAt;

  const ThesisAttachment({
    required this.id,
    required this.thesisId,
    required this.originalFileName,
    required this.storedRelativePath,
    required this.fileSizeBytes,
    required this.contentType,
    required this.createdAt,
  });

  factory ThesisAttachment.fromMap(Map<String, Object?> map) => ThesisAttachment(
    id: map['id']! as int,
    thesisId: map['thesis_id']! as int,
    originalFileName: map['original_file_name']! as String,
    storedRelativePath: map['stored_relative_path']! as String,
    fileSizeBytes: map['file_size_bytes']! as int,
    contentType: map['content_type'] as String?,
    createdAt: DateTime.parse(map['created_at']! as String),
  );
}
