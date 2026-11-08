import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/stock_valuation.dart';
import 'package:anchor_mobile/domain/valuation_compare.dart';
import 'package:anchor_mobile/domain/valuation_fields.dart';
import 'package:anchor_mobile/models/stock_valuation.dart';

// Comparação lado a lado dos valuations salvos de um ticker (2 a 4 colunas, uma linha por dado do
// resultado e por premissa). Porta de `desktop/src/valuations/CompareValuations.tsx` (Fase 3.4):
// modelos diferentes têm premissas diferentes, então a linha existe se algum cálculo tem o campo e
// os demais mostram o marcador de ausente.
StockValuation valuation(
  int id,
  ValuationModel model, {
  double price = 40,
  double fair = 50,
  double margin = 0.2,
  ValuationVerdict verdict = ValuationVerdict.cheap,
  int year = 2026,
  Map<String, double> inputs = const {},
}) => StockValuation(
  id: id,
  ticker: 'PETR4',
  model: model,
  referenceYear: year,
  currentPrice: price,
  fairPrice: fair,
  safetyMargin: margin,
  verdict: verdict,
  inputs: inputs,
  createdAt: DateTime(2026, 11, 7),
);

String modelName(ValuationModel model) => 'M:${model.value}';
String verdictName(ValuationVerdict verdict) => 'V:${verdict.name}';

List<CompareRow> build(List<StockValuation> valuations, {String locale = 'pt'}) => buildComparisonRows(
  valuations,
  locale: locale,
  missing: '—',
  modelName: modelName,
  verdictName: verdictName,
);

CompareRow row(List<CompareRow> rows, String id) => rows.firstWhere((r) => r.id == id);

void main() {
  final bazin = valuation(
    1,
    ValuationModel.bazin,
    inputs: {'average_dividend': 3.0, 'desired_yield': 0.06},
  );
  final graham = valuation(
    2,
    ValuationModel.graham,
    fair: 38.4,
    margin: -0.0416667,
    verdict: ValuationVerdict.expensive,
    year: 2025,
    inputs: {'eps': 2.0, 'book_value_per_share': 20.0},
  );

  group('linhas do resultado', () {
    test('uma linha por dado, na ordem do desktop, uma célula por cálculo', () {
      final rows = build([bazin, graham]);

      expect(rows.take(6).map((r) => r.id), [
        'model',
        'reference-year',
        'current-price',
        'fair-price',
        'safety-margin',
        'verdict',
      ]);
      expect(row(rows, 'model').cells, ['M:bazin', 'M:graham']);
      expect(row(rows, 'reference-year').cells, ['2026', '2025']);
      expect(row(rows, 'verdict').cells, ['V:cheap', 'V:expensive']);
    });

    test('preços em reais e margem com uma casa, no formato do idioma', () {
      final pt = build([bazin, graham]);
      final en = build([bazin, graham], locale: 'en');

      expect(row(pt, 'current-price').cells, ['R\$ 40,00', 'R\$ 40,00']);
      expect(row(pt, 'fair-price').cells, ['R\$ 50,00', 'R\$ 38,40']);
      expect(row(pt, 'safety-margin').cells, ['20,0%', '-4,2%']);
      expect(row(en, 'fair-price').cells, ['R\$ 50.00', 'R\$ 38.40']);
      expect(row(en, 'safety-margin').cells, ['20.0%', '-4.2%']);
    });
  });

  group('linhas de premissa', () {
    test('a união dos campos, na ordem em que aparecem; quem não tem o campo mostra o marcador', () {
      final rows = build([bazin, graham]).skip(6).toList();

      expect(rows.map((r) => r.id), ['average-dividend', 'desired-yield', 'eps', 'book-value-per-share']);
      expect(row(rows, 'average-dividend').cells, ['R\$ 3,00', '—']);
      expect(row(rows, 'eps').cells, ['—', 'R\$ 2,00']);
    });

    test('a mesma chave em dois modelos vira uma linha só, com os dois valores', () {
      final banks = valuation(
        3,
        ValuationModel.banks,
        inputs: {'book_value_per_share': 20.0, 'roe': 0.15, 'payout': 0.4, 'ke': 0.12},
      );
      final rim = valuation(
        4,
        ValuationModel.rim,
        inputs: {'book_value_per_share': 25.0, 'roe_current': 0.18, 'payout': 0.5, 'ke': 0.12, 'fade_years': 5.0},
      );

      final rows = build([banks, rim]).skip(6).toList();

      expect(rows.map((r) => r.id), [
        'book-value-per-share',
        'roe',
        'payout',
        'ke',
        'roe-current',
        'fade-years',
      ]);
      expect(row(rows, 'book-value-per-share').cells, ['R\$ 20,00', 'R\$ 25,00']);
      expect(row(rows, 'roe').cells, ['15,0%', '—']);
      expect(row(rows, 'fade-years').cells, ['—', '5']);
    });

    test('premissa que falta num cálculo que deveria ter o campo mostra o marcador, sem quebrar', () {
      final incomplete = valuation(5, ValuationModel.bazin, inputs: {'average_dividend': 3.0});

      final rows = build([incomplete, bazin]).skip(6).toList();

      expect(row(rows, 'desired-yield').cells, ['—', '6,0%']);
    });
  });

  group('formato de cada premissa', () {
    test('dinheiro, porcentagem, número e inteiro', () {
      expect(formatInputValue(1234.5, ValuationFieldFormat.money, locale: 'pt'), 'R\$ 1.234,50');
      expect(formatInputValue(0.06, ValuationFieldFormat.percent, locale: 'pt'), '6,0%');
      expect(formatInputValue(0.06, ValuationFieldFormat.percent, locale: 'en'), '6.0%');
      expect(formatInputValue(1.5, ValuationFieldFormat.number, locale: 'pt'), '1,50');
      expect(formatInputValue(3.0, ValuationFieldFormat.integer, locale: 'pt'), '3');
    });
  });

  group('o que difere', () {
    test('linha com a mesma célula em todos os cálculos não difere; com valores diferentes, difere', () {
      final rows = build([bazin, graham]);

      expect(row(rows, 'current-price').differs, isFalse);
      expect(row(rows, 'fair-price').differs, isTrue);
    });

    test('valor contra o marcador de ausente também é diferença', () {
      final rows = build([bazin, graham]).skip(6);

      expect(rows.every((r) => r.differs), isTrue);
    });

    test('dois cálculos do mesmo modelo com as mesmas premissas só diferem no que mudou', () {
      final other = valuation(
        6,
        ValuationModel.bazin,
        fair: 60,
        margin: 0.33,
        inputs: {'average_dividend': 3.0, 'desired_yield': 0.05},
      );

      final rows = build([bazin, other]);

      expect(row(rows, 'model').differs, isFalse);
      expect(row(rows, 'average-dividend').differs, isFalse);
      expect(row(rows, 'desired-yield').differs, isTrue);
      expect(row(rows, 'fair-price').differs, isTrue);
    });
  });

  group('seleção de 2 a 4 cálculos', () {
    test('tocar marca, tocar de novo desmarca', () {
      expect(toggleCompareSelection([], 7), [7]);
      expect(toggleCompareSelection([7, 8], 7), [8]);
    });

    test('o quinto não entra: a lista passa de 4 sem mudar', () {
      expect(toggleCompareSelection([1, 2, 3, 4], 5), [1, 2, 3, 4]);
      expect(toggleCompareSelection([1, 2, 3, 4], 4), [1, 2, 3]);
    });

    test('só dá para comparar com 2 a 4 marcados', () {
      expect(canCompare([]), isFalse);
      expect(canCompare([1]), isFalse);
      expect(canCompare([1, 2]), isTrue);
      expect(canCompare([1, 2, 3, 4]), isTrue);
      expect(canCompare([1, 2, 3, 4, 5]), isFalse);
    });
  });
}
