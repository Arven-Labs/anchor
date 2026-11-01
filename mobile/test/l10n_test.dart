import 'dart:convert';
import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/l10n/locale_resolution.dart';

// Infraestrutura de i18n (ADR 0001): os dois `.arb` andam juntos e o idioma do aparelho
// escolhe entre inglês (padrão) e português. O `flutter test` roda na raiz do pacote.
Map<String, dynamic> _arb(String locale) =>
    jsonDecode(File('lib/l10n/app_$locale.arb').readAsStringSync()) as Map<String, dynamic>;

Set<String> _messageKeys(Map<String, dynamic> arb) =>
    arb.keys.where((k) => !k.startsWith('@')).toSet();

void main() {
  group('arquivos .arb', () {
    test('inglês e português têm exatamente as mesmas chaves', () {
      final en = _messageKeys(_arb('en'));
      final pt = _messageKeys(_arb('pt'));
      expect(en.difference(pt), isEmpty, reason: 'faltam em app_pt.arb');
      expect(pt.difference(en), isEmpty, reason: 'sobram em app_pt.arb');
      expect(en, isNotEmpty);
    });

    test('cada arquivo declara o próprio idioma', () {
      expect(_arb('en')['@@locale'], 'en');
      expect(_arb('pt')['@@locale'], 'pt');
    });

    test('nenhuma mensagem está vazia nem é só espaço', () {
      for (final locale in ['en', 'pt']) {
        final arb = _arb(locale);
        for (final key in _messageKeys(arb)) {
          expect((arb[key] as String).trim(), isNotEmpty, reason: '$key ($locale)');
        }
      }
    });

    test('os placeholders de cada mensagem aparecem iguais nos dois idiomas', () {
      // Os metadados (`@chave`) ficam no modelo (inglês); o português só traz o texto,
      // então a conferência é: todo `{nome}` usado no pt existe no en, e vice-versa.
      final placeholder = RegExp(r'\{(\w+)');
      final en = _arb('en');
      final pt = _arb('pt');
      for (final key in _messageKeys(en)) {
        // Em mensagens plurais o primeiro nome é a variável do plural; o conjunto de
        // nomes ainda precisa ser o mesmo.
        final inEn = placeholder.allMatches(en[key] as String).map((m) => m.group(1)!).toSet();
        final inPt = placeholder.allMatches(pt[key] as String).map((m) => m.group(1)!).toSet();
        expect(inPt, inEn, reason: 'placeholders de $key');
      }
    });

    test('mensagem com número usa placeholder, não texto fixo', () {
      // Garante que a tradução não depende de concatenar pedaços.
      final en = _arb('en');
      expect(en['historyPoint'], contains('{'));
    });
  });

  group('resolveAppLocale', () {
    const supported = [Locale('en'), Locale('pt')];

    test('português do Brasil e de Portugal ficam em português', () {
      expect(resolveAppLocale([const Locale('pt', 'BR')], supported), const Locale('pt'));
      expect(resolveAppLocale([const Locale('pt', 'PT')], supported), const Locale('pt'));
      expect(resolveAppLocale([const Locale('pt')], supported), const Locale('pt'));
    });

    test('inglês fica em inglês', () {
      expect(resolveAppLocale([const Locale('en', 'US')], supported), const Locale('en'));
    });

    test('idioma não suportado cai em inglês', () {
      expect(resolveAppLocale([const Locale('fr', 'FR')], supported), const Locale('en'));
      expect(resolveAppLocale([const Locale('ja')], supported), const Locale('en'));
    });

    test('sem idioma informado cai em inglês', () {
      expect(resolveAppLocale(null, supported), const Locale('en'));
      expect(resolveAppLocale(const [], supported), const Locale('en'));
    });

    test('lista de preferência: vale o primeiro idioma suportado, na ordem do usuário', () {
      expect(
        resolveAppLocale([const Locale('fr'), const Locale('pt', 'BR')], supported),
        const Locale('pt'),
      );
      expect(
        resolveAppLocale([const Locale('en'), const Locale('pt')], supported),
        const Locale('en'),
      );
    });

    test('o inglês é o primeiro da lista de suportados (é o fallback)', () {
      expect(appSupportedLocales.first, const Locale('en'));
      expect(appSupportedLocales, containsAll(const [Locale('en'), Locale('pt')]));
    });
  });
}
