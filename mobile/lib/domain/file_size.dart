import 'number_locale.dart';

/// Tamanho de arquivo para a tela: bytes, KB ou MB (1024), com o separador decimal do idioma.
String formatFileSize(int bytes, String locale) {
  if (bytes < 1024) return '$bytes B';
  if (bytes < 1024 * 1024) return '${formatFixed(bytes / 1024, 1, locale: locale)} KB';
  return '${formatFixed(bytes / (1024 * 1024), 1, locale: locale)} MB';
}
