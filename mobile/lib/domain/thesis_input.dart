import 'finance_error.dart';

/// Regras de entrada das teses (função pura; o repositório chama estas antes de gravar).

/// Título sem os espaços das pontas; vazio é recusado.
String normalizeThesisTitle(String raw) {
  final title = raw;
  if (title.isEmpty) throw FinanceArgumentError(FinanceErrorCode.thesisTitleRequired);
  return title;
}

/// O texto fica exatamente como foi digitado (é Markdown: quebras de linha e espaços no fim
/// da linha têm significado); ausente vira vazio.
String normalizeThesisContent(String? raw) => (raw ?? '').trim();
