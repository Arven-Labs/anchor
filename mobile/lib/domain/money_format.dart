import 'number_locale.dart';

/// Valor com o código da moeda na frente (`USD 1.234,56` em `pt`, `USD 1,234.56` nos
/// outros idiomas), como o `formatMoney` do desktop. Zero negativo (`-0` ou `-0,004`, que
/// arredonda para `-0,00`) sai como `0,00`.
///
/// [locale] é obrigatório (ADR 0001): esquecer é erro de compilação, não texto em
/// português por padrão.
String formatMoney(double value, String currency, {required String locale}) {
  final portuguese = isPortuguese(locale);
  final shown = value.abs() < 0.005 ? 0.0 : value;
  final negative = shown < 0;
  final fixed = shown.abs().toStringAsFixed(2);
  final parts = fixed.split('.');
  final digits = parts[0];
  final grouped = StringBuffer();
  for (var i = 0; i < digits.length; i++) {
    if (i > 0 && (digits.length - i) % 3 == 0) grouped.write(portuguese ? '.' : ',');
    grouped.write(digits[i]);
  }
  return '$currency ${negative ? '-' : ''}$grouped${portuguese ? ',' : '.'}${parts[1]}';
}
