import 'package:flutter/material.dart';

import '../domain/crypto_history_view.dart';
import '../domain/crypto_score.dart';
import '../theme.dart';

const _amber = Color(0xFFFBBF24);
const _chartHeight = 160.0;
const _sidePad = 12.0;
const _topPad = 8.0;
const _bottomPad = 8.0;

/// Cartão "Histórico" da aba Cripto: a % de indicadores verdes em cada data, com
/// as réguas dos limiares do veredito (70 e 40). A série chega pronta
/// (`CryptoScoreRepository.scoreHistory`); o cartão só recorta pelo período.
class CryptoHistoryCard extends StatefulWidget {
  final List<ScoreHistoryPoint> history;

  const CryptoHistoryCard({super.key, required this.history});

  @override
  State<CryptoHistoryCard> createState() => _CryptoHistoryCardState();
}

class _CryptoHistoryCardState extends State<CryptoHistoryCard> {
  HistoryPeriod _period = HistoryPeriod.all;
  int? _selected;

  @override
  void didUpdateWidget(CryptoHistoryCard oldWidget) {
    super.didUpdateWidget(oldWidget);
    // Outra série (outra moeda, nova coleta): o índice antigo não vale mais.
    if (!identical(oldWidget.history, widget.history)) _selected = null;
  }

  void _setPeriod(HistoryPeriod period) {
    if (period == _period) return;
    setState(() {
      _period = period;
      _selected = null;
    });
  }

  @override
  Widget build(BuildContext context) {
    final history = widget.history;
    final visible = filterPeriod(history, _period);
    final selected = _selected;
    final chosen = selected != null && selected < visible.length
        ? visible[selected]
        : null;

    return Card(
      key: const Key('history-card'),
      margin: const EdgeInsets.symmetric(vertical: 6),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text('Histórico', style: Theme.of(context).textTheme.titleMedium),
            const SizedBox(height: 8),
            if (visible.isEmpty)
              const Text('Sem histórico ainda.')
            else ...[
              Text(_summary(visible), key: const Key('history-summary')),
              const SizedBox(height: 4),
              Text(
                'Parte dos indicadores verdes em cada data. Uma leitura deixa de '
                'contar quando passa do prazo.',
                style: Theme.of(context).textTheme.bodySmall,
              ),
              if (spansMoreThan30Days(history)) ...[
                const SizedBox(height: 8),
                Wrap(
                  spacing: 8,
                  children: [
                    for (final period in HistoryPeriod.values)
                      ChoiceChip(
                        label: Text(period.label),
                        selected: period == _period,
                        onSelected: (_) => _setPeriod(period),
                      ),
                  ],
                ),
              ],
              const SizedBox(height: 12),
              LayoutBuilder(
                builder: (context, constraints) => GestureDetector(
                  behavior: HitTestBehavior.opaque,
                  onTapDown: (details) => setState(
                    () => _selected = CryptoHistoryPainter.indexAt(
                      details.localPosition.dx,
                      constraints.maxWidth,
                      visible.length,
                    ),
                  ),
                  child: CustomPaint(
                    key: const Key('history-chart'),
                    size: Size(constraints.maxWidth, _chartHeight),
                    painter: CryptoHistoryPainter(
                      percentages: [for (final p in visible) greenPercent(p)],
                      selected: selected,
                      lineColor: anchorPrimary,
                      textColor: Theme.of(context).colorScheme.onSurfaceVariant,
                    ),
                  ),
                ),
              ),
              if (chosen != null) ...[
                const SizedBox(height: 8),
                Text(
                  '${formatHistoryDate(chosen.date)} — ${describeHistoryPoint(chosen)}',
                  key: const Key('history-detail'),
                ),
              ],
            ],
          ],
        ),
      ),
    );
  }

  String _summary(List<ScoreHistoryPoint> visible) {
    final last = visible.last;
    final count = visible.length == 1 ? '1 data' : '${visible.length} datas';
    final greens = '${last.green} ${last.green == 1 ? 'verde' : 'verdes'}';
    return '$count · último: $greens de ${last.total} em '
        '${formatHistoryDate(last.date)}';
  }
}

/// Desenha a linha da % de verdes (0 a 100) com as réguas dos limiares do
/// veredito. Os pontos ficam a espaços iguais, na ordem das datas (como o
/// gráfico do desktop), não proporcionais ao calendário.
class CryptoHistoryPainter extends CustomPainter {
  /// Um valor de 0 a 100 por data, na ordem das datas.
  final List<double> percentages;
  final int? selected;
  final Color lineColor;
  final Color textColor;

  const CryptoHistoryPainter({
    required this.percentages,
    required this.selected,
    required this.lineColor,
    required this.textColor,
  });

  /// Índice do ponto mais próximo de [dx] numa área de largura [width] com
  /// [count] pontos. Toque fora das pontas vale o ponto da ponta.
  static int indexAt(double dx, double width, int count) {
    if (count <= 1) return 0;
    final usable = width - 2 * _sidePad;
    if (usable <= 0) return 0;
    final index = ((dx - _sidePad) / usable * (count - 1)).round();
    return index.clamp(0, count - 1);
  }

  double _x(int index, double width) {
    if (percentages.length <= 1) return width / 2;
    return _sidePad + index * (width - 2 * _sidePad) / (percentages.length - 1);
  }

  double _y(double percent, double height) {
    final usable = height - _topPad - _bottomPad;
    return height - _bottomPad - percent.clamp(0.0, 100.0) / 100.0 * usable;
  }

  @override
  void paint(Canvas canvas, Size size) {
    for (final (line, color) in [
      (historyIntactLine, anchorPrimary),
      (historyNeutralLine, _amber),
    ]) {
      final y = _y(line, size.height);
      final paint = Paint()
        ..color = color.withValues(alpha: 0.6)
        ..strokeWidth = 1;
      _dashed(canvas, y, size.width, paint);
      _label(canvas, '${line.round()}%', Offset(_sidePad, y - 12));
    }

    if (percentages.isEmpty) return;

    final path = Path();
    for (var i = 0; i < percentages.length; i++) {
      final point = Offset(_x(i, size.width), _y(percentages[i], size.height));
      if (i == 0) {
        path.moveTo(point.dx, point.dy);
      } else {
        path.lineTo(point.dx, point.dy);
      }
    }
    canvas.drawPath(
      path,
      Paint()
        ..color = lineColor
        ..style = PaintingStyle.stroke
        ..strokeWidth = 2,
    );

    final dot = Paint()..color = lineColor;
    for (var i = 0; i < percentages.length; i++) {
      final center = Offset(_x(i, size.width), _y(percentages[i], size.height));
      canvas.drawCircle(center, i == selected ? 6 : 3, dot);
      if (i == selected) {
        canvas.drawCircle(
          center,
          9,
          Paint()
            ..color = lineColor
            ..style = PaintingStyle.stroke
            ..strokeWidth = 1.5,
        );
      }
    }
  }

  void _dashed(Canvas canvas, double y, double width, Paint paint) {
    const dash = 6.0;
    const gap = 4.0;
    for (var x = 0.0; x < width; x += dash + gap) {
      canvas.drawLine(Offset(x, y), Offset((x + dash).clamp(0, width), y), paint);
    }
  }

  void _label(Canvas canvas, String text, Offset offset) {
    final painter = TextPainter(
      text: TextSpan(text: text, style: TextStyle(color: textColor, fontSize: 11)),
      textDirection: TextDirection.ltr,
    )..layout();
    painter.paint(canvas, offset);
  }

  @override
  bool shouldRepaint(CryptoHistoryPainter old) =>
      old.selected != selected ||
      old.lineColor != lineColor ||
      old.textColor != textColor ||
      !_sameValues(old.percentages, percentages);

  static bool _sameValues(List<double> a, List<double> b) {
    if (a.length != b.length) return false;
    for (var i = 0; i < a.length; i++) {
      if (a[i] != b[i]) return false;
    }
    return true;
  }
}
