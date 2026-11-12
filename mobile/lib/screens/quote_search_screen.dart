import 'package:material_ui/material_ui.dart';

import '../l10n/app_localizations.dart';
import '../l10n/home_labels.dart';
import '../l10n/registration_labels.dart';
import '../models/asset.dart';
import '../models/quote.dart';
import '../services/quote_dispatcher.dart';
import '../services/quote_exceptions.dart';
import '../services/thesis_repository.dart';
import 'asset_theses_section.dart';
import 'stock_valuation_screen.dart';

class QuoteSearchScreen extends StatefulWidget {
  /// Quem busca a cotação; o padrão usa a rede. Injetável para teste, sem rede.
  final QuoteDispatcher? dispatcher;

  /// Repositório das teses do bloco; o padrão usa o banco do app. Injetável para teste.
  final ThesisRepository? thesisRepository;

  const QuoteSearchScreen({super.key, this.dispatcher, this.thesisRepository});

  @override
  State<QuoteSearchScreen> createState() => _QuoteSearchScreenState();
}

/// Classes em que a busca mostra as teses do ticker (as mesmas do `AssetThesesSidebar` do desktop).
const _thesesClasses = {AssetClass.acaoBr, AssetClass.acaoInternacional};

class _QuoteSearchScreenState extends State<QuoteSearchScreen> {
  final _tickerController = TextEditingController();
  late final QuoteDispatcher _dispatcher = widget.dispatcher ?? QuoteDispatcher();

  AssetClass _assetClass = AssetClass.acaoBr;
  bool _loading = false;
  String? _error;
  Quote? _quote;

  /// Classe e ticker da busca que gerou [_quote]: trocar a classe depois não muda o cartão.
  AssetClass? _quoteClass;
  String _quoteTicker = '';

  Future<void> _search() async {
    final ticker = _tickerController.text.trim().toUpperCase();
    if (ticker.isEmpty) return;
    final l = AppLocalizations.of(context);

    setState(() {
      _loading = true;
      _error = null;
      _quote = null;
    });

    try {
      final quote = await _dispatcher.fetchQuoteForTicker(ticker, _assetClass);
      setState(() {
        _quote = quote;
        _quoteClass = _assetClass;
        _quoteTicker = ticker;
      });
    } on QuoteUnavailableException {
      setState(() => _error = l.quoteSourceUnavailable);
    } catch (_) {
      setState(
        () => _error = l.quoteSearchFailed(ticker, assetClassName(l, _assetClass)),
      );
    } finally {
      setState(() => _loading = false);
    }
  }

  void _openValuation() {
    final quote = _quote;
    if (quote == null) return;
    Navigator.of(context).push(
      MaterialPageRoute(
        builder: (_) => StockValuationScreen(ticker: _quoteTicker, currentPrice: quote.price),
      ),
    );
  }

  @override
  void dispose() {
    _tickerController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return Scaffold(
      appBar: AppBar(title: const Text('Anchor')),
      body: Padding(
        padding: const EdgeInsets.all(16),
        // Rola: com o cartão de cotação e o bloco de teses a tela passa de 390 px de altura.
        child: ListView(
          children: [
            DropdownButtonFormField<AssetClass>(
              initialValue: _assetClass,
              decoration: InputDecoration(labelText: l.classLabel),
              // Renda fixa e os cadastros manuais não têm cotação nenhuma
              // (nem no desktop) — não fazem sentido numa tela de busca.
              items: AssetClass.values
                  .where((c) => c.hasAutoQuote)
                  .map(
                    (c) => DropdownMenuItem(
                      value: c,
                      child: Text(assetClassName(l, c)),
                    ),
                  )
                  .toList(),
              onChanged: (value) {
                if (value != null) setState(() => _assetClass = value);
              },
            ),
            const SizedBox(height: 16),
            Row(
              children: [
                Expanded(
                  child: TextField(
                    controller: _tickerController,
                    textCapitalization: TextCapitalization.characters,
                    decoration: InputDecoration(
                      labelText: l.tickerLabel(assetClassName(l, _assetClass)),
                      hintText: assetTickerHint(l, _assetClass),
                    ),
                    onSubmitted: (_) => _search(),
                  ),
                ),
                const SizedBox(width: 12),
                FilledButton(
                  onPressed: _loading ? null : _search,
                  child: Text(l.searchButton),
                ),
              ],
            ),
            const SizedBox(height: 24),
            if (_loading) const Center(child: CircularProgressIndicator()),
            if (_error != null)
              Text(
                _error!,
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
            if (_quote != null)
              _QuoteCard(
                quote: _quote!,
                // Valuation por modelos de ação: só para Ação BR.
                onValuation: _quoteClass == AssetClass.acaoBr ? _openValuation : null,
              ),
            // Teses do ticker: só Ação BR e Ação internacional, como no desktop. A chave refaz
            // o bloco quando a busca muda de ticker ou de classe.
            if (_quote != null && _quoteClass != null && _thesesClasses.contains(_quoteClass)) ...[
              const SizedBox(height: 24),
              AssetThesesSection(
                key: ValueKey('$_quoteTicker-${_quoteClass!.value}'),
                ticker: _quoteTicker,
                assetClass: _quoteClass!,
                repository: widget.thesisRepository,
              ),
            ],
          ],
        ),
      ),
    );
  }
}

class _QuoteCard extends StatelessWidget {
  final Quote quote;
  final VoidCallback? onValuation;

  const _QuoteCard({required this.quote, this.onValuation});

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              quote.name ?? quote.ticker,
              style: Theme.of(context).textTheme.titleMedium,
            ),
            const SizedBox(height: 4),
            Text(
              quote.exchange ?? '',
              style: Theme.of(context).textTheme.bodySmall,
            ),
            const SizedBox(height: 12),
            Text(
              '${quote.currency ?? ''} ${quote.price.toStringAsFixed(2)}',
              style: Theme.of(context).textTheme.headlineMedium?.copyWith(
                color: Theme.of(context).colorScheme.primary,
                fontWeight: FontWeight.bold,
              ),
            ),
            if (onValuation != null) ...[
              const SizedBox(height: 12),
              OutlinedButton(onPressed: onValuation, child: Text(l.valuationButton)),
            ],
          ],
        ),
      ),
    );
  }
}
