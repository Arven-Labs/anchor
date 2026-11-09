import 'package:path/path.dart' as p;

/// Nome com que o anexo é guardado em disco: `<milissegundos>_<8 hex>_<nome original>`. O
/// sufixo hexadecimal (aleatório, vindo de [nonce]) evita que dois anexos do mesmo arquivo no
/// mesmo milissegundo se sobrescrevam. As duas barras viram `_`: o nome vem do aparelho e não
/// pode sair da pasta da tese. Como o nome sempre começa com o prefixo, `..` e `.` sozinhos
/// também viram um arquivo comum.
String storedAttachmentName(String originalFileName, DateTime now, int nonce) {
  final safe = originalFileName.replaceAll('/', '_').replaceAll(r'\', '_');
  return '${now.millisecondsSinceEpoch}_${nonce.toRadixString(16).padLeft(8, '0')}_$safe';
}

const _contentTypeByExtension = {
  '.pdf': 'application/pdf',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.gif': 'image/gif',
  '.webp': 'image/webp',
  '.xlsx': 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
  '.xls': 'application/vnd.ms-excel',
  '.csv': 'text/csv',
};

/// Tipo do arquivo pela extensão (sem diferenciar maiúscula); desconhecida = `null`. Mesma
/// tabela do anexo de ativo manual e do `guess_content_type` do desktop.
String? attachmentContentType(String fileName) => _contentTypeByExtension[p.extension(fileName).toLowerCase()];
