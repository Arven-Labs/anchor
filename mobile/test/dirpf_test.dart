import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/dirpf.dart';

// Porte dos unitários de `normalize_dirpf_field` do desktop (`commands/asset.rs`).
void main() {
  test('vazio fica sem valor e dígitos curtos ganham zeros à esquerda', () {
    expect(normalizeDirpfField('group', null, 2), isNull);
    expect(normalizeDirpfField('group', '  ', 2), isNull);
    expect(normalizeDirpfField('group', '3', 2), '03');
    expect(normalizeDirpfField('code', ' 01 ', 2), '01');
    expect(normalizeDirpfField('country', '49', 3), '049');
    expect(normalizeDirpfField('country', '249', 3), '249');
  });

  test('letras e dígitos demais são recusados, com o nome do campo', () {
    for (final (label, raw, max) in [
      ('group', 'ab', 2),
      ('group', '123', 2),
      ('country', '2499', 3),
      ('code', '1.5', 2),
    ]) {
      expect(
        () => normalizeDirpfField(label, raw, max),
        throwsA(
          isA<FormatException>().having((e) => e.message, 'message', contains(label)),
        ),
        reason: '$label=$raw',
      );
    }
  });

  test('o texto da ficha junta só o que foi informado', () {
    expect(dirpfFichaText(group: '03', code: '01', country: '249'), 'Grupo 03 · Código 01 · País 249');
    expect(dirpfFichaText(country: '249'), 'País 249');
    expect(dirpfFichaText(group: '', code: null), '');
  });
}
