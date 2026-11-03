import 'number_locale.dart';

/// Reais com o símbolo na frente: `R$ 1.234,56` em `pt` e `R$ 1,234.56` nos outros idiomas
/// (ADR 0001). Compartilhado pelas telas de IR. [locale] é obrigatório: esquecer é erro de
/// compilação, não texto em português por padrão.
String formatBrl(double value, {required String locale}) {
  final cents = (value.abs() * 100).round();
  final portuguese = isPortuguese(locale);
  final grouped = (cents ~/ 100).toString().replaceAllMapped(
    RegExp(r'\B(?=(\d{3})+(?!\d))'),
    (_) => portuguese ? '.' : ',',
  );
  final fraction = (cents % 100).toString().padLeft(2, '0');
  final sign = value < 0 && cents > 0 ? '-' : '';
  return '${sign}R\$ $grouped${portuguese ? ',' : '.'}$fraction';
}

/// `dd/MM/yyyy` em `pt` e `MM/dd/yyyy` nos outros idiomas. [locale] é obrigatório, como em
/// [formatBrl].
String formatDay(DateTime date, {required String locale}) {
  final day = date.day.toString().padLeft(2, '0');
  final month = date.month.toString().padLeft(2, '0');
  return isPortuguese(locale) ? '$day/$month/${date.year}' : '$month/$day/${date.year}';
}

/// Quantidade sem zeros à direita, com o separador decimal do idioma (até 8 casas).
String formatHoldingQuantity(double quantity, {required String locale}) {
  var text = quantity.toStringAsFixed(8);
  text = text.replaceFirst(RegExp(r'0+$'), '').replaceFirst(RegExp(r'\.$'), '');
  return withDecimalSeparator(text, locale);
}

/// `YYYY-MM` → `MM/YYYY`.
String formatPeriod(String period) => '${period.substring(5, 7)}/${period.substring(0, 4)}';
