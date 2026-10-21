import 'package:flutter/material.dart';

import '../models/asset.dart';
import '../models/custodia.dart';
import '../models/portfolio_transaction.dart';
import '../services/portfolio_repository.dart';

class AddTransactionScreen extends StatefulWidget {
  final PortfolioTransaction? transaction;

  const AddTransactionScreen({super.key, this.transaction});

  @override
  State<AddTransactionScreen> createState() => _AddTransactionScreenState();
}

class _AddTransactionScreenState extends State<AddTransactionScreen> {
  final _repository = PortfolioRepository();
  final _quantityController = TextEditingController();
  final _unitPriceController = TextEditingController();
  final _totalValueController = TextEditingController();
  final _feeController = TextEditingController();
  final _fiEmissorController = TextEditingController();
  final _fiTaxaController = TextEditingController();

  List<Asset> _assets = [];
  List<Custodia> _custodias = [];
  Asset? _selectedAsset;
  Custodia? _custodia;
  Custodia? _transferToCustodia;
  TransactionType _type = TransactionType.compra;
  DateTime _date = DateTime.now();
  String _fiIndexador = fiIndexadores.first;
  String _fiLiquidez = fiLiquidezOptions.first;
  DateTime? _fiDataVencimento;
  bool _loading = true;
  bool _saving = false;

  @override
  void initState() {
    super.initState();
    _loadData();
    _quantityController.addListener(() => setState(() {}));
    _unitPriceController.addListener(() => setState(() {}));
  }

  bool get _editing => widget.transaction != null;

  Future<void> _loadData() async {
    final assets = await _repository.listAssets();
    final custodias = await _repository.listCustodias();
    final tx = widget.transaction;

    setState(() {
      _assets = assets;
      _custodias = custodias;

      if (tx != null) {
        _type = tx.type;
        _selectedAsset = _findById(assets, (a) => a.id, tx.assetId);
        _custodia = _findById(custodias, (c) => c.id, tx.custodiaId);
        _transferToCustodia = _findById(
          custodias,
          (c) => c.id,
          tx.transferToCustodiaId,
        );
        _date = tx.date;
        _quantityController.text = tx.quantity?.toString() ?? '';
        _unitPriceController.text = tx.unitPrice?.toString() ?? '';
        _totalValueController.text = tx.totalValue.toString();
        _feeController.text = tx.fee?.toString() ?? '';
        _fiEmissorController.text = tx.fiEmissor ?? '';
        _fiIndexador = tx.fiIndexador ?? fiIndexadores.first;
        _fiTaxaController.text = tx.fiTaxaPercentual?.toString() ?? '';
        _fiDataVencimento = tx.fiDataVencimento;
        _fiLiquidez = tx.fiLiquidez ?? fiLiquidezOptions.first;
      } else {
        _selectedAsset = assets.isNotEmpty ? assets.first : null;
      }

      _loading = false;
    });
  }

  T? _findById<T>(List<T> items, int? Function(T) idOf, int? id) {
    if (id == null) return null;
    for (final item in items) {
      if (idOf(item) == id) return item;
    }
    return null;
  }

  double get _quantity =>
      double.tryParse(_quantityController.text.replaceAll(',', '.')) ?? 0;
  double get _unitPrice =>
      double.tryParse(_unitPriceController.text.replaceAll(',', '.')) ?? 0;
  double get _computedTotal => _quantity * _unitPrice;
  double get _manualTotal =>
      double.tryParse(_totalValueController.text.replaceAll(',', '.')) ?? 0;

  /// Mesma regra do `showFixedIncomeFields` do desktop
  /// (`TransactionSection.tsx:131-134`): só faz sentido registrar
  /// emissor/indexador/taxa/vencimento/liquidez numa compra de ativo de
  /// renda fixa.
  bool get _showFixedIncomeFields =>
      _type == TransactionType.compra &&
      (_selectedAsset?.assetClass.isFixedIncome ?? false);

  Future<void> _pickDate() async {
    final picked = await showDatePicker(
      context: context,
      initialDate: _date,
      firstDate: DateTime(2000),
      lastDate: DateTime.now(),
    );
    if (picked != null) setState(() => _date = picked);
  }

  Future<void> _pickFiDataVencimento() async {
    final picked = await showDatePicker(
      context: context,
      initialDate: _fiDataVencimento ?? DateTime.now(),
      firstDate: DateTime(2000),
      lastDate: DateTime(2100),
    );
    if (picked != null) setState(() => _fiDataVencimento = picked);
  }

  Future<void> _save() async {
    final needsAsset = _type.needsAsset;
    final asset = _selectedAsset;
    if (needsAsset && asset?.id == null) return;

    if (_type.needsTransferDestination) {
      if (_custodia?.id == null ||
          _transferToCustodia?.id == null ||
          _custodia!.id == _transferToCustodia!.id) {
        return;
      }
    }

    final needsQuantity = _type.needsQuantity;
    final needsUnitPrice = _type.needsUnitPrice;
    final autoTotal = needsQuantity && needsUnitPrice;

    final quantity = needsQuantity ? _quantity : null;
    final unitPrice = needsUnitPrice ? _unitPrice : null;
    final totalValue = autoTotal ? _computedTotal : _manualTotal;

    if (needsQuantity && quantity! <= 0) return;
    if (needsUnitPrice && unitPrice! <= 0) return;
    if (totalValue <= 0) return;

    // Taxa só em compra/venda; vazia = sem taxa, negativa ou inválida bloqueia.
    final feeText = _feeController.text.trim().replaceAll(',', '.');
    double? fee;
    if (needsUnitPrice && feeText.isNotEmpty) {
      fee = double.tryParse(feeText);
      if (fee == null || !fee.isFinite || fee < 0) return;
    }

    final showFixedIncomeFields = _showFixedIncomeFields;

    setState(() => _saving = true);

    final transaction = PortfolioTransaction(
      id: widget.transaction?.id,
      assetId: needsAsset ? asset!.id : null,
      type: _type,
      quantity: quantity,
      unitPrice: unitPrice,
      totalValue: totalValue,
      fee: fee,
      custodiaId: _custodia?.id,
      transferToCustodiaId: _type.needsTransferDestination
          ? _transferToCustodia?.id
          : null,
      date: _date,
      notes: widget.transaction?.notes,
      createdAt: widget.transaction?.createdAt ?? DateTime.now(),
      fiEmissor:
          showFixedIncomeFields && _fiEmissorController.text.trim().isNotEmpty
          ? _fiEmissorController.text.trim()
          : null,
      fiIndexador: showFixedIncomeFields ? _fiIndexador : null,
      fiTaxaPercentual: showFixedIncomeFields
          ? double.tryParse(_fiTaxaController.text.replaceAll(',', '.'))
          : null,
      fiDataVencimento: showFixedIncomeFields ? _fiDataVencimento : null,
      fiLiquidez: showFixedIncomeFields ? _fiLiquidez : null,
    );

    if (_editing) {
      await _repository.updateTransaction(transaction);
    } else {
      await _repository.insertTransaction(transaction);
    }

    if (mounted) Navigator.of(context).pop(true);
  }

  @override
  void dispose() {
    _quantityController.dispose();
    _unitPriceController.dispose();
    _totalValueController.dispose();
    _feeController.dispose();
    _fiEmissorController.dispose();
    _fiTaxaController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (_loading) {
      return const Scaffold(body: Center(child: CircularProgressIndicator()));
    }

    final needsAsset = _type.needsAsset;
    final needsQuantity = _type.needsQuantity;
    final needsUnitPrice = _type.needsUnitPrice;
    final needsTransferDestination = _type.needsTransferDestination;
    final autoTotal = needsQuantity && needsUnitPrice;
    final blockedByMissingAsset = needsAsset && _assets.isEmpty;

    return Scaffold(
      appBar: AppBar(
        title: Text(_editing ? 'Editar transação' : 'Lançar transação'),
      ),
      body: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            DropdownButtonFormField<TransactionType>(
              initialValue: _type,
              decoration: const InputDecoration(labelText: 'Tipo'),
              items: TransactionType.values
                  .map((t) => DropdownMenuItem(value: t, child: Text(t.label)))
                  .toList(),
              onChanged: (value) {
                if (value != null) setState(() => _type = value);
              },
            ),
            const SizedBox(height: 16),
            if (blockedByMissingAsset)
              const Text('Cadastre um ativo primeiro, na aba Portfolio.')
            else ...[
              if (needsAsset) ...[
                DropdownButtonFormField<Asset>(
                  initialValue: _selectedAsset,
                  decoration: const InputDecoration(labelText: 'Ativo'),
                  items: _assets
                      .map(
                        (a) => DropdownMenuItem(
                          value: a,
                          child: Text('${a.ticker} — ${a.name}'),
                        ),
                      )
                      .toList(),
                  onChanged: (value) => setState(() => _selectedAsset = value),
                ),
                const SizedBox(height: 16),
              ],
              if (_showFixedIncomeFields) ...[
                TextField(
                  controller: _fiEmissorController,
                  decoration: const InputDecoration(
                    labelText: 'Emissor (opcional)',
                  ),
                ),
                const SizedBox(height: 16),
                DropdownButtonFormField<String>(
                  initialValue: _fiIndexador,
                  decoration: const InputDecoration(labelText: 'Indexador'),
                  items: fiIndexadores
                      .map((i) => DropdownMenuItem(value: i, child: Text(i)))
                      .toList(),
                  onChanged: (value) {
                    if (value != null) setState(() => _fiIndexador = value);
                  },
                ),
                const SizedBox(height: 16),
                TextField(
                  controller: _fiTaxaController,
                  keyboardType: const TextInputType.numberWithOptions(
                    decimal: true,
                  ),
                  decoration: const InputDecoration(
                    labelText: 'Taxa contratada (%)',
                  ),
                ),
                const SizedBox(height: 16),
                ListTile(
                  contentPadding: EdgeInsets.zero,
                  title: Text(
                    _fiDataVencimento != null
                        ? 'Vencimento: ${_fiDataVencimento!.day.toString().padLeft(2, '0')}/'
                              '${_fiDataVencimento!.month.toString().padLeft(2, '0')}/${_fiDataVencimento!.year}'
                        : 'Vencimento (opcional)',
                  ),
                  trailing: const Icon(Icons.calendar_today),
                  onTap: _pickFiDataVencimento,
                ),
                const SizedBox(height: 16),
                DropdownButtonFormField<String>(
                  initialValue: _fiLiquidez,
                  decoration: const InputDecoration(labelText: 'Liquidez'),
                  items: fiLiquidezOptions
                      .map((l) => DropdownMenuItem(value: l, child: Text(l)))
                      .toList(),
                  onChanged: (value) {
                    if (value != null) setState(() => _fiLiquidez = value);
                  },
                ),
                const SizedBox(height: 16),
              ],
              if (needsQuantity) ...[
                TextField(
                  controller: _quantityController,
                  keyboardType: const TextInputType.numberWithOptions(
                    decimal: true,
                  ),
                  decoration: const InputDecoration(labelText: 'Quantidade'),
                ),
                const SizedBox(height: 16),
              ],
              if (needsUnitPrice) ...[
                TextField(
                  controller: _unitPriceController,
                  keyboardType: const TextInputType.numberWithOptions(
                    decimal: true,
                  ),
                  decoration: const InputDecoration(
                    labelText: 'Preço unitário',
                  ),
                ),
                const SizedBox(height: 16),
                TextField(
                  controller: _feeController,
                  keyboardType: const TextInputType.numberWithOptions(
                    decimal: true,
                  ),
                  decoration: const InputDecoration(
                    labelText: 'Taxa (opcional)',
                  ),
                ),
                const SizedBox(height: 16),
              ],
              if (autoTotal)
                Text('Total: ${_computedTotal.toStringAsFixed(2)}')
              else
                TextField(
                  controller: _totalValueController,
                  keyboardType: const TextInputType.numberWithOptions(
                    decimal: true,
                  ),
                  decoration: const InputDecoration(labelText: 'Valor'),
                ),
              const SizedBox(height: 16),
              DropdownButtonFormField<Custodia?>(
                initialValue: _custodia,
                decoration: InputDecoration(
                  labelText: needsTransferDestination
                      ? 'Custódia (origem)'
                      : 'Custódia (opcional)',
                ),
                items: [
                  if (!needsTransferDestination)
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
              if (needsTransferDestination) ...[
                const SizedBox(height: 16),
                DropdownButtonFormField<Custodia?>(
                  initialValue: _transferToCustodia,
                  decoration: const InputDecoration(
                    labelText: 'Custódia (destino)',
                  ),
                  items: _custodias
                      .map(
                        (c) => DropdownMenuItem(value: c, child: Text(c.label)),
                      )
                      .toList(),
                  onChanged: (value) =>
                      setState(() => _transferToCustodia = value),
                ),
              ],
              const SizedBox(height: 16),
              ListTile(
                contentPadding: EdgeInsets.zero,
                title: Text(
                  'Data: ${_date.day.toString().padLeft(2, '0')}/'
                  '${_date.month.toString().padLeft(2, '0')}/${_date.year}',
                ),
                trailing: const Icon(Icons.calendar_today),
                onTap: _pickDate,
              ),
              const SizedBox(height: 24),
              FilledButton(
                onPressed: _saving ? null : _save,
                child: const Text('Salvar'),
              ),
            ],
          ],
        ),
      ),
    );
  }
}
