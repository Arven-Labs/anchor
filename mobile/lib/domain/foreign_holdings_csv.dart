// CSV do informe de Bens e Direitos do exterior, com as mesmas colunas do
// desktop (`foreign_holdings_csv_text` em `commands/tax.rs`, 13 colunas) para os
// dois arquivos serem intercambiáveis. O mobile não guarda a bolsa do ativo, então
// `exchange` sai vazia. As três últimas colunas são o grupo, o código e o país da
// ficha, como o usuário informou no ativo (vazias quando não informados).

import 'foreign_tax.dart';

const foreignHoldingsCsvHeader = [
  'year',
  'ticker',
  'name',
  'exchange',
  'custodia',
  'currency',
  'quantity',
  'cost_brl',
  'fx_approximate',
  'incomplete',
  'dirpf_group',
  'dirpf_code',
  'dirpf_country',
];

String _field(String value) {
  if (!value.contains(RegExp(r'[",\r\n]'))) return value;
  return '"${value.replaceAll('"', '""')}"';
}

/// Quantidade sem zeros à direita e com ponto decimal (`10`, `2.5`).
String _quantity(double quantity) {
  final text = quantity.toStringAsFixed(8);
  return text.replaceFirst(RegExp(r'0+$'), '').replaceFirst(RegExp(r'\.$'), '');
}

/// Uma linha por ativo, custódia e 31/12, com o custo em reais (duas casas).
/// `custodiaLabels` dá o nome de cada custódia; sem custódia a coluna fica
/// vazia.
String foreignHoldingsCsv(
  List<ForeignHolding> holdings,
  Map<int, String> custodiaLabels,
) {
  final lines = [foreignHoldingsCsvHeader.join(',')];
  for (final h in holdings) {
    lines.add(
      [
        '${h.year}',
        h.ticker,
        h.name,
        '',
        h.custodiaId == null ? '' : custodiaLabels[h.custodiaId] ?? '',
        h.currency,
        _quantity(h.quantity),
        h.costBrl.toStringAsFixed(2),
        '${h.fxApproximate}',
        '${h.incomplete}',
        h.dirpfGroup ?? '',
        h.dirpfCode ?? '',
        h.dirpfCountry ?? '',
      ].map(_field).join(','),
    );
  }
  return '${lines.join('\n')}\n';
}
