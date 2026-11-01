import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/crypto_history_view.dart';
import 'package:anchor_mobile/domain/crypto_score.dart';

// As regras do gráfico do histórico, sem tela nem banco. Espelham
// `desktop/src/stock-lookup/CryptoScoreHistoryChart.tsx`.
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
  group('greenPercent', () {
    test('é a fração de verdes sobre o total do conjunto', () {
      expect(greenPercent(point('2026-10-01', green: 7, total: 10)), 70.0);
      expect(greenPercent(point('2026-10-01', green: 4, total: 10)), 40.0);
      expect(greenPercent(point('2026-10-01', green: 1, total: 3)), closeTo(33.33, 0.01));
    });

    test('é sobre o total, não sobre o registrado', () {
      expect(
        greenPercent(point('2026-10-01', green: 2, logged: 2, total: 10)),
        20.0,
      );
    });

    test('total zero dá 0, nunca NaN', () {
      final value = greenPercent(point('2026-10-01', green: 0, logged: 0, total: 0));
      expect(value, 0.0);
      expect(value.isNaN, isFalse);
    });

    test('nenhum verde dá 0 e todos verdes dão 100', () {
      expect(greenPercent(point('2026-10-01', green: 0, total: 10)), 0.0);
      expect(greenPercent(point('2026-10-01', green: 10, total: 10)), 100.0);
    });
  });

  group('as réguas do gráfico', () {
    test('são os limiares do veredito', () {
      expect(historyIntactLine, 70.0);
      expect(historyNeutralLine, 40.0);
    });
  });

  group('filterPeriod', () {
    // Último ponto em 2026-10-29. 2026-09-29 está a 30 dias; 2026-07-31 a 90;
    // 2025-10-29 a 365.
    final series = [
      point('2025-10-28'), // 366 dias
      point('2025-10-29'), // 365
      point('2026-07-30'), // 91
      point('2026-07-31'), // 90
      point('2026-09-28'), // 31
      point('2026-09-29'), // 30
      point('2026-10-29'), // 0
    ];

    List<String> dates(HistoryPeriod period) => [
      for (final p in filterPeriod(series, period)) p.date,
    ];

    test('"Tudo" devolve a série inteira', () {
      expect(dates(HistoryPeriod.all), [for (final p in series) p.date]);
    });

    test('30D inclui exatamente 30 dias para trás, contados do último ponto', () {
      expect(dates(HistoryPeriod.days30), ['2026-09-29', '2026-10-29']);
    });

    test('90D inclui exatamente 90 dias para trás', () {
      expect(dates(HistoryPeriod.days90), [
        '2026-07-31',
        '2026-09-28',
        '2026-09-29',
        '2026-10-29',
      ]);
    });

    test('12M inclui exatamente 365 dias para trás', () {
      expect(dates(HistoryPeriod.months12), [
        '2025-10-29',
        '2026-07-30',
        '2026-07-31',
        '2026-09-28',
        '2026-09-29',
        '2026-10-29',
      ]);
    });

    test('conta do último ponto, não de hoje: moeda parada não fica vazia', () {
      // O dado mais novo é de 2020 (muito antes de "hoje"): a janela de 30D ainda
      // pega os pontos próximos dele.
      final old = [
        point('2019-06-01'),
        point('2020-02-10'),
        point('2020-02-20'),
        point('2020-03-01'),
      ];
      expect(
        [for (final p in filterPeriod(old, HistoryPeriod.days30)) p.date],
        ['2020-02-10', '2020-02-20', '2020-03-01'],
      );
    });

    test('janela com menos de 2 pontos devolve a série inteira', () {
      final sparse = [point('2026-01-01'), point('2026-06-01'), point('2026-10-29')];
      expect(
        [for (final p in filterPeriod(sparse, HistoryPeriod.days30)) p.date],
        ['2026-01-01', '2026-06-01', '2026-10-29'],
      );
    });

    test('lista vazia e ponto único', () {
      expect(filterPeriod(const [], HistoryPeriod.days30), isEmpty);
      final one = [point('2026-10-29')];
      expect(filterPeriod(one, HistoryPeriod.days30), one);
    });

    test('não altera a lista recebida', () {
      final copy = [...series];
      filterPeriod(series, HistoryPeriod.days30);
      expect([for (final p in series) p.date], [for (final p in copy) p.date]);
    });
  });

  group('spansMoreThan30Days (quando mostrar os botões de período)', () {
    test('só quando o primeiro e o último ponto distam mais de 30 dias', () {
      expect(
        spansMoreThan30Days([point('2026-09-29'), point('2026-10-29')]),
        isFalse,
        reason: '30 dias exatos não passa',
      );
      expect(
        spansMoreThan30Days([point('2026-09-28'), point('2026-10-29')]),
        isTrue,
      );
    });

    test('vazio e ponto único não têm o que escolher', () {
      expect(spansMoreThan30Days(const []), isFalse);
      expect(spansMoreThan30Days([point('2026-10-29')]), isFalse);
    });
  });

  // A frase do ponto tocado ("14 verdes de 20 · 18 registrados · Tese intacta") saiu do
  // domínio (`describeHistoryPoint`): plural e veredito são do `.arb` e são cobertos em
  // `crypto_labels_test.dart` nos dois idiomas.

  group('formatHistoryDate', () {
    test('pt: dia/mês/ano de 2 dígitos', () {
      expect(formatHistoryDate('2026-10-29', locale: 'pt'), '29/10/26');
      expect(formatHistoryDate('2026-01-05', locale: 'pt'), '05/01/26');
    });

    test('en: mês/dia/ano de 2 dígitos', () {
      expect(formatHistoryDate('2026-10-29', locale: 'en'), '10/29/26');
      expect(formatHistoryDate('2026-01-05', locale: 'en'), '01/05/26');
    });

    test('data ilegível volta como veio, em qualquer idioma', () {
      for (final locale in ['pt', 'en']) {
        expect(formatHistoryDate('hoje', locale: locale), 'hoje', reason: locale);
        expect(formatHistoryDate('26-10-29', locale: locale), '26-10-29', reason: locale);
        expect(formatHistoryDate('', locale: locale), '', reason: locale);
      }
    });
  });
}
