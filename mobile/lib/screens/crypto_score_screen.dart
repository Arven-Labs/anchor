import 'package:flutter/material.dart';

import '../domain/crypto_indicator_info.dart';
import '../domain/crypto_score.dart';
import '../domain/general_finance.dart' show formatReadingDate;
import '../l10n/app_localizations.dart';
import '../l10n/crypto_labels.dart';
import '../services/crypto_collector.dart';
import '../services/crypto_score_repository.dart';
import '../services/crypto_source_http.dart';
import '../theme.dart';
import 'crypto_history_card.dart';
import 'crypto_thresholds_screen.dart';

const _coins = [coinEth, coinBtc, coinAda, coinXrp, coinBnb];

const _amber = Color(0xFFFBBF24);
const _red = Color(0xFFF87171);

/// Uma falha da última coleta. [unexpected] é um bug de código (não uma fonte fora do ar):
/// a tela escreve "Erro inesperado: …" no idioma do aparelho.
typedef _Failure = ({String text, bool unexpected});

/// A aba "Cripto": o score de cada moeda (veredito, grupos e indicadores), lido do
/// banco, com a coleta das fontes públicas.
///
/// A coleta automática roda **uma vez por moeda por sessão**, só quando a aba está
/// [active] (o `IndexedStack` constrói todas as abas na abertura do app, então o
/// `initState` sozinho coletaria sem o usuário ver a tela). O botão "Atualizar" é a
/// reserva manual. O coletor já só busca o que falta no dia.
class CryptoScoreScreen extends StatefulWidget {
  /// A aba está à vista.
  final bool active;

  final CryptoScoreRepository? repository;

  /// Coleta uma moeda. O padrão usa o [CryptoCollector]; os testes trocam.
  final Future<CollectionResult> Function(String coin)? collect;

  final DateTime Function()? now;

  const CryptoScoreScreen({
    super.key,
    required this.active,
    this.repository,
    this.collect,
    this.now,
  });

  @override
  State<CryptoScoreScreen> createState() => _CryptoScoreScreenState();
}

class _CryptoScoreScreenState extends State<CryptoScoreScreen> {
  late final CryptoScoreRepository _repository =
      widget.repository ?? CryptoScoreRepository();
  late final DateTime Function() _now = widget.now ?? DateTime.now;

  String _coin = coinEth;
  CryptoScore? _score;
  List<ScoreHistoryPoint> _history = const [];
  bool _collecting = false;
  final _autoCollected = <String>{};

  /// Linhas de erro da última coleta de cada moeda.
  final _errors = <String, List<_Failure>>{};

  @override
  void initState() {
    super.initState();
    _load();
    _autoCollect();
  }

  @override
  void didUpdateWidget(CryptoScoreScreen oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!oldWidget.active && widget.active) _autoCollect();
  }

  Future<void> _load() async {
    final coin = _coin;
    final score = await _repository.score(coin, _now());
    final history = await _repository.scoreHistory(coin);
    if (!mounted || coin != _coin) return;
    setState(() {
      _score = score;
      _history = history;
    });
  }

  /// Abre os limites da moeda selecionada. Ao voltar relê o score e o histórico:
  /// editar um limite reclassifica as leituras gravadas, então os dois mudam.
  Future<void> _openThresholds() async {
    await Navigator.of(context).push<void>(
      MaterialPageRoute(
        builder: (_) =>
            CryptoThresholdsScreen(coin: _coin, repository: _repository),
      ),
    );
    if (!mounted) return;
    await _load();
  }

  void _select(String coin) {
    if (coin == _coin) return;
    setState(() {
      _coin = coin;
      _score = null;
      _history = const [];
    });
    _load();
    _autoCollect();
  }

  /// Coleta a moeda selecionada se a aba está à vista, se ainda não foi coletada
  /// nesta sessão e se não há outra coleta em andamento (nesse caso a tela tenta
  /// de novo quando ela terminar).
  void _autoCollect() {
    if (!widget.active || _collecting || _autoCollected.contains(_coin)) return;
    _autoCollected.add(_coin);
    _collect();
  }

  Future<void> _collect() async {
    if (_collecting) return;
    final coin = _coin;
    setState(() => _collecting = true);
    List<_Failure> errors;
    try {
      final collect =
          widget.collect ??
          (String c) => CryptoCollector(repository: _repository).collect([c]);
      errors = [for (final line in (await collect(coin)).errors) (text: line, unexpected: false)];
    } on CryptoSourceException catch (e) {
      errors = [(text: e.toString(), unexpected: false)];
    } catch (e) {
      // Bug de código: aparece na tela com o texto, não some em silêncio. Guardado como
      // código e traduzido ao desenhar: esta função roda também a partir do `initState`,
      // onde ainda não dá para ler o idioma.
      errors = [(text: '$e', unexpected: true)];
    }
    if (!mounted) return;
    _errors[coin] = errors;
    // Relê o banco ANTES de liberar o botão: "Atualizar" de volta quer dizer que a
    // coleta e a releitura acabaram, sem consulta pendente.
    await _load();
    if (!mounted) return;
    setState(() => _collecting = false);
    _autoCollect();
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final score = _score;
    final errors = _errors[_coin] ?? const <_Failure>[];
    return Scaffold(
      appBar: AppBar(
        title: Text(l.cryptoScoreTitle),
        actions: [
          IconButton(
            key: const Key('open-thresholds'),
            icon: const Icon(Icons.tune),
            tooltip: l.cryptoThresholdsTooltip,
            onPressed: _openThresholds,
          ),
        ],
      ),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          Wrap(
            spacing: 8,
            children: [
              for (final coin in _coins)
                ChoiceChip(
                  label: Text(coin),
                  selected: coin == _coin,
                  onSelected: (_) => _select(coin),
                ),
            ],
          ),
          const SizedBox(height: 12),
          Align(
            alignment: Alignment.centerRight,
            child: FilledButton.icon(
              onPressed: _collecting ? null : _collect,
              icon: _collecting
                  ? const SizedBox(
                      width: 16,
                      height: 16,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(Icons.refresh),
              label: Text(_collecting ? l.cryptoRefreshing : l.cryptoRefresh),
            ),
          ),
          const SizedBox(height: 12),
          if (score == null)
            const Padding(
              padding: EdgeInsets.only(top: 48),
              child: Center(child: CircularProgressIndicator()),
            )
          else ...[
            _VerdictCard(score: score),
            CryptoHistoryCard(history: _history),
            if (_hasNoReading(score))
              Padding(
                padding: const EdgeInsets.symmetric(vertical: 12, horizontal: 4),
                child: Text(l.cryptoNoReadings),
              ),
            if (errors.isNotEmpty) _FailuresCard(errors: errors),
            for (final group in score.groups) _GroupCard(group: group),
          ],
        ],
      ),
    );
  }

  bool _hasNoReading(CryptoScore score) => score.groups.every(
    (group) => group.indicators.every((i) => i.reading == null),
  );
}

Color _verdictColor(ScoreVerdict verdict) => switch (verdict) {
  ScoreVerdict.intact => anchorPrimary,
  ScoreVerdict.neutral => _amber,
  ScoreVerdict.reduceRisk => _red,
};

class _VerdictCard extends StatelessWidget {
  final CryptoScore score;

  const _VerdictCard({required this.score});

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final color = _verdictColor(score.verdict);
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              verdictText(l, score.verdict),
              style: Theme.of(
                context,
              ).textTheme.titleMedium?.copyWith(color: color),
            ),
            const SizedBox(height: 8),
            Text(greensOfTotalText(l, score.green, score.total)),
            Text(l.cryptoValidReadings(score.logged)),
          ],
        ),
      ),
    );
  }
}

class _FailuresCard extends StatelessWidget {
  final List<_Failure> errors;

  const _FailuresCard({required this.errors});

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
              l.cryptoFailuresTitle,
              style: const TextStyle(color: _red, fontWeight: FontWeight.w600),
            ),
            const SizedBox(height: 8),
            for (final error in errors)
              Text(
                error.unexpected
                    ? l.cryptoUnexpectedError(error.text)
                    : describeCollectionError(
                        error.text,
                        nameOf: (key) => indicatorNameOrNull(l, key),
                      ),
              ),
          ],
        ),
      ),
    );
  }
}

class _GroupCard extends StatelessWidget {
  final GroupScore group;

  const _GroupCard({required this.group});

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return Card(
      key: ValueKey('group-${group.group.name}'),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Expanded(
                  child: Text(
                    groupName(l, group.group),
                    style: Theme.of(context).textTheme.titleSmall,
                  ),
                ),
                Text('${group.green}/${group.total}'),
              ],
            ),
            const Divider(),
            for (final indicator in group.indicators)
              _IndicatorRow(indicator: indicator),
          ],
        ),
      ),
    );
  }
}

class _IndicatorRow extends StatelessWidget {
  final GroupIndicator indicator;

  const _IndicatorRow({required this.indicator});

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final reading = indicator.reading;
    final String status;
    final Color color;
    if (reading == null) {
      status = l.cryptoStatusNoReading;
      color = Colors.grey;
    } else if (indicator.expired) {
      status = l.cryptoStatusExpired;
      color = Colors.grey;
    } else {
      status = signalText(l, reading.signal);
      color = switch (reading.signal) {
        'GREEN' => anchorPrimary,
        'RED' => _red,
        _ => _amber,
      };
    }
    return Padding(
      key: ValueKey('row-${indicator.indicator}'),
      padding: const EdgeInsets.symmetric(vertical: 6),
      child: Row(
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(indicatorName(l, indicator.indicator)),
                if (reading != null)
                  Text(
                    '${reading.source} · '
                    '${formatReadingDate(reading.readingDate, locale: l.localeName)}',
                    style: Theme.of(context).textTheme.bodySmall?.copyWith(
                      color: Colors.white60,
                    ),
                  ),
              ],
            ),
          ),
          Column(
            crossAxisAlignment: CrossAxisAlignment.end,
            children: [
              Text(
                reading == null
                    ? '—'
                    : formatIndicatorValue(
                        indicator.indicator,
                        reading.rawValue,
                        locale: l.localeName,
                      ),
              ),
              Text(
                status,
                style: Theme.of(
                  context,
                ).textTheme.bodySmall?.copyWith(color: color),
              ),
            ],
          ),
        ],
      ),
    );
  }
}
