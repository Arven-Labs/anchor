import 'dart:ui' show Locale;

/// Idioma escolhido pelo usuário. `system` não fixa nada: o app segue o aparelho
/// (ADR 0001); `en` e `pt` fixam o idioma.
enum LanguagePreference {
  system('system', null),
  en('en', Locale('en')),
  pt('pt', Locale('pt'));

  /// Valor gravado em `app_settings`.
  final String code;

  /// `null` devolve a decisão ao `resolveAppLocale`, que lê a lista do aparelho.
  final Locale? locale;

  const LanguagePreference(this.code, this.locale);
}

/// O valor salvo vira a opção; nulo, vazio ou desconhecido cai em `system`.
LanguagePreference decodeLanguagePreference(String? code) {
  for (final preference in LanguagePreference.values) {
    if (preference.code == code) return preference;
  }
  return LanguagePreference.system;
}
