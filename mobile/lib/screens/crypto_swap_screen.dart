import 'package:flutter/material.dart';

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

  String _formatDate(DateTime date) =>
      '${date.day.toString().padLeft(2, '0')}/'
      '${date.month.toString().padLeft(2, '0')}/${date.year}';

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

    setState(() {
      _suggesting = true;
      _hint = null;
    });
    try {
      final quote = await _dispatcher.fetchQuoteForAsset(from.asset);
      if (quote.currency != 'BRL') {
        setState(() => _hint = 'Sem cotação em reais para ${from.asset.ticker}.');
      } else {
        _valueController.text = (quantity * quote.price).toStringAsFixed(2);
        setState(
          () => _hint =
              'Preço de hoje (${_formatDate(DateTime.now())}); se a troca foi em outra data, ajuste o valor.',
        );
      }
    } on QuoteUnavailableException {
      setState(() => _hint = 'Cotação indisponível agora; digite o valor.');
    } catch (_) {
      setState(() => _hint = 'Sem cotação para ${from.asset.ticker}; digite o valor.');
    } finally {
      if (mounted) setState(() => _suggesting = false);
    }
  }

  Future<void> _save() async {
    final from = _from;
    final to = _to;
    if (from == null || to == null) return;

    final feeText = _feeController.text.trim().replaceAll(',', '.');
    final fee = feeText.isEmpty ? null : double.tryParse(feeText);
    if (feeText.isNotEmpty && fee == null) {
      setState(() => _error = 'A taxa precisa ser um número.');
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
      setState(() => _error = e.message);
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

    final sellable = _cryptoPositions.where((p) => p.netQuantity > 0).toList();

    return Scaffold(
      appBar: AppBar(title: const Text('Trocar cripto')),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          Text(
            'Trocar uma moeda por outra conta como venda da moeda entregue '
            'para o IR: entra nas vendas do mês (limite de R\$ 35 mil) e '
            'realiza ganho sobre o custo médio. Grava uma venda e uma compra '
            'pelo mesmo valor.',
            style: Theme.of(context).textTheme.bodySmall,
          ),
          const SizedBox(height: 16),
          if (sellable.isEmpty || _cryptoPositions.length < 2)
            const Text('É preciso ter saldo em uma cripto e outra cadastrada.')
          else ...[
            DropdownButtonFormField<Position>(
              initialValue: _from,
              decoration: const InputDecoration(labelText: 'Moeda que sai'),
              items: sellable
                  .map(
                    (p) => DropdownMenuItem(
                      value: p,
                      child: Text('${p.asset.ticker} (saldo ${p.netQuantity})'),
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
              decoration: const InputDecoration(labelText: 'Quantidade que sai'),
            ),
            const SizedBox(height: 16),
            DropdownButtonFormField<Position>(
              // Recria o campo quando a moeda que sai muda (a lista e o valor
              // inicial dependem dela).
              key: ValueKey('to-${_from?.asset.id}'),
              initialValue: _to,
              decoration: const InputDecoration(labelText: 'Moeda que entra'),
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
              decoration: const InputDecoration(labelText: 'Quantidade que entra'),
            ),
            const SizedBox(height: 16),
            TextField(
              controller: _valueController,
              keyboardType: const TextInputType.numberWithOptions(decimal: true),
              decoration: const InputDecoration(
                labelText: 'Valor de mercado da troca (BRL)',
              ),
            ),
            Align(
              alignment: Alignment.centerLeft,
              child: TextButton(
                onPressed: _suggesting ? null : _suggestValue,
                child: Text(
                  _suggesting ? 'Buscando...' : 'Usar preço de mercado',
                ),
              ),
            ),
            if (_hint != null)
              Text(_hint!, style: Theme.of(context).textTheme.bodySmall),
            const SizedBox(height: 16),
            TextField(
              controller: _feeController,
              keyboardType: const TextInputType.numberWithOptions(decimal: true),
              decoration: const InputDecoration(
                labelText: 'Taxa (opcional, BRL)',
              ),
            ),
            const SizedBox(height: 16),
            DropdownButtonFormField<Custodia?>(
              initialValue: _custodia,
              decoration: const InputDecoration(labelText: 'Custódia (opcional)'),
              items: [
                const DropdownMenuItem<Custodia?>(
                  value: null,
                  child: Text('Nenhuma'),
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
              title: Text('Data: ${_formatDate(_date)}'),
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
              child: const Text('Registrar troca'),
            ),
          ],
        ],
      ),
    );
  }
}
