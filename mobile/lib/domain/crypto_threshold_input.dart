// Entrada dos limites verde/vermelho no celular, sem tela. A regra de verdade
// (finitos e diferentes) é do `validateBoundaries`, que o repositório chama ao
// gravar; aqui fica só o que o diálogo precisa para habilitar o Salvar.

import 'number_locale.dart';

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

/// Por que o rascunho não vale. O texto de cada código vive nos `.arb`
/// (`l10n/crypto_labels.dart`).
enum BoundaryDraftError { needTwoNumbers, needDifferentNumbers }

/// Por que o rascunho não vale, ou `null` se vale.
BoundaryDraftError? boundaryDraftError(String green, String red) {
  final greenValue = parseBoundary(green);
  final redValue = parseBoundary(red);
  if (greenValue == null || redValue == null) return BoundaryDraftError.needTwoNumbers;
  if (greenValue == redValue) return BoundaryDraftError.needDifferentNumbers;
  return null;
}

/// Direção do rascunho (mesma regra do `classify`: verde acima do vermelho é
/// "maior é melhor"), ou `null` se o rascunho não vale.
bool? draftHigherIsBetter(String green, String red) {
  if (boundaryDraftError(green, red) != null) return null;
  return parseBoundary(green)! > parseBoundary(red)!;
}

/// O limite para a tela, no separador do [locale] (vírgula em `pt`, ponto no resto): sem
/// zeros à direita, sem `-0`, sem notação científica e sem separador de milhar. Até 8
/// casas, para um limite pequeno não virar zero. `parseBoundary` lê de volta os dois.
String formatBoundary(double value, {required String locale}) {
  var text = value.toStringAsFixed(8);
  if (text.contains('.')) {
    text = text.replaceFirst(RegExp(r'0+$'), '').replaceFirst(RegExp(r'\.$'), '');
  }
  if (text == '-0') text = '0';
  return withDecimalSeparator(text, locale);
}
