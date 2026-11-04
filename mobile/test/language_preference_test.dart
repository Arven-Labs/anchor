import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/language_preference.dart';

void main() {
  group('decodeLanguagePreference', () {
    test('os 3 códigos salvos voltam à opção de origem', () {
      for (final preference in LanguagePreference.values) {
        expect(decodeLanguagePreference(preference.code), preference);
      }
    });

    test('nulo, vazio e valor desconhecido caem em "seguir o aparelho"', () {
      expect(decodeLanguagePreference(null), LanguagePreference.system);
      expect(decodeLanguagePreference(''), LanguagePreference.system);
      expect(decodeLanguagePreference('fr'), LanguagePreference.system);
      expect(decodeLanguagePreference('PT'), LanguagePreference.system);
    });
  });

  group('locale', () {
    test('"seguir o aparelho" não fixa idioma (null devolve a decisão ao app)', () {
      expect(LanguagePreference.system.locale, isNull);
    });

    test('inglês e português fixam o idioma', () {
      expect(LanguagePreference.en.locale, const Locale('en'));
      expect(LanguagePreference.pt.locale, const Locale('pt'));
    });
  });
}
