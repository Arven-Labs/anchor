import 'dart:convert';

import '../data/db.dart';
import '../domain/stock_valuation.dart';
import '../domain/valuation_fields.dart';
import '../models/stock_valuation.dart';

/// Cálculos de valuation salvos. Cada [save] é uma linha nova (nada é sobrescrito) e o
/// resultado é sempre recalculado aqui a partir das premissas, nunca aceito pronto da tela.
class StockValuationRepository {
  final DateTime Function() _now;

  /// [now] é o relógio (injetável nos testes): dá o ano de referência e a data do cálculo.
  StockValuationRepository({DateTime Function()? now}) : _now = now ?? DateTime.now;

  /// Calcula e grava. Lança [ValuationException] se as premissas não valem (nada é gravado) e
  /// [ArgumentError] se o ticker é vazio. Só as premissas do [model] são guardadas.
  Future<StockValuation> save({
    required String ticker,
    required ValuationModel model,
    required double currentPrice,
    required Map<String, double> inputs,
  }) async {
    final normalized = ticker.trim().toUpperCase();
    if (normalized.isEmpty) {
      throw ArgumentError('The ticker is required');
    }
    final outcome = calculateValuation(model, inputs, currentPrice);
    final kept = {
      for (final field in valuationFields[model]!) field.key: inputs[field.key]!,
    };
    final now = _now();
    final row = {
      'ticker': normalized,
      'model': model.value,
      'reference_year': now.year,
      'current_price': currentPrice,
      'fair_price': outcome.fairPrice,
      'safety_margin': outcome.safetyMargin,
      'verdict': outcome.verdict.name,
      'inputs_json': jsonEncode(kept),
      'created_at': now.toIso8601String(),
    };
    final db = await AppDatabase.instance();
    final id = await db.insert('stock_valuations', row);
    return StockValuation.tryFromMap({...row, 'id': id})!;
  }

  /// Cálculos salvos, do mais novo para o mais antigo; só os de [ticker] se informado.
  Future<List<StockValuation>> list({String? ticker}) async {
    final db = await AppDatabase.instance();
    final rows = await db.query(
      'stock_valuations',
      where: ticker == null ? null : 'ticker = ?',
      whereArgs: ticker == null ? null : [ticker.trim().toUpperCase()],
      orderBy: 'created_at DESC, id DESC',
    );
    // Linha de um modelo que esta versão não conhece é pulada, não derruba a lista.
    return rows.map(StockValuation.tryFromMap).whereType<StockValuation>().toList();
  }

  Future<void> delete(int id) async {
    final db = await AppDatabase.instance();
    await db.delete('stock_valuations', where: 'id = ?', whereArgs: [id]);
  }
}
