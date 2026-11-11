import 'package:material_ui/material_ui.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/crypto_score.dart';
import 'package:anchor_mobile/screens/crypto_history_card.dart';

import 'support/localized_app.dart';

// O cartão "Histórico" isolado: a série entra pronta, sem banco. A integração com
// a tela (carregar do banco ao trocar de moeda) fica em crypto_score_screen_test.
ScoreHistoryPoint point(
  String date, {
  int green = 5,
  int logged = 10,
  int total = 10,
  ScoreVerdict verdict = ScoreVerdict.neutral,
}) => ScoreHistoryPoint(
  date: date,
  green: green,
  logged: logged,
  total: total,
  verdict: verdict,
);

void main() {
  // Idioma fixado em pt: as asserções abaixo são o texto que o app já mostrava.
  Widget app(List<ScoreHistoryPoint> history, {String locale = 'pt'}) => localizedApp(
    Scaffold(
      body: SingleChildScrollView(child: CryptoHistoryCard(history: history)),
    ),
    locale: locale,
  );

  /// Série curta (menos de 30 dias): sem botões de período.
  final shortSeries = [
    point('2026-10-20', green: 2, verdict: ScoreVerdict.reduceRisk),
    point('2026-10-25', green: 5, verdict: ScoreVerdict.neutral),
    point('2026-10-29', green: 8, logged: 10, verdict: ScoreVerdict.intact),
  ];

  /// Série longa: 4 pontos em 4 meses.
  final longSeries = [
    point('2026-06-29', green: 2),
    point('2026-08-29', green: 4),
    point('2026-10-01', green: 6),
    point('2026-10-29', green: 8, verdict: ScoreVerdict.intact),
  ];

  /// O `Text` com a chave [key] cujo conteúdo contém [part]. A chave está no
  /// próprio `Text`, então `find.descendant` (que só olha os filhos) não serve.
  Finder keyedText(String key, String part) => find.byWidgetPredicate(
    (w) => w is Text && w.key == Key(key) && (w.data ?? '').contains(part),
  );

  /// Os pontos que o gráfico desenha, lidos do painter (a asserção é sobre dado,
  /// não sobre pixel).
  List<double> plotted(WidgetTester tester) {
    final paint = tester.widget<CustomPaint>(find.byKey(const Key('history-chart')));
    return (paint.painter! as CryptoHistoryPainter).percentages;
  }

  group('o cartão', () {
    testWidgets('aparece com título, resumo e o gráfico', (tester) async {
      await tester.pumpWidget(app(shortSeries));

      expect(find.byKey(const Key('history-card')), findsOneWidget);
      expect(find.text('Histórico'), findsOneWidget);
      expect(find.byKey(const Key('history-chart')), findsOneWidget);
      expect(keyedText('history-summary', '3 datas'), findsOneWidget);
      expect(
        keyedText('history-summary', '8 verdes de 10'),
        findsOneWidget,
        reason: 'o último ponto',
      );
    });

    testWidgets('em inglês: título, resumo e botão "All"', (tester) async {
      await tester.pumpWidget(app(longSeries, locale: 'en'));

      expect(find.text('History'), findsOneWidget);
      expect(keyedText('history-summary', 'dates'), findsOneWidget);
      expect(keyedText('history-summary', 'latest:'), findsOneWidget);
      expect(find.widgetWithText(ChoiceChip, 'All'), findsOneWidget);
      expect(find.text('Tudo'), findsNothing);
      expect(find.text('Histórico'), findsNothing);
    });

    testWidgets('em inglês: sem histórico mostra o aviso traduzido', (tester) async {
      await tester.pumpWidget(app(const [], locale: 'en'));

      expect(find.text('No history yet.'), findsOneWidget);
      expect(find.text('Sem histórico ainda.'), findsNothing);
    });

    testWidgets('em inglês: um ponto só diz "1 date", sem plural', (tester) async {
      await tester.pumpWidget(app([point('2026-10-29', green: 4)], locale: 'en'));

      expect(keyedText('history-summary', '1 date ·'), findsOneWidget);
      expect(keyedText('history-summary', '4 greens of 10 on 10/29/26'), findsOneWidget);
    });

    testWidgets('plota a % de verdes de cada ponto, na ordem das datas', (tester) async {
      await tester.pumpWidget(app(shortSeries));

      expect(plotted(tester), [20.0, 50.0, 80.0]);
    });

    testWidgets('sem histórico mostra o aviso e nenhum gráfico', (tester) async {
      await tester.pumpWidget(app(const []));

      expect(find.byKey(const Key('history-card')), findsOneWidget);
      expect(find.text('Sem histórico ainda.'), findsOneWidget);
      expect(find.byKey(const Key('history-chart')), findsNothing);
      expect(find.byType(ChoiceChip), findsNothing);
    });

    testWidgets('um ponto só: resumo, mas sem linha nem botões de período', (tester) async {
      await tester.pumpWidget(app([point('2026-10-29', green: 4)]));

      expect(find.textContaining('1 data'), findsOneWidget);
      expect(find.textContaining('1 datas'), findsNothing);
      expect(find.byType(ChoiceChip), findsNothing);
      expect(plotted(tester), [40.0]);
    });

    testWidgets('total zero não gera NaN no desenho', (tester) async {
      await tester.pumpWidget(
        app([
          point('2026-10-28', green: 0, logged: 0, total: 0),
          point('2026-10-29', green: 0, logged: 0, total: 0),
        ]),
      );

      expect(plotted(tester), [0.0, 0.0]);
      expect(plotted(tester).any((v) => v.isNaN), isFalse);
    });
  });

  group('períodos', () {
    testWidgets('série de até 30 dias não mostra os botões', (tester) async {
      await tester.pumpWidget(app(shortSeries));

      expect(find.byType(ChoiceChip), findsNothing);
    });

    testWidgets('série mais longa mostra 30D, 90D, 12M e Tudo, com Tudo marcado', (tester) async {
      await tester.pumpWidget(app(longSeries));

      for (final label in ['30D', '90D', '12M', 'Tudo']) {
        expect(find.widgetWithText(ChoiceChip, label), findsOneWidget, reason: label);
      }
      expect(
        tester.widget<ChoiceChip>(find.widgetWithText(ChoiceChip, 'Tudo')).selected,
        isTrue,
      );
      expect(plotted(tester), [20.0, 40.0, 60.0, 80.0]);
    });

    testWidgets('trocar o período muda os pontos desenhados', (tester) async {
      await tester.pumpWidget(app(longSeries));

      // 90D a partir de 2026-10-29: 2026-07-31 em diante, ou seja, 08-29, 10-01, 10-29.
      await tester.tap(find.widgetWithText(ChoiceChip, '90D'));
      await tester.pump();
      expect(plotted(tester), [40.0, 60.0, 80.0]);
      expect(
        tester.widget<ChoiceChip>(find.widgetWithText(ChoiceChip, '90D')).selected,
        isTrue,
      );

      // 30D tem só 2026-10-01 (28 dias) e 10-29: 2 pontos, então corta.
      await tester.tap(find.widgetWithText(ChoiceChip, '30D'));
      await tester.pump();
      expect(plotted(tester), [60.0, 80.0]);

      await tester.tap(find.widgetWithText(ChoiceChip, 'Tudo'));
      await tester.pump();
      expect(plotted(tester), [20.0, 40.0, 60.0, 80.0]);
    });

    testWidgets('o resumo acompanha o período', (tester) async {
      await tester.pumpWidget(app(longSeries));
      expect(find.textContaining('4 datas'), findsOneWidget);

      await tester.tap(find.widgetWithText(ChoiceChip, '30D'));
      await tester.pump();
      expect(find.textContaining('2 datas'), findsOneWidget);
    });
  });

  group('toque no gráfico', () {
    testWidgets('sem toque não há detalhe', (tester) async {
      await tester.pumpWidget(app(shortSeries));

      expect(find.byKey(const Key('history-detail')), findsNothing);
    });

    testWidgets('tocar na ponta esquerda mostra o primeiro ponto, na direita o último', (tester) async {
      await tester.pumpWidget(app(shortSeries));
      final rect = tester.getRect(find.byKey(const Key('history-chart')));

      await tester.tapAt(Offset(rect.left + 1, rect.center.dy));
      await tester.pump();
      expect(keyedText('history-detail', '20/10/26'), findsOneWidget);
      expect(
        keyedText('history-detail', '2 verdes de 10 · 10 registrados · Reduzir risco'),
        findsOneWidget,
      );

      await tester.tapAt(Offset(rect.right - 1, rect.center.dy));
      await tester.pump();
      expect(keyedText('history-detail', '29/10/26'), findsOneWidget);
      expect(keyedText('history-detail', 'Tese intacta'), findsOneWidget);
    });

    testWidgets('trocar o período limpa o ponto selecionado', (tester) async {
      await tester.pumpWidget(app(longSeries));
      final rect = tester.getRect(find.byKey(const Key('history-chart')));
      await tester.tapAt(Offset(rect.left + 1, rect.center.dy));
      await tester.pump();
      expect(find.byKey(const Key('history-detail')), findsOneWidget);

      await tester.tap(find.widgetWithText(ChoiceChip, '90D'));
      await tester.pump();

      expect(find.byKey(const Key('history-detail')), findsNothing);
    });
  });
}
