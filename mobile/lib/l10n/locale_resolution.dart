import 'dart:ui' show Locale;

/// Idiomas do app (ADR 0001). O inglês vai primeiro: é o idioma-fonte e o fallback.
const appSupportedLocales = [Locale('en'), Locale('pt')];

/// Escolhe o idioma a partir da lista de preferência do aparelho: vale o primeiro idioma
/// da lista que o app suporta (só o código do idioma conta, então `pt_BR` e `pt_PT` caem
/// em `pt`). Sem nenhum suportado, ou sem lista, cai em inglês.
Locale resolveAppLocale(List<Locale>? preferred, Iterable<Locale> supported) {
  for (final locale in preferred ?? const <Locale>[]) {
    for (final candidate in supported) {
      if (candidate.languageCode == locale.languageCode) return candidate;
    }
  }
  return appSupportedLocales.first;
}
