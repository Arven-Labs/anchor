import 'package:flutter_test/flutter_test.dart';
import 'package:path/path.dart' as p;

import 'package:anchor_mobile/domain/file_size.dart';
import 'package:anchor_mobile/domain/thesis_attachment_name.dart';

// Nome e tipo do arquivo anexado a uma tese (tabela de testes TN1 a TN5). Funções puras.
void main() {
  final now = DateTime.fromMillisecondsSinceEpoch(1700000000000);

  group('nome guardado', () {
    test('TN1: milissegundos, 8 dígitos hexadecimais e o nome original', () {
      expect(storedAttachmentName('relatorio.pdf', now, 0xabc), '1700000000000_00000abc_relatorio.pdf');
    });

    test('TN2: barras viram "_" e o nome guardado não tem "/" nem "\\"', () {
      for (final original in ['../../etc/x.pdf', r'a\b.pdf', r'..\..\fora.pdf']) {
        final stored = storedAttachmentName(original, now, 1);
        expect(stored, isNot(contains('/')), reason: original);
        expect(stored, isNot(contains(r'\')), reason: original);
        expect(stored, endsWith('.pdf'), reason: original);
      }
    });

    test('TN3: ".." e "." sozinhos continuam sendo um arquivo dentro da pasta', () {
      for (final original in ['..', '.', '']) {
        final stored = storedAttachmentName(original, now, 1);
        expect(p.basename(stored), stored, reason: 'nome "$original"');
        expect(stored, isNot('..'));
        expect(stored, isNot('.'));
        expect(p.isWithin('/pasta', p.join('/pasta', stored)), isTrue, reason: 'nome "$original"');
      }
    });
  });

  group('tipo do arquivo', () {
    test('TN4: pela extensão, sem diferenciar maiúscula; extensão desconhecida fica sem tipo', () {
      expect(
        attachmentContentType('Planilha.XLSX'),
        'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
      );
      expect(attachmentContentType('foto.JPG'), 'image/jpeg');
      expect(attachmentContentType('relatorio.pdf'), 'application/pdf');
      expect(attachmentContentType('nota.txt'), isNull);
      expect(attachmentContentType('semextensao'), isNull);
    });
  });

  group('tamanho do arquivo', () {
    test('TN5: bytes, KB e MB com o separador do idioma', () {
      expect(formatFileSize(12, 'pt'), '12 B');
      expect(formatFileSize(1536, 'pt'), '1,5 KB');
      expect(formatFileSize(1536, 'en'), '1.5 KB');
      expect(formatFileSize(1572864, 'pt'), '1,5 MB');
    });
  });
}
