import 'package:flutter/material.dart';

import '../domain/valuation_compare.dart';
import '../l10n/app_localizations.dart';
import '../l10n/valuation_labels.dart';
import '../models/stock_valuation.dart';

/// Comparação lado a lado dos valuations salvos escolhidos na lista (2 a 4): uma coluna por cálculo,
/// uma linha por dado do resultado e por premissa; a linha que difere ganha um aviso. Porta de
/// `CompareValuations.tsx` do desktop. Só leitura: não grava nem recalcula nada.
class StockValuationCompareScreen extends StatelessWidget {
  final List<StockValuation> valuations;

  const StockValuationCompareScreen({super.key, required this.valuations});

  static const _labelWidth = 170.0;
  static const _cellWidth = 120.0;

  String _rowLabel(AppLocalizations l, CompareRow row) => switch (row.kind) {
    CompareRowKind.model => l.valuationCompareRowModel,
    CompareRowKind.referenceYear => l.valuationCompareRowReferenceYear,
    CompareRowKind.currentPrice => l.valuationCompareRowCurrentPrice,
    CompareRowKind.fairPrice => l.valuationCompareRowFairPrice,
    CompareRowKind.safetyMargin => l.valuationCompareRowSafetyMargin,
    CompareRowKind.verdict => l.valuationCompareRowVerdict,
    CompareRowKind.assumption => valuationFieldLabel(l, row.fieldKey!),
  };

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final rows = buildComparisonRows(
      valuations,
      locale: l.localeName,
      missing: compareMissing,
      modelName: (model) => valuationModelName(l, model),
      verdictName: (verdict) => valuationVerdictShort(l, verdict),
    );
    final differsColor = Theme.of(context).colorScheme.tertiaryContainer;
    return Scaffold(
      appBar: AppBar(title: Text(l.valuationCompareTitle(valuations.first.ticker))),
      body: SingleChildScrollView(
        child: SingleChildScrollView(
          scrollDirection: Axis.horizontal,
          child: Column(
            children: [
              Row(
                children: [
                  const SizedBox(width: _labelWidth),
                  for (var i = 0; i < valuations.length; i++)
                    SizedBox(
                      width: _cellWidth,
                      child: Padding(
                        padding: const EdgeInsets.all(8),
                        child: Text(
                          l.valuationCompareColumn(i + 1),
                          style: const TextStyle(fontWeight: FontWeight.bold),
                        ),
                      ),
                    ),
                ],
              ),
              for (final row in rows)
                Container(
                  key: Key('compare-row-${row.id}'),
                  color: row.differs ? differsColor : null,
                  child: Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      SizedBox(
                        width: _labelWidth,
                        child: Padding(
                          padding: const EdgeInsets.all(8),
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Text(_rowLabel(l, row)),
                              if (row.differs)
                                Text(
                                  l.valuationCompareDiffers,
                                  key: Key('compare-differs-${row.id}'),
                                  style: Theme.of(context).textTheme.labelSmall,
                                ),
                            ],
                          ),
                        ),
                      ),
                      for (final cell in row.cells)
                        SizedBox(
                          width: _cellWidth,
                          child: Padding(padding: const EdgeInsets.all(8), child: Text(cell)),
                        ),
                    ],
                  ),
                ),
            ],
          ),
        ),
      ),
    );
  }
}
