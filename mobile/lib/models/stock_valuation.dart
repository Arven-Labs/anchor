import 'dart:convert';

import '../domain/stock_valuation.dart';

/// Um cálculo de valuation salvo (tabela `stock_valuations`). Espelha a tabela
/// `valuation` do desktop; as premissas ficam juntas, em `inputs`, em vez de
/// numa tabela por modelo.
class StockValuation {
  final int id;
  final String ticker;
  final ValuationModel model;
  final int referenceYear;
  final double currentPrice;
  final double fairPrice;
  final double safetyMargin;
  final ValuationVerdict verdict;

  /// Premissas como foram calculadas: chaves de `valuation_fields.dart`, taxas em fração.
  final Map<String, double> inputs;
  final DateTime createdAt;

  const StockValuation({
    required this.id,
    required this.ticker,
    required this.model,
    required this.referenceYear,
    required this.currentPrice,
    required this.fairPrice,
    required this.safetyMargin,
    required this.verdict,
    required this.inputs,
    required this.createdAt,
  });

  /// Lê a linha do banco. Devolve null se o modelo ou o veredito não existem nesta versão
  /// do app (linha de uma versão futura ou corrompida), para a lista pular a linha em vez
  /// de falhar inteira.
  static StockValuation? tryFromMap(Map<String, Object?> map) {
    final model = valuationModelFromValue(map['model'] as String);
    final verdict = ValuationVerdict.values.where((v) => v.name == map['verdict']).firstOrNull;
    if (model == null || verdict == null) return null;
    final decoded = jsonDecode(map['inputs_json'] as String) as Map<String, dynamic>;
    return StockValuation(
      id: map['id'] as int,
      ticker: map['ticker'] as String,
      model: model,
      referenceYear: map['reference_year'] as int,
      currentPrice: (map['current_price'] as num).toDouble(),
      fairPrice: (map['fair_price'] as num).toDouble(),
      safetyMargin: (map['safety_margin'] as num).toDouble(),
      verdict: verdict,
      inputs: {for (final e in decoded.entries) e.key: (e.value as num).toDouble()},
      createdAt: DateTime.parse(map['created_at'] as String),
    );
  }
}
