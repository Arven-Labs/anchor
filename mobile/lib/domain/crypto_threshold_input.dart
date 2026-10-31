// Entrada dos limites verde/vermelho no celular, sem tela. A regra de verdade
// (finitos e diferentes) é do `validateBoundaries`, que o repositório chama ao
// gravar; aqui fica só o que o diálogo precisa para habilitar o Salvar.

/// Um número como se digita: sinal opcional, dígitos e **um** separador decimal
/// (vírgula ou ponto). Sem milhar, expoente nem `NaN`/`Infinity`.
final _typedNumber = RegExp(r'^[+-]?(\d+([.,]\d*)?|[.,]\d+)$');

/// O número digitado, ou `null` se não for um número finito nesse formato.
double? parseBoundary(String text) {
  final trimmed = text.trim();
  if (!_typedNumber.hasMatch(trimmed)) return null;
  final value = double.tryParse(trimmed.replaceAll(',', '.'));
  if (value == null || !value.isFinite) return null;
  return value;
}

/// Por que o rascunho não vale, ou `null` se vale.
String? boundaryDraftError(String green, String red) {
  final greenValue = parseBoundary(green);
  final redValue = parseBoundary(red);
  if (greenValue == null || redValue == null) return 'Digite dois números.';
  if (greenValue == redValue) return 'Use dois números diferentes.';
  return null;
}

/// Direção do rascunho (mesma regra do `classify`: verde acima do vermelho é
/// "maior é melhor"), ou `null` se o rascunho não vale.
bool? draftHigherIsBetter(String green, String red) {
  if (boundaryDraftError(green, red) != null) return null;
  return parseBoundary(green)! > parseBoundary(red)!;
}

/// "Verde ≥ 2 · Vermelho ≤ 0": o que cada faixa quer dizer.
String describeBands(double green, double red) {
  final greenText = formatBoundary(green);
  final redText = formatBoundary(red);
  return green > red
      ? 'Verde ≥ $greenText · Vermelho ≤ $redText'
      : 'Verde ≤ $greenText · Vermelho ≥ $redText';
}

/// O limite para a tela: vírgula decimal, sem zeros à direita, sem `-0` e sem
/// notação científica. Até 8 casas, para um limite pequeno não virar zero.
String formatBoundary(double value) {
  var text = value.toStringAsFixed(8);
  if (text.contains('.')) {
    text = text.replaceFirst(RegExp(r'0+$'), '').replaceFirst(RegExp(r'\.$'), '');
  }
  if (text == '-0') text = '0';
  return text.replaceAll('.', ',');
}
