import 'package:flutter/material.dart';

import '../models/asset.dart';
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
      setState(
        () => _error =
            'Fonte de cotação indisponível agora (limite ou rede) — tente de novo em instantes.',
      );
    } catch (_) {
      setState(
        () => _error =
            'Não foi possível buscar $ticker (${_assetClass.label}) — confira o ticker.',
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

  Future<void> _save() async {
    final ticker = _tickerController.text.trim().toUpperCase();
    final name = _nameController.text.trim();
    if (ticker.isEmpty || name.isEmpty) return;

    setState(() => _saving = true);

    await _repository.insertAsset(
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

    return Scaffold(
      appBar: AppBar(title: const Text('Adicionar ativo')),
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
                      labelText: 'Ticker (${_assetClass.label})',
                      hintText: _assetClass.tickerHint,
                    ),
                  ),
                ),
                if (hasAutoQuote) ...[
                  const SizedBox(width: 12),
                  OutlinedButton(
                    onPressed: _searching ? null : _search,
                    child: const Text('Buscar'),
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
              decoration: const InputDecoration(labelText: 'Nome'),
            ),
            const SizedBox(height: 16),
            DropdownButtonFormField<AssetClass>(
              initialValue: _assetClass,
              decoration: const InputDecoration(labelText: 'Classe'),
              items: AssetClass.values
                  .map((c) => DropdownMenuItem(value: c, child: Text(c.label)))
                  .toList(),
              onChanged: (value) {
                if (value != null) _onAssetClassChanged(value);
              },
            ),
            if (_assetClass == AssetClass.empresaNaoListada) ...[
              const SizedBox(height: 16),
              Text(
                'Participação societária'
                '${_equityPercentual != null ? ' (${(_equityPercentual! * 100).toStringAsFixed(2)}%'
                          '${_equityParticipationValue != null ? ' — ${_equityParticipationValue!.toStringAsFixed(2)}' : ''})' : ''}',
                style: Theme.of(context).textTheme.bodySmall,
              ),
              const SizedBox(height: 8),
              TextField(
                controller: _sharesOwnedController,
                keyboardType: const TextInputType.numberWithOptions(
                  decimal: true,
                ),
                decoration: const InputDecoration(
                  labelText: 'Cotas/ações possuídas',
                ),
              ),
              const SizedBox(height: 16),
              TextField(
                controller: _totalSharesController,
                keyboardType: const TextInputType.numberWithOptions(
                  decimal: true,
                ),
                decoration: const InputDecoration(
                  labelText: 'Total de cotas/ações da empresa',
                ),
              ),
              const SizedBox(height: 16),
              TextField(
                controller: _companyValuationController,
                keyboardType: const TextInputType.numberWithOptions(
                  decimal: true,
                ),
                decoration: InputDecoration(
                  labelText: 'Valuation da empresa ($_currency)',
                ),
              ),
            ],
            const SizedBox(height: 24),
            FilledButton(
              onPressed: _saving ? null : _save,
              child: const Text('Salvar'),
            ),
          ],
        ),
      ),
    );
  }
}
