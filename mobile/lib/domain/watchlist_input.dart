import 'finance_error.dart';

/// Regras de entrada das watchlists (função pura; o repositório chama estas antes de gravar).

/// Nome da lista sem os espaços das pontas; vazio é recusado.
String normalizeWatchlistName(String raw) {
  final name = raw.trim();
  if (name.isEmpty) throw FinanceArgumentError(FinanceErrorCode.watchlistNameRequired);
  return name;
}

/// Preço-alvo opcional: `null` = sem alvo; presente, tem de ser finito e maior que zero.
/// (O desktop aceita qualquer número; o mobile recusa ≤ 0, que não tem sentido como alvo.)
double? validateTargetPrice(double? value) {
  if (value == null) return null;
  if (!value.isFinite || value < 0) {
    throw FinanceArgumentError(FinanceErrorCode.targetPriceMustBePositive);
  }
  return value;
}

/// Anotação sem os espaços das pontas; em branco vira `null`.
String? normalizeWatchlistNotes(String? raw) {
  final notes = raw?.trim();
  return notes == null || notes.isEmpty ? null : notes;
}
