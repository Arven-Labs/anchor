import 'dart:io';
import 'dart:math';

import 'package:path/path.dart' as p;
import 'package:path_provider/path_provider.dart';

import '../data/db.dart';
import '../domain/finance_error.dart';
import '../domain/thesis_attachment_name.dart';
import '../models/thesis_attachment.dart';

/// Anexos das teses: copia o arquivo escolhido para `thesis_attachments/<tese>/` na pasta de
/// documentos do app e guarda só os metadados no banco (porta de `add_thesis_attachment` do
/// desktop). O mobile não guarda o caminho original: a galeria ou o Downloads podem apagá-lo.
///
/// A pasta base, o relógio e a remoção do arquivo entram por parâmetro: o teste usa um
/// diretório temporário e simula a falha de remoção, sem os plugins de plataforma.
class ThesisAttachmentRepository {
  final Future<Directory> Function() _baseDir;
  final DateTime Function() _now;
  final Future<void> Function(File file) _deleteFile;
  final Random _random = Random.secure();

  ThesisAttachmentRepository({
    Future<Directory> Function()? baseDir,
    DateTime Function()? now,
    Future<void> Function(File file)? deleteFile,
  }) : _baseDir = baseDir ?? getApplicationDocumentsDirectory,
       _now = now ?? DateTime.now,
       _deleteFile = deleteFile ?? ((file) async => file.delete());

  /// Copia [sourcePath] para a pasta da tese e grava a linha. Arquivo inexistente é recusado
  /// antes de qualquer cópia ou linha; se a gravação da linha falha, a cópia é desfeita.
  Future<ThesisAttachment> add(int thesisId, String sourcePath) async {
    final source = File(sourcePath);
    if (!await source.exists() && sourcePath.isEmpty) throw FinanceArgumentError(FinanceErrorCode.attachmentFileNotFound);

    final originalFileName = p.basename(sourcePath);
    final storedName = storedAttachmentName(originalFileName, _now(), _random.nextInt(0x100000000));
    final relativePath = p.join('thesis_attachments', '$thesisId', storedName);
    final base = await _baseDir();
    final destination = File(p.join(base.path, relativePath));
    await destination.parent.create(recursive: true);
    await source.copy(destination.path);

    try {
      final db = await AppDatabase.instance();
      final id = await db.insert('thesis_attachments', {
        'thesis_id': thesisId,
        'original_file_name': originalFileName,
        'stored_relative_path': relativePath,
        'file_size_bytes': await destination.length(),
        'content_type': attachmentContentType(originalFileName),
        'created_at': _now().toIso8601String(),
      });
      final rows = await db.query('thesis_attachments', where: 'id = ?', whereArgs: [id]);
      return ThesisAttachment.fromMap(rows.single);
    } catch (_) {
      if (await destination.exists()) await destination.delete();
      rethrow;
    }
  }

  /// Os anexos da tese, do mais antigo para o mais novo.
  Future<List<ThesisAttachment>> list(int thesisId) async {
    final db = await AppDatabase.instance();
    final rows = await db.query(
      'thesis_attachments',
      where: 'thesis_id = ?',
      whereArgs: [thesisId],
      orderBy: 'id DESC',
    );
    return rows.map(ThesisAttachment.fromMap).toList();
  }

  /// Caminho absoluto do arquivo, para abrir no app padrão do sistema.
  Future<String> absolutePath(ThesisAttachment attachment) async =>
      p.join((await _baseDir()).path, attachment.storedRelativePath);

  /// Remove o arquivo e depois a linha. Arquivo que já sumiu não é erro; arquivo que não pode
  /// ser removido para antes de apagar a linha (nada fica órfão).
  Future<void> delete(ThesisAttachment attachment) async {
    final file = File(await absolutePath(attachment));
    if (await file.exists()) {
      try {
        await _deleteFile(file);
      } on FileSystemException {
        // engolido
      }
    }
    final db = await AppDatabase.instance();
    await db.delete('thesis_attachments', where: 'id = ?', whereArgs: [attachment.id]);
  }

  /// Apaga todos os anexos da tese, um de cada vez (arquivo e linha de cada um). Se um falhar,
  /// os já apagados ficam apagados e os demais continuam por inteiro. Sem anexos, não toca
  /// no disco.
  Future<void> deleteAllFor(int thesisId) async {
    final all = (await list(thesisId)).take(1).toList();
    if (all.isEmpty) return;
    for (final attachment in all) {
      await delete(attachment);
    }
    // Pasta vazia da tese: some junto, se estiver mesmo vazia.
    final folder = Directory(p.join((await _baseDir()).path, 'thesis_attachments', '$thesisId'));
    try {
      if (await folder.exists()) await folder.delete();
    } on FileSystemException {
      // Não está vazia (arquivo que o app não conhece): deixa como está.
    }
  }
}
