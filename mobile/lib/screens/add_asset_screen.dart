import 'package:material_ui/material_ui.dart';

import '../domain/number_locale.dart' show formatFixed;
import '../l10n/app_localizations.dart';
import '../l10n/fund_tax_labels.dart' show fundTermName;
import '../l10n/home_labels.dart';
import '../l10n/registration_labels.dart';
import '../models/asset.dart';
import '../models/fund.dart';
import '../services/portfolio_repository.dart';
import '../services/quote_dispatcher.dart';
import '../services/quote_exceptions.dart';

class AddAssetScreen extends StatefulWidget {
  const AddAssetScreen({super.key});

  @override
  State<AddAssetScreen> createState() => _AddAssetScreenState();
}

class _AddAssetScreenState extends State<AddAssetScreen> {
  final _tickerController = TextEditingController();
  final _nameController = TextEditingController();
  final _sharesOwnedController = TextEditingController();
  final _totalSharesController = TextEditingController();
  final _companyValuationController = TextEditingController();
  final _dispatcher = QuoteDispatcher();
  final _repository = PortfolioRepository();

  AssetClass _assetClass = AssetClass.acaoBr;
  FundTerm? _fundTerm;
  String _currency = AssetClass.acaoBr.defaultCurrency;
  String? _externalId;
  bool _searching = false;
  bool _saving = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    for (final controller in [
      _sharesOwnedController,
      _totalSharesController,
      _companyValuationController,
    ]) {
      controller.addListener(() => setState(() {}));
    }
  }

  void _onAssetClassChanged(AssetClass value) {
    setState(() {
      _assetClass = value;
      _currency = value.defaultCurrency;
      _externalId = null;
    });
  }

  Future<void> _search() async {
    final ticker = _tickerController.text.trim().toUpperCase();
    if (ticker.isEmpty) return;
    final l = AppLocalizations.of(context);

    setState(() {
      _searching = true;
      _error = null;
      _externalId = null;
    });

    try {
      final quote = await _dispatcher.fetchQuoteForTicker(ticker, _assetClass);
      setState(() {
        _nameController.text = quote.name ?? ticker;
        _currency = quote.currency ?? _assetClass.defaultCurrency;
        _externalId = quote.externalId;
      });
    } on QuoteUnavailableException {
      setState(() => _error = l.quoteSourceUnavailable);
    } catch (_) {
      setState(
        () => _error = l.addAssetLookupFailed(ticker, assetClassName(l, _assetClass)),
      );
    } finally {
      setState(() => _searching = false);
    }
  }

  double? get _sharesOwned =>
      double.tryParse(_sharesOwnedController.text.replaceAll(',', '.'));
  double? get _totalShares =>
      double.tryParse(_totalSharesController.text.replaceAll(',', '.'));
  double? get _companyValuation =>
      double.tryParse(_companyValuationController.text.replaceAll(',', '.'));

  double? get _equityPercentual {
    final owned = _sharesOwned, total = _totalShares;
    if (owned == null || total == null || total <= 0) return null;
    return owned / total;
  }

  double? get _equityParticipationValue {
    final percentual = _equityPercentual, valuation = _companyValuation;
    if (percentual == null || valuation == null) return null;
    return percentual * valuation;
  }

  /// `Participação societária (12,50% — 1250,00)`: o título e, quando os campos permitem
  /// calcular, a porcentagem e o valor da participação.
  String _equityTitle(AppLocalizations l) {
    final percent = _equityPercentual, value = _equityParticipationValue;
    final title = l.addAssetEquity;
    if (percent == null) return title;
    final shownPercent = formatFixed(percent * 100, 2, locale: l.localeName);
    if (value == null) return l.addAssetEquityPercent(title, shownPercent);
    return l.addAssetEquityPercentValue(
      title,
      shownPercent,
      formatFixed(value, 2, locale: l.localeName),
    );
  }

  Future<void> _save() async {
    final ticker = _tickerController.text.trim().toUpperCase();
    final name = _nameController.text.trim();
    if (ticker.isEmpty || name.isEmpty) return;

    setState(() => _saving = true);

    final assetId = await _repository.insertAsset(
      Asset(
        ticker: ticker,
        name: name,
        assetClass: _assetClass,
        currency: _currency,
        createdAt: DateTime.now(),
        externalId: _externalId,
        equitySharesOwned: _assetClass == AssetClass.empresaNaoListada
            ? _sharesOwned
            : null,
        equityTotalShares: _assetClass == AssetClass.empresaNaoListada
            ? _totalShares
            : null,
        equityCompanyValuation: _assetClass == AssetClass.empresaNaoListada
            ? _companyValuation
            : null,
      ),
    );
    final fundTerm = _fundTerm;
    if (_assetClass == AssetClass.fundo && fundTerm != null) {
      await _repository.setFundTerm(assetId, fundTerm);
    }

    if (mounted) Navigator.of(context).pop(true);
  }

  @override
  void dispose() {
    _tickerController.dispose();
    _nameController.dispose();
    _sharesOwnedController.dispose();
    _totalSharesController.dispose();
    _companyValuationController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final hasAutoQuote = _assetClass.hasAutoQuote;
    final l = AppLocalizations.of(context);

    return Scaffold(
      appBar: AppBar(title: Text(l.portfolioAddAsset)),
      body: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
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
                  ),
                ),
                if (hasAutoQuote) ...[
                  const SizedBox(width: 12),
                  OutlinedButton(
                    onPressed: _searching ? null : _search,
                    child: Text(l.searchButton),
                  ),
                ],
              ],
            ),
            if (_error != null) ...[
              const SizedBox(height: 8),
              Text(
                _error!,
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
            ],
            const SizedBox(height: 16),
            TextField(
              controller: _nameController,
              decoration: InputDecoration(labelText: l.addAssetNameLabel),
            ),
            const SizedBox(height: 16),
            DropdownButtonFormField<AssetClass>(
              initialValue: _assetClass,
              decoration: InputDecoration(labelText: l.classLabel),
              items: AssetClass.values
                  .map(
                    (c) => DropdownMenuItem(
                      value: c,
                      child: Text(assetClassName(l, c)),
                    ),
                  )
                  .toList(),
              onChanged: (value) {
                if (value != null) _onAssetClassChanged(value);
              },
            ),
            if (_assetClass == AssetClass.fundo) ...[
              const SizedBox(height: 16),
              DropdownButtonFormField<FundTerm>(
                initialValue: _fundTerm,
                decoration: InputDecoration(labelText: l.addAssetFundTerm),
                items: FundTerm.values
                    .map(
                      (t) => DropdownMenuItem(
                        value: t,
                        child: Text(fundTermName(l, t)),
                      ),
                    )
                    .toList(),
                onChanged: (value) => setState(() => _fundTerm = value),
              ),
            ],
            if (_assetClass == AssetClass.empresaNaoListada) ...[
              const SizedBox(height: 16),
              Text(
                _equityTitle(l),
                style: Theme.of(context).textTheme.bodySmall,
              ),
              const SizedBox(height: 8),
              TextField(
                controller: _sharesOwnedController,
                keyboardType: const TextInputType.numberWithOptions(
                  decimal: true,
                ),
                decoration: InputDecoration(labelText: l.addAssetSharesOwned),
              ),
              const SizedBox(height: 16),
              TextField(
                controller: _totalSharesController,
                keyboardType: const TextInputType.numberWithOptions(
                  decimal: true,
                ),
                decoration: InputDecoration(labelText: l.addAssetTotalShares),
              ),
              const SizedBox(height: 16),
              TextField(
                controller: _companyValuationController,
                keyboardType: const TextInputType.numberWithOptions(
                  decimal: true,
                ),
                decoration: InputDecoration(
                  labelText: l.addAssetValuation(_currency),
                ),
              ),
            ],
            const SizedBox(height: 24),
            FilledButton(
              onPressed: _saving ? null : _save,
              child: Text(l.actionSave),
            ),
          ],
        ),
      ),
    );
  }
}
