import 'package:flutter/material.dart';

import '../l10n/app_localizations.dart';
import '../l10n/home_labels.dart';
import '../l10n/registration_labels.dart';
import '../models/asset.dart';
import '../models/quote.dart';
import '../services/quote_dispatcher.dart';
import '../services/quote_exceptions.dart';

class QuoteSearchScreen extends StatefulWidget {
  const QuoteSearchScreen({super.key});

  @override
  State<QuoteSearchScreen> createState() => _QuoteSearchScreenState();
}

class _QuoteSearchScreenState extends State<QuoteSearchScreen> {
  final _tickerController = TextEditingController();
  final _dispatcher = QuoteDispatcher();

  AssetClass _assetClass = AssetClass.acaoBr;
  bool _loading = false;
  String? _error;
  Quote? _quote;

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
      setState(() => _quote = quote);
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
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
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
            if (_quote != null) _QuoteCard(quote: _quote!),
          ],
        ),
      ),
    );
  }
}

class _QuoteCard extends StatelessWidget {
  final Quote quote;

  const _QuoteCard({required this.quote});

  @override
  Widget build(BuildContext context) {
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
          ],
        ),
      ),
    );
  }
}
