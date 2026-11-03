import 'package:flutter/material.dart';

import '../l10n/app_localizations.dart';
import '../models/custodia.dart';
import '../services/portfolio_repository.dart';

/// CRUD de custódias: criar/listar/editar/excluir. O desktop
/// (`desktop/src-tauri/src/commands/custodia.rs`) não tem edição — isso é
/// uma melhoria só do mobile.
class CustodiasScreen extends StatefulWidget {
  const CustodiasScreen({super.key});

  @override
  State<CustodiasScreen> createState() => _CustodiasScreenState();
}

class _CustodiasScreenState extends State<CustodiasScreen> {
  final _repository = PortfolioRepository();
  final _instituicaoController = TextEditingController();
  final _titularController = TextEditingController();

  bool _loading = true;
  bool _saving = false;
  List<Custodia> _custodias = [];

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    final custodias = await _repository.listCustodias();
    setState(() {
      _custodias = custodias;
      _loading = false;
    });
  }

  Future<void> _add() async {
    final instituicao = _instituicaoController.text.trim();
    final titular = _titularController.text.trim();
    if (instituicao.isEmpty || titular.isEmpty) return;

    setState(() => _saving = true);

    await _repository.insertCustodia(
      Custodia(
        instituicao: instituicao,
        titular: titular,
        createdAt: DateTime.now(),
      ),
    );

    _instituicaoController.clear();
    _titularController.clear();
    setState(() => _saving = false);
    await _load();
  }

  Future<void> _edit(Custodia custodia) async {
    final instituicaoController = TextEditingController(
      text: custodia.instituicao,
    );
    final titularController = TextEditingController(text: custodia.titular);
    final l = AppLocalizations.of(context);

    final saved = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(l.custodianEditTitle),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            TextField(
              controller: instituicaoController,
              decoration: InputDecoration(labelText: l.custodianInstitution),
            ),
            const SizedBox(height: 12),
            TextField(
              controller: titularController,
              decoration: InputDecoration(labelText: l.custodianHolder),
            ),
          ],
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: Text(l.actionCancel),
          ),
          FilledButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: Text(l.actionSave),
          ),
        ],
      ),
    );

    final instituicao = instituicaoController.text.trim();
    final titular = titularController.text.trim();
    instituicaoController.dispose();
    titularController.dispose();

    if (saved != true || instituicao.isEmpty || titular.isEmpty) return;

    await _repository.updateCustodia(
      Custodia(
        id: custodia.id,
        instituicao: instituicao,
        titular: titular,
        createdAt: custodia.createdAt,
      ),
    );
    await _load();
  }

  Future<void> _delete(int id) async {
    await _repository.deleteCustodia(id);
    await _load();
  }

  @override
  void dispose() {
    _instituicaoController.dispose();
    _titularController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return Scaffold(
      appBar: AppBar(title: Text(l.portfolioCustodies)),
      body: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Row(
              children: [
                Expanded(
                  child: TextField(
                    controller: _instituicaoController,
                    decoration: InputDecoration(
                      labelText: l.custodianInstitution,
                      hintText: 'XP',
                    ),
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: TextField(
                    controller: _titularController,
                    decoration: InputDecoration(labelText: l.custodianHolder),
                  ),
                ),
              ],
            ),
            const SizedBox(height: 12),
            FilledButton(
              onPressed: _saving ? null : _add,
              child: Text(l.custodianAdd),
            ),
            const SizedBox(height: 24),
            Expanded(
              child: _loading
                  ? const Center(child: CircularProgressIndicator())
                  : _custodias.isEmpty
                  ? Center(child: Text(l.custodianNone))
                  : ListView.builder(
                      itemCount: _custodias.length,
                      itemBuilder: (context, index) {
                        final custodia = _custodias[index];
                        return Card(
                          margin: const EdgeInsets.only(bottom: 8),
                          child: ListTile(
                            title: Text(custodia.instituicao),
                            subtitle: Text(custodia.titular),
                            onTap: () => _edit(custodia),
                            trailing: Row(
                              mainAxisSize: MainAxisSize.min,
                              children: [
                                IconButton(
                                  icon: const Icon(Icons.edit_outlined),
                                  onPressed: () => _edit(custodia),
                                ),
                                IconButton(
                                  icon: const Icon(Icons.delete_outline),
                                  onPressed: () => _delete(custodia.id!),
                                ),
                              ],
                            ),
                          ),
                        );
                      },
                    ),
            ),
          ],
        ),
      ),
    );
  }
}
