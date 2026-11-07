import 'package:flutter/material.dart';

import '../domain/number_locale.dart';
import '../domain/tax_format.dart';
import '../l10n/app_localizations.dart';
import '../l10n/valuation_labels.dart';
import '../models/stock_valuation.dart';
import '../services/stock_valuation_repository.dart';
import 'stock_valuation_screen.dart';

/// Cálculos de valuation salvos de um ticker, do mais novo para o mais antigo. Tocar abre o
/// cálculo com as premissas preenchidas; a lixeira exclui (sem confirmação, como nas outras
/// listas do app).
class StockValuationsScreen extends StatefulWidget {
  final String ticker;

  const StockValuationsScreen({super.key, required this.ticker});

  @override
  State<StockValuationsScreen> createState() => _StockValuationsScreenState();
}

class _StockValuationsScreenState extends State<StockValuationsScreen> {
  final _repository = StockValuationRepository();

  bool _loading = true;
  List<StockValuation> _items = const [];

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    final items = await _repository.list(ticker: widget.ticker);
    if (!mounted) return;
    setState(() {
      _items = items;
      _loading = false;
    });
  }

  Future<void> _delete(StockValuation item) async {
    await _repository.delete(item.id);
    await _load();
  }

  Future<void> _open(StockValuation item) async {
    await Navigator.of(context).push(
      MaterialPageRoute(
        builder: (_) => StockValuationScreen(
          ticker: item.ticker,
          currentPrice: item.currentPrice,
          initial: item,
        ),
      ),
    );
    // Voltar pode ter salvado um cálculo novo.
    await _load();
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final locale = l.localeName;
    String brl(double value) => formatBrl(value, locale: locale);
    return Scaffold(
      appBar: AppBar(title: Text(l.valuationSavedList)),
      body: _loading
          ? const Center(child: CircularProgressIndicator())
          : _items.isEmpty
          ? Center(child: Text(l.valuationSavedEmpty(widget.ticker)))
          : ListView(
              children: [
                for (final item in _items)
                  ListTile(
                    title: Text(valuationModelName(l, item.model)),
                    subtitle: Text(
                      l.valuationSavedDetail(
                        brl(item.currentPrice),
                        brl(item.fairPrice),
                        '${formatFixed(item.safetyMargin * 100, 1, locale: locale)}%',
                        valuationVerdictShort(l, item.verdict),
                        formatDay(item.createdAt, locale: locale),
                      ),
                    ),
                    trailing: IconButton(
                      icon: const Icon(Icons.delete_outline),
                      tooltip: l.valuationDelete,
                      onPressed: () => _delete(item),
                    ),
                    onTap: () => _open(item),
                  ),
              ],
            ),
    );
  }
}
