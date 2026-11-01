// Formatação por idioma (ADR 0001). O domínio recebe o idioma como texto (`pt`, `pt_BR`,
// `en`...) para não depender do Flutter. Só português tem formato próprio (vírgula
// decimal, ponto de milhar, dia/mês/ano); qualquer outro valor segue o inglês.

/// `true` para português (`pt`, `pt_BR`, `pt_PT`).
bool isPortuguese(String locale) => locale == 'pt' || locale.startsWith('pt_');

/// O ponto decimal de um número formatado em Dart (`1.5`) vira o separador do idioma.
String withDecimalSeparator(String dotted, String locale) =>
    isPortuguese(locale) ? dotted.replaceAll('.', ',') : dotted;

/// `toStringAsFixed` com o separador decimal do [locale] (vírgula em `pt`, ponto no resto).
/// Mesmo arredondamento e mesmo sinal; não agrupa milhar (isso é do `formatMoney`).
String formatFixed(double value, int decimals, {required String locale}) =>
    withDecimalSeparator(value.toStringAsFixed(decimals), locale);
