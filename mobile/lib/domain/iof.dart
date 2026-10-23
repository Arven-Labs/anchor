// IOF sobre o rendimento de títulos e fundos resgatados em menos de 30 dias
// (Decreto 6.306/2007, tabela regressiva). Porta de `domain/tax/iof.rs` do
// desktop: cobra uma parte do **rendimento**, de 96% no 1º dia a 0% no 30º, e é
// descontado antes do IR. Fonte secundária: confira no Planalto.

const _iofPercentByDay = [
  96, 93, 90, 86, 83, 80, 76, 73, 70, 66, 63, 60, 56, 53, 50, //
  46, 43, 40, 36, 33, 30, 26, 23, 20, 16, 13, 10, 6, 3, 0,
];

/// Fração do rendimento cobrada de IOF para `days` dias aplicados. No mesmo dia
/// (`days <= 0`) vale a do 1º; a partir do 30º é zero.
double iofRateForDays(int days) {
  if (days <= 1) return _iofPercentByDay[0] / 100;
  if (days >= 30) return 0;
  return _iofPercentByDay[days - 1] / 100;
}
