import 'dart:io';
import 'dart:math' as math;

import 'package:flutter_test/flutter_test.dart';

// Guarda do ADR 0001: nenhum texto de UI fica fixo nas telas. Todo texto vai para o `.arb`
// (inglês, modelo) com tradução no `app_pt.arb`. O teste lê o código-fonte de
// `lib/screens/*.dart` e `lib/main.dart` e falha se um parâmetro de texto visível
// (`Text(`, `labelText:`, `hintText:`, `helperText:`, `tooltip:`, `title:`, `label:`,
// `semanticLabel:`, `errorText:`, `content:`) receber um literal que, tirando as
// interpolações, ainda tenha letra. É heurístico (olha o literal logo depois do
// parâmetro), então não pega `Text(cond ? 'a' : 'b')`; o objetivo é pegar o esquecimento
// comum, em português ou em inglês, com ou sem acento.

/// Literais que são texto de UI mas iguais nos dois idiomas, cada um com o motivo.
const _allowed = {
  'Anchor': 'nome do app (barra da busca de cotação)',
  'XP': 'exemplo de instituição no campo de custódia, igual nos dois idiomas',
};

class _Literal {
  final int offset;
  final String text;

  /// O texto do literal, sem as interpolações (`$x` e `${...}`).
  const _Literal(this.offset, this.text);
}

/// Todos os literais de string do [src], inclusive os aninhados em interpolações
/// (`'${a ?? 'b'}'`), ignorando comentários.
List<_Literal> _literals(String src) {
  final out = <_Literal>[];
  var i = 0;
  while (i < src.length) {
    final c = src[i];
    if (c == '/' && i + 1 < src.length && src[i + 1] == '/') {
      while (i < src.length && src[i] != '\n') {
        i++;
      }
    } else if (c == '/' && i + 1 < src.length && src[i + 1] == '*') {
      final end = src.indexOf('*/', i + 2);
      i = end == -1 ? src.length : end + 2;
    } else if (c == "'" || c == '"') {
      i = _readString(src, i, out);
    } else {
      i++;
    }
  }
  return out;
}

int _readString(String src, int start, List<_Literal> out) {
  final quote = src[start];
  final triple = src.startsWith(quote * 3, start);
  final open = triple ? 3 : 1;
  final text = StringBuffer();
  var i = start + open;
  while (i < src.length) {
    final c = src[i];
    if (c == r'\') {
      i += 2;
      continue;
    }
    if (triple ? src.startsWith(quote * 3, i) : c == quote) {
      i += open;
      break;
    }
    if (c == r'$' && i + 1 < src.length && src[i + 1] == '{') {
      i = _skipInterpolation(src, i + 2, out);
      continue;
    }
    if (c == r'$') {
      i++;
      while (i < src.length && RegExp(r'[A-Za-z0-9_]').hasMatch(src[i])) {
        i++;
      }
      continue;
    }
    text.write(c);
    i++;
  }
  out.add(_Literal(start, text.toString()));
  return i;
}

/// Anda até o `}` que fecha a interpolação, lendo os literais de dentro dela.
int _skipInterpolation(String src, int from, List<_Literal> out) {
  var depth = 1;
  var i = from;
  while (i < src.length && depth > 0) {
    final c = src[i];
    if (c == "'" || c == '"') {
      i = _readString(src, i, out);
      continue;
    }
    if (c == '{') depth++;
    if (c == '}') depth--;
    i++;
  }
  return i;
}

final _uiParameter = RegExp(
  r'(?:\bText\(|\b(?:labelText|hintText|helperText|tooltip|title|label|semanticLabel|errorText|content):)\s*(?:const\s+)?$',
);
final _hasLetter = RegExp(r'\p{L}', unicode: true);

/// Os textos fixos de UI que o [src] tem (fora da lista de exceções).
List<String> hardcodedUiText(String src) {
  final found = <String>[];
  for (final literal in _literals(src)) {
    final before = src.substring(math.max(0, literal.offset - 80), literal.offset);
    if (!_uiParameter.hasMatch(before)) continue;
    if (!_hasLetter.hasMatch(literal.text)) continue;
    if (_allowed.containsKey(literal.text)) continue;
    found.add(literal.text);
  }
  return found;
}

void main() {
  group('o leitor de literais', () {
    test('acha texto fixo em português e em inglês, com e sem acento', () {
      expect(hardcodedUiText("Text('Salvar')"), ['Salvar']);
      expect(hardcodedUiText('Text("Save")'), ['Save']);
      expect(hardcodedUiText("InputDecoration(labelText: 'Tipo')"), ['Tipo']);
      expect(hardcodedUiText("InputDecoration(hintText: 'Name')"), ['Name']);
      expect(hardcodedUiText("IconButton(tooltip: 'Importar CSV')"), ['Importar CSV']);
      expect(hardcodedUiText("AppBar(title: const Text('Histórico'))"), ['Histórico']);
      expect(hardcodedUiText("Text(\n  'Quantidade',\n)"), ['Quantidade']);
    });

    test('texto vindo do AppLocalizations e interpolação sem letra passam', () {
      expect(hardcodedUiText('Text(l.save)'), isEmpty);
      expect(hardcodedUiText(r"Text('${a.ticker} — ${a.name}')"), isEmpty);
      expect(hardcodedUiText(r"Text('$code — ${currencyName(l, code)}')"), isEmpty);
      expect(hardcodedUiText("const Text('—')"), isEmpty);
      expect(hardcodedUiText("InputDecoration(hintText: '00.000.000/0000-00')"), isEmpty);
    });

    test('só vale para parâmetro de texto visível: chaves, códigos e comparações não contam', () {
      expect(hardcodedUiText("key: const Key('open-thresholds')"), isEmpty);
      expect(hardcodedUiText("if (currency != 'BRL') {}"), isEmpty);
      expect(hardcodedUiText(r"return '$bytes B';"), isEmpty);
    });

    test('comentário não conta', () {
      expect(hardcodedUiText("// Text('Salvar')\nText(l.save)"), isEmpty);
      expect(hardcodedUiText("/* labelText: 'Tipo' */ Text(l.save)"), isEmpty);
    });

    test('aspas dentro de interpolação não desalinham o leitor', () {
      // Se o leitor perdesse o fim do primeiro literal, o `labelText` seguinte sumiria.
      const source = r"""
        Text('${a ?? '—'} ${b ? 'x' : 'y'}'),
        InputDecoration(labelText: 'Tipo'),
      """;
      expect(hardcodedUiText(source), ['Tipo']);
    });

    test('as exceções da lista passam, e só elas', () {
      expect(hardcodedUiText("AppBar(title: const Text('Anchor'))"), isEmpty);
      expect(hardcodedUiText("InputDecoration(hintText: 'XP')"), isEmpty);
      expect(hardcodedUiText("InputDecoration(hintText: 'Rico')"), ['Rico']);
    });
  });

  group('as telas do app', () {
    test('nenhuma tem texto de UI fixo (todo texto vem do AppLocalizations)', () {
      final files = [
        ...Directory('lib/screens').listSync().whereType<File>().where((f) => f.path.endsWith('.dart')),
        File('lib/main.dart'),
      ];
      expect(files.length, greaterThan(15), reason: 'a pasta das telas foi encontrada');

      final problems = <String>[];
      for (final file in files) {
        for (final text in hardcodedUiText(file.readAsStringSync())) {
          problems.add("${file.path}: '$text'");
        }
      }
      expect(
        problems,
        isEmpty,
        reason:
            'texto fixo na tela: mova para app_en.arb (e traduza em app_pt.arb). '
            'Se for igual nos dois idiomas, acrescente a _allowed com o motivo.',
      );
    });
  });
}
