import 'package:flutter/material.dart';

import '../domain/tax_format.dart' show formatDay;
import '../l10n/app_localizations.dart';
import '../l10n/crypto_tax_labels.dart';
import '../models/asset.dart';
import '../models/crypto_swap.dart';
import '../models/custodia.dart';
import '../models/portfolio_transaction.dart';
import '../models/position.dart';
import '../services/portfolio_repository.dart';
import '../services/quote_dispatcher.dart';
import '../services/quote_exceptions.dart';

/// Permuta cripto↔cripto (ver `buildCryptoSwap`): registra a troca como uma
/// venda da moeda entregue e uma compra da recebida, pelo mesmo valor em reais.
class CryptoSwapScreen extends StatefulWidget {
  const CryptoSwapScreen({super.key});

  @override
  State<CryptoSwapScreen> createState() => _CryptoSwapScreenState();
}

class _CryptoSwapScreenState extends State<CryptoSwapScreen> {
  final _repository = PortfolioRepository();
  final _dispatcher = QuoteDispatcher();
  final _quantityOutController = TextEditingController();
  final _quantityInController = TextEditingController();
  final _valueController = TextEditingController();
  final _feeController = TextEditingController();

  List<Position> _cryptoPositions = [];
  List<Custodia> _custodias = [];
  Position? _from;
  Position? _to;
  Custodia? _custodia;
  DateTime _date = DateTime.now();
  bool _loading = true;
  bool _saving = false;
  bool _suggesting = false;
  String? _error;
  String? _hint;

  @override
  void initState() {
    super.initState();
    _loadData();
  }

  Future<void> _loadData() async {
    final positions = await _repository.getPositions();
    final custodias = await _repository.listCustodias();
    if (!mounted) return;
    setState(() {
      _cryptoPositions = positions
          .where((p) => p.asset.assetClass == AssetClass.cripto)
          .toList();
      _custodias = custodias;
      _loading = false;
    });
  }

  double _number(TextEditingController controller) =>
      double.tryParse(controller.text.replaceAll(',', '.')) ?? 0;

  Future<void> _pickDate() async {
    final picked = await showDatePicker(
      context: context,
      initialDate: _date,
      firstDate: DateTime(2000),
      lastDate: DateTime.now(),
    );
    if (picked != null) setState(() => _date = picked);
  }

  /// Só preenche o campo de valor com quantidade × preço atual em reais; o
  /// dono confere e pode mudar. O preço é o de hoje, não o da data da troca.
  Future<void> _suggestValue() async {
    final from = _from;
    final quantity = _number(_quantityOutController);
    if (from == null || quantity <= 0) return;
    final l = AppLocalizations.of(context);

    setState(() {
      _suggesting = true;
      _hint = null;
    });
    try {
      final quote = await _dispatcher.fetchQuoteForAsset(from.asset);
      if (quote.currency != 'BRL') {
        setState(() => _hint = l.swapHintNoBrlQuote(from.asset.ticker));
      } else {
        _valueController.text = (quantity * quote.price).toStringAsFixed(2);
        setState(
          () => _hint = l.swapHintTodayPrice(
            formatDay(DateTime.now(), locale: l.localeName),
          ),
        );
      }
    } on QuoteUnavailableException {
      setState(() => _hint = l.swapHintQuoteUnavailable);
    } catch (_) {
      setState(() => _hint = l.swapHintNoQuote(from.asset.ticker));
    } finally {
      if (mounted) setState(() => _suggesting = false);
    }
  }

  Future<void> _save() async {
    final from = _from;
    final to = _to;
    if (from == null || to == null) return;
    final l = AppLocalizations.of(context);

    final feeText = _feeController.text.trim().replaceAll(',', '.');
    final fee = feeText.isEmpty ? null : double.tryParse(feeText);
    if (feeText.isNotEmpty && fee == null) {
      setState(() => _error = l.swapFeeNotNumber);
      return;
    }

    final ({PortfolioTransaction sale, PortfolioTransaction purchase}) swap;
    try {
      swap = buildCryptoSwap(
        from: from.asset,
        to: to.asset,
        quantityOut: _number(_quantityOutController),
        quantityIn: _number(_quantityInController),
        valueBrl: _number(_valueController),
        date: _date,
        available: from.netQuantity,
        fee: fee,
        custodiaId: _custodia?.id,
      );
    } on CryptoSwapException catch (e) {
      setState(() => _error = cryptoSwapErrorText(l, e));
      return;
    }

    setState(() {
      _saving = true;
      _error = null;
    });
    await _repository.insertTransactions([swap.sale, swap.purchase]);
    if (mounted) Navigator.of(context).pop(true);
  }

  @override
  void dispose() {
    _quantityOutController.dispose();
    _quantityInController.dispose();
    _valueController.dispose();
    _feeController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (_loading) {
      return const Scaffold(body: Center(child: CircularProgressIndicator()));
    }

    final l = AppLocalizations.of(context);
    final sellable = _cryptoPositions.where((p) => p.netQuantity > 0).toList();

    return Scaffold(
      appBar: AppBar(title: Text(l.portfolioSwapCrypto)),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          Text(l.swapIntro, style: Theme.of(context).textTheme.bodySmall),
          const SizedBox(height: 16),
          if (sellable.isEmpty || _cryptoPositions.length < 2)
            Text(l.swapNeedTwoCoins)
          else ...[
            DropdownButtonFormField<Position>(
              initialValue: _from,
              decoration: InputDecoration(labelText: l.swapCoinOut),
              items: sellable
                  .map(
                    (p) => DropdownMenuItem(
                      value: p,
                      child: Text(l.swapCoinOutItem(p.asset.ticker, '${p.netQuantity}')),
                    ),
                  )
                  .toList(),
              onChanged: (value) => setState(() {
                _from = value;
                // A moeda que entra não pode ser a que sai.
                if (_to?.asset.id == value?.asset.id) _to = null;
              }),
            ),
            const SizedBox(height: 16),
            TextField(
              controller: _quantityOutController,
              keyboardType: const TextInputType.numberWithOptions(decimal: true),
              decoration: InputDecoration(labelText: l.swapQuantityOut),
            ),
            const SizedBox(height: 16),
            DropdownButtonFormField<Position>(
              // Recria o campo quando a moeda que sai muda (a lista e o valor
              // inicial dependem dela).
              key: ValueKey('to-${_from?.asset.id}'),
              initialValue: _to,
              decoration: InputDecoration(labelText: l.swapCoinIn),
              items: _cryptoPositions
                  .where((p) => p.asset.id != _from?.asset.id)
                  .map(
                    (p) => DropdownMenuItem(
                      value: p,
                      child: Text('${p.asset.ticker} — ${p.asset.name}'),
                    ),
                  )
                  .toList(),
              onChanged: (value) => setState(() => _to = value),
            ),
            const SizedBox(height: 16),
            TextField(
              controller: _quantityInController,
              keyboardType: const TextInputType.numberWithOptions(decimal: true),
              decoration: InputDecoration(labelText: l.swapQuantityIn),
            ),
            const SizedBox(height: 16),
            TextField(
              controller: _valueController,
              keyboardType: const TextInputType.numberWithOptions(decimal: true),
              decoration: InputDecoration(labelText: l.swapMarketValue),
            ),
            Align(
              alignment: Alignment.centerLeft,
              child: TextButton(
                onPressed: _suggesting ? null : _suggestValue,
                child: Text(
                  _suggesting ? l.swapSearching : l.swapUseMarketPrice,
                ),
              ),
            ),
            if (_hint != null)
              Text(_hint!, style: Theme.of(context).textTheme.bodySmall),
            const SizedBox(height: 16),
            TextField(
              controller: _feeController,
              keyboardType: const TextInputType.numberWithOptions(decimal: true),
              decoration: InputDecoration(labelText: l.swapFee),
            ),
            const SizedBox(height: 16),
            DropdownButtonFormField<Custodia?>(
              initialValue: _custodia,
              decoration: InputDecoration(labelText: l.swapCustody),
              items: [
                DropdownMenuItem<Custodia?>(
                  value: null,
                  child: Text(l.swapNoCustody),
                ),
                ..._custodias.map(
                  (c) => DropdownMenuItem(value: c, child: Text(c.label)),
                ),
              ],
              onChanged: (value) => setState(() => _custodia = value),
            ),
            const SizedBox(height: 16),
            ListTile(
              contentPadding: EdgeInsets.zero,
              title: Text(l.swapDate(formatDay(_date, locale: l.localeName))),
              trailing: const Icon(Icons.calendar_today),
              onTap: _pickDate,
            ),
            if (_error != null) ...[
              const SizedBox(height: 8),
              Text(
                _error!,
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
            ],
            const SizedBox(height: 24),
            FilledButton(
              onPressed: _saving || _from == null || _to == null ? null : _save,
              child: Text(l.swapRegister),
            ),
          ],
        ],
      ),
    );
  }
}
