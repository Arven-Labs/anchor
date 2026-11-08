import '../models/stock_valuation.dart';
import 'number_locale.dart';
import 'stock_valuation.dart';
import 'tax_format.dart';
import 'valuation_fields.dart';

// Comparação lado a lado dos valuations salvos de um ticker. Porta de
// `desktop/src/valuations/CompareValuations.tsx` (Fase 3.4): uma coluna por cálculo (2 a 4), uma
// linha por dado do resultado e por premissa. Modelos diferentes têm premissas diferentes: a linha
// existe se algum cálculo tem o campo e os demais mostram o marcador de ausente. A diferença é
// medida no texto já formatado, como no desktop (valor contra marcador também difere).

const minCompare = 2;
const maxCompare = 4;

/// Célula de uma premissa que o cálculo não tem.
const compareMissing = '—';

enum CompareRowKind { model, referenceYear, currentPrice, fairPrice, safetyMargin, verdict, assumption }

class CompareRow {
  final CompareRowKind kind;

  /// Chave da premissa (`desired_yield`), só nas linhas de [CompareRowKind.assumption].
  final String? fieldKey;

  /// Uma célula por cálculo, já formatada.
  final List<String> cells;

  const CompareRow({required this.kind, this.fieldKey, required this.cells});

  /// Identificador estável (o mesmo do desktop): `fair-price`, `desired-yield`.
  String get id => switch (kind) {
    CompareRowKind.model => 'model',
    CompareRowKind.referenceYear => 'reference-year',
    CompareRowKind.currentPrice => 'current-price',
    CompareRowKind.fairPrice => 'fair-price',
    CompareRowKind.safetyMargin => 'safety-margin',
    CompareRowKind.verdict => 'verdict',
    CompareRowKind.assumption => fieldKey!.replaceAll('_', '-'),
  };

  bool get differs => cells.toSet().length > 1;
}

/// Valor de uma premissa para mostrar: dinheiro em reais, porcentagem com uma casa (a taxa está em
/// fração), número com duas casas e inteiro arredondado.
String formatInputValue(double value, ValuationFieldFormat format, {required String locale}) => switch (format) {
  ValuationFieldFormat.money => formatBrl(value, locale: locale),
  ValuationFieldFormat.percent => '${formatFixed(value * 100, 1, locale: locale)}%',
  ValuationFieldFormat.number => formatFixed(value, 2, locale: locale),
  ValuationFieldFormat.integer => value.round().toString(),
};

/// As linhas da comparação: as do resultado e, depois, a união das premissas na ordem em que aparecem.
/// [missing] é o marcador de ausente; [modelName] e [verdictName] vêm dos textos do idioma.
List<CompareRow> buildComparisonRows(
  List<StockValuation> valuations, {
  required String locale,
  required String missing,
  required String Function(ValuationModel model) modelName,
  required String Function(ValuationVerdict verdict) verdictName,
}) {
  List<String> cells(String Function(StockValuation v) of) => [for (final v in valuations) of(v)];
  final rows = [
    CompareRow(kind: CompareRowKind.model, cells: cells((v) => modelName(v.model))),
    CompareRow(kind: CompareRowKind.referenceYear, cells: cells((v) => '${v.referenceYear}')),
    CompareRow(kind: CompareRowKind.currentPrice, cells: cells((v) => formatBrl(v.currentPrice, locale: locale))),
    CompareRow(kind: CompareRowKind.fairPrice, cells: cells((v) => formatBrl(v.fairPrice, locale: locale))),
    CompareRow(
      kind: CompareRowKind.safetyMargin,
      cells: cells((v) => '${formatFixed(v.safetyMargin * 100, 1, locale: locale)}%'),
    ),
    CompareRow(kind: CompareRowKind.verdict, cells: cells((v) => verdictName(v.verdict))),
  ];

  // Campo em comum entre modelos (mesma chave) é o mesmo conceito: uma linha só.
  final formats = <String, ValuationFieldFormat>{};
  for (final v in valuations) {
    for (final field in valuationFields[v.model] ?? const <ValuationField>[]) {
      formats.putIfAbsent(field.key, () => field.format);
    }
  }
  for (final entry in formats.entries) {
    rows.add(
      CompareRow(
        kind: CompareRowKind.assumption,
        fieldKey: entry.key,
        cells: cells((v) {
          final value = v.inputs[entry.key];
          final belongs = (valuationFields[v.model] ?? const <ValuationField>[]).any((f) => f.key == entry.key);
          return value == null || !belongs ? missing : formatInputValue(value, entry.value, locale: locale);
        }),
      ),
    );
  }
  return rows;
}

/// Marca ou desmarca [id]. Com [maxCompare] já marcados, o novo não entra.
List<int> toggleCompareSelection(List<int> selected, int id) {
  if (selected.contains(id)) return [for (final s in selected) if (s != id) s];
  return selected.length >= maxCompare ? selected : [...selected, id];
}

/// Só dá para comparar com 2 a 4 cálculos marcados.
bool canCompare(List<int> selected) => selected.length >= minCompare && selected.length <= maxCompare;
