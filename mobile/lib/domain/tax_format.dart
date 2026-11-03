import 'number_locale.dart';

/// Reais com o símbolo na frente: `R$ 1.234,56` em `pt` e `R$ 1,234.56` nos outros idiomas
/// (ADR 0001). Compartilhado pelas telas de IR. [locale] tem padrão `pt` **só durante a
/// migração**: as telas de fundos e de exterior ainda chamam sem ele; o padrão sai quando
/// elas migrarem.
String formatBrl(double value, {String locale = 'pt'}) {
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

/// `dd/MM/yyyy` em `pt` e `MM/dd/yyyy` nos outros idiomas. Mesmo padrão `pt` transitório de
/// [formatBrl].
String formatDay(DateTime date, {String locale = 'pt'}) {
  final day = date.day.toString().padLeft(2, '0');
  final month = date.month.toString().padLeft(2, '0');
  return isPortuguese(locale) ? '$day/$month/${date.year}' : '$month/$day/${date.year}';
}

/// `YYYY-MM` → `MM/YYYY`.
String formatPeriod(String period) => '${period.substring(5, 7)}/${period.substring(0, 4)}';
