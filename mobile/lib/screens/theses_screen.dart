import 'package:material_ui/material_ui.dart';

import '../domain/general_finance.dart' show toIsoDay;
import '../l10n/app_localizations.dart';
import '../models/thesis.dart';
import '../services/thesis_repository.dart';
import 'thesis_edit_screen.dart';

/// Lista de teses de investimento (Fase 10.5 do desktop): título, ticker do ativo ou
/// "Global", e a data da última atualização. Tocar abre a edição; o botão cria uma nova.
class ThesesScreen extends StatefulWidget {
  /// Repositório; o padrão usa o banco do app. Injetável para teste (relógio fixo).
  final ThesisRepository? repository;

  const ThesesScreen({super.key, this.repository});

  @override
  State<ThesesScreen> createState() => _ThesesScreenState();
}

class _ThesesScreenState extends State<ThesesScreen> {
  late final ThesisRepository _repository = widget.repository ?? ThesisRepository();

  bool _loading = true;
  List<ThesisEntry> _entries = [];

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    final entries = await _repository.list();
    if (!mounted) return;
    setState(() {
      _entries = entries;
      _loading = false;
    });
  }

  Future<void> _open({ThesisEntry? entry}) async {
    final changed = await Navigator.of(context).push<bool>(
      MaterialPageRoute(builder: (_) => ThesisEditScreen(repository: _repository, entry: entry)),
    );
    if (changed == true) await _load();
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return Scaffold(
      appBar: AppBar(title: Text(l.thesesTitle)),
      floatingActionButton: FloatingActionButton.extended(
        key: const Key('thesis-new-button'),
        onPressed: () => _open(),
        icon: const Icon(Icons.add),
        label: Text(l.thesesNew),
      ),
      body: _loading
          ? const Center(child: CircularProgressIndicator())
          : _entries.isEmpty
          ? Center(child: Text(l.thesesEmpty))
          : ListView(
              padding: const EdgeInsets.fromLTRB(16, 16, 16, 88),
              children: [
                for (final entry in _entries)
                  Card(
                    child: ListTile(
                      key: Key('thesis-tile-${entry.thesis.id}'),
                      title: Text(entry.thesis.title),
                      subtitle: Text(
                        l.thesesSubtitle(entry.asset?.ticker ?? l.thesesGlobal, toIsoDay(entry.thesis.updatedAt)),
                      ),
                      onTap: () => _open(entry: entry),
                    ),
                  ),
              ],
            ),
    );
  }
}
