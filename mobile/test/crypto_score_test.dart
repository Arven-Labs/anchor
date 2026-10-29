// Porta 1:1 dos testes de `desktop/src-tauri/src/domain/crypto_score.rs`. As
// mesmas entradas e as mesmas saídas esperadas: é isso que mantém o score do
// celular igual ao do desktop. Mudou um teste lá, mude aqui.
import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/crypto_score.dart';

LatestReading reading(String indicator, String signal) => LatestReading(
  indicator: indicator,
  rawValue: 1.0,
  signal: signal,
  readingDate: '2026-10-16',
  source: 'test',
);

LatestReading dated(String date, String indicator, String signal) {
  final base = reading(indicator, signal);
  return LatestReading(
    indicator: base.indicator,
    rawValue: base.rawValue,
    signal: base.signal,
    readingDate: date,
    source: base.source,
  );
}

/// A data de `reading()`: nos testes de score, as leituras são do próprio dia.
final DateTime today = DateTime(2026, 10, 16);

List<String> grouped(String coin) =>
    IndicatorGroup.values.expand((g) => g.indicators(coin)).toList();

List<(String, int, int, int)> shapeWithTotal(List<ScoreHistoryPoint> points) =>
    points.map((p) => (p.date, p.green, p.logged, p.total)).toList();

List<(String, int, int)> shape(List<ScoreHistoryPoint> points) =>
    points.map((p) => (p.date, p.green, p.logged)).toList();

String signalOf(double value, double green, double red) =>
    classify(value, Threshold(greenBoundary: green, redBoundary: red)).wire;

void main() {
  test('groups cover distinct indicators per coin', () {
    for (final (coin, expected) in [
      ('ETH', 20),
      ('BTC', 16),
      ('ADA', 10),
      ('XRP', 9),
      ('BNB', 8),
    ]) {
      final all = grouped(coin);
      expect(all.length, expected, reason: coin);
      expect(all.toSet().length, expected, reason: coin);
    }
  });

  test('btc drops the fundamentals group and the eth-only indicators', () {
    expect(IndicatorGroup.forCoin('BTC'), [
      IndicatorGroup.onChain,
      IndicatorGroup.derivatives,
      IndicatorGroup.liquidity,
      IndicatorGroup.sentiment,
    ]);
    final btc = grouped('BTC');
    for (final ethOnly in [
      'staking_yield',
      'tvl_trend',
      'net_issuance',
      'fees_vs_emission',
      'eth_btc_trend_30d',
      'btc_dominance',
    ]) {
      expect(btc.contains(ethOnly), isFalse, reason: ethOnly);
    }
    expect(IndicatorGroup.forCoin('ETH').length, 5);
  });

  // O ADA é o BTC sem o que o CoinMetrics Community e a Deribit não servem
  // para ele: netflow e supply das exchanges, DVOL e put/call.
  test('ada is the btc set minus what its sources do not serve', () {
    expect(IndicatorGroup.forCoin('ADA'), [
      IndicatorGroup.onChain,
      IndicatorGroup.derivatives,
      IndicatorGroup.liquidity,
      IndicatorGroup.sentiment,
    ]);
    final btc = grouped('BTC');
    final ada = grouped('ADA');
    expect(ada.every(btc.contains), isTrue);
    final dropped = btc.where((n) => !ada.contains(n)).toList();
    // `sopr` e `nupl` vêm do bitcoin-data.com, que só atende o BTC.
    expect(dropped, [
      'exchange_netflow',
      'exchange_supply_change_30d',
      'sopr',
      'nupl',
      'dvol',
      'put_call_ratio',
    ]);
    expect(thresholdCoin('ada'), 'ADA');
  });

  // O XRP é o ADA sem o Puell: o CoinMetrics Community não tem `IssTotUSD` para ele.
  test('xrp is the ada set minus puell', () {
    expect(IndicatorGroup.forCoin('XRP'), IndicatorGroup.forCoin('ADA'));
    final ada = grouped('ADA');
    final xrp = grouped('XRP');
    expect(xrp.every(ada.contains), isTrue);
    expect(ada.where((n) => !xrp.contains(n)).toList(), ['puell_multiple']);
    expect(thresholdCoin('xrp'), 'XRP');
  });

  // O BNB é só de mercado: o CoinMetrics Community parou o on-chain dele em 2019.
  // Só mantém o NVT (CoinGecko) e o TVL da BSC (DefiLlama) além dos de mercado.
  test('bnb is a market-only set with nvt and bsc tvl', () {
    expect(IndicatorGroup.forCoin('BNB').length, 5);
    expect(IndicatorGroup.onChain.indicators('BNB'), ['nvt_ratio']);
    expect(IndicatorGroup.fundamentals.indicators('BNB'), ['tvl_trend']);
    final bnb = grouped('BNB');
    for (final missing in [
      'mvrv_z_score',
      'puell_multiple',
      'active_addresses_trend',
      'exchange_netflow',
      'dvol',
    ]) {
      expect(bnb.contains(missing), isFalse, reason: missing);
    }
    // É o conjunto do XRP sem o on-chain que a fonte não serve, mais o TVL da BSC.
    final fromXrp = grouped(
      'XRP',
    ).where((n) => !['mvrv_z_score', 'active_addresses_trend'].contains(n));
    expect(fromXrp.every(bnb.contains), isTrue);
    expect(bnb.contains('tvl_trend'), isTrue);
    expect(thresholdCoin('bnb'), 'BNB');
  });

  test(
    'sentiment group has fear-greed and mayer for both and dominance only for eth',
    () {
      expect(IndicatorGroup.sentiment.indicators('ETH'), [
        'fear_greed_index',
        'mayer_multiple',
        'btc_dominance',
      ]);
      expect(IndicatorGroup.sentiment.indicators('BTC'), [
        'fear_greed_index',
        'mayer_multiple',
      ]);
      expect(IndicatorGroup.sentiment.label, 'Market sentiment');
    },
  );

  // Os 3 seeds de sentimento (lote 4): menor é melhor.
  test('sentiment seeds are lower-is-better', () {
    // Fear & Greed: medo extremo (≤25) é verde, ganância extrema (≥75) é vermelha.
    expect(signalOf(12.0, 25.0, 75.0), 'GREEN');
    expect(signalOf(50.0, 25.0, 75.0), 'NEUTRAL');
    expect(signalOf(88.0, 25.0, 75.0), 'RED');

    // Mayer: hoje BTC 1,10 e ETH 1,08 → verde; 1,6 → neutro; 2,4 → vermelho.
    expect(signalOf(1.1, 1.3, 2.0), 'GREEN');
    expect(signalOf(1.6, 1.3, 2.0), 'NEUTRAL');
    expect(signalOf(2.4, 1.3, 2.0), 'RED');

    // Dominância do BTC (só ETH): hoje 60% → neutro.
    expect(signalOf(60.0, 55.0, 65.0), 'NEUTRAL');
    expect(signalOf(50.0, 55.0, 65.0), 'GREEN');
    expect(signalOf(70.0, 55.0, 65.0), 'RED');
  });

  test('other coins use the eth set and thresholds', () {
    expect(thresholdCoin('SOL'), 'ETH');
    expect(thresholdCoin('btc'), 'BTC');
    expect(grouped('SOL'), grouped('ETH'));
    expect(defaultThreshold('SOL', 'dvol'), defaultThreshold('ETH', 'dvol'));
    expect(
      defaultThreshold('BTC', 'dvol'),
      isNot(defaultThreshold('ETH', 'dvol')),
    );
  });

  test('empty score has full total and nothing logged', () {
    final score = computeScore('ETH', [], today);
    expect((score.green, score.logged, score.total), (0, 0, 20));
    expect(score.verdict, ScoreVerdict.reduceRisk);
    expect(
      score.groups.every((g) => g.indicators.every((i) => i.reading == null)),
      isTrue,
    );

    final btc = computeScore('BTC', [], today);
    expect((btc.green, btc.logged, btc.total), (0, 0, 16));
    expect(btc.groups.length, 4);

    final ada = computeScore('ADA', [], today);
    expect((ada.green, ada.logged, ada.total), (0, 0, 10));
    expect(ada.groups.length, 4);

    final xrp = computeScore('XRP', [], today);
    expect((xrp.green, xrp.logged, xrp.total), (0, 0, 9));
    expect(xrp.groups.length, 4);
  });

  test('subscores count greens per group', () {
    final score = computeScore('ETH', [
      reading('mvrv_z_score', 'GREEN'),
      reading('nvt_ratio', 'RED'),
      reading('funding_rate_7d', 'GREEN'),
      reading('long_short_ratio', 'NEUTRAL'),
    ], today);
    (int, int, int) counts(GroupScore g) => (g.green, g.logged, g.total);
    expect(counts(score.groups[0]), (1, 2, 6)); // on-chain
    expect(counts(score.groups[1]), (0, 0, 4)); // fundamentals
    expect(counts(score.groups[2]), (1, 2, 5)); // derivatives
    expect(counts(score.groups[3]), (0, 0, 2)); // liquidity
    expect(counts(score.groups[4]), (0, 0, 3)); // sentiment
    expect((score.green, score.logged), (2, 4));
  });

  test(
    'history has one point per reading date and carries indicators forward',
    () {
      final points = scoreHistory('ETH', [
        dated('2026-10-01', 'mvrv_z_score', 'GREEN'),
        dated('2026-10-01', 'nvt_ratio', 'RED'),
        dated('2026-10-05', 'funding_rate_7d', 'GREEN'),
        // O MVRV de 10/10 substitui o de 01/10; o NVT de 01/10 (prazo de 7 dias) já
        // venceu em 10/10 (9 dias) e sai da contagem.
        dated('2026-10-10', 'mvrv_z_score', 'RED'),
      ]);
      expect(shapeWithTotal(points), [
        ('2026-10-01', 1, 2, 20),
        ('2026-10-05', 2, 3, 20),
        ('2026-10-10', 1, 2, 20),
      ]);
      expect(points.every((p) => p.verdict == ScoreVerdict.reduceRisk), isTrue);
    },
  );

  test('expiry is per indicator and counts from the reading date', () {
    DateTime on(int day) => DateTime(2026, 10, day);
    // Mercado (7 dias): 8 dias depois vence; 7 ainda vale.
    expect(isExpired('funding_rate_7d', '2026-10-01', on(8)), isFalse);
    expect(isExpired('funding_rate_7d', '2026-10-01', on(9)), isTrue);
    // On-chain (14 dias).
    expect(isExpired('mvrv_z_score', '2026-10-01', on(15)), isFalse);
    expect(isExpired('mvrv_z_score', '2026-10-01', on(16)), isTrue);
    // SOPR e NUPL (21 dias, contando o atraso de 7 da fonte).
    expect(isExpired('sopr', '2026-10-01', on(22)), isFalse);
    expect(isExpired('nupl', '2026-10-01', on(23)), isTrue);
    // Leitura de dia futuro nunca vence, e data ilegível sempre vence.
    expect(isExpired('dvol', '2026-10-20', on(16)), isFalse);
    expect(isExpired('dvol', 'not-a-date', on(16)), isTrue);
  });

  // Só no Dart: `DateTime.tryParse` aceita o que o chrono recusa (hora junto, dia
  // inexistente que "rola" para o mês seguinte). A leitura é gravada como
  // `YYYY-MM-DD`; qualquer outra coisa tem de contar como ilegível.
  test('only a real YYYY-MM-DD date is readable', () {
    final asOf = DateTime(2026, 10, 16);
    expect(isExpired('dvol', '2026-10-16T10:00:00', asOf), isTrue);
    expect(isExpired('dvol', '2026-02-30', asOf), isTrue);
    expect(isExpired('dvol', '2026-13-01', asOf), isTrue);
    expect(isExpired('dvol', '', asOf), isTrue);
    expect(isExpired('dvol', '2026-10-16', asOf), isFalse);
  });

  // Só no Dart: a hora do `asOf` não pode mexer na conta de dias.
  test('the time of day of as-of does not change the day count', () {
    final lateEvening = DateTime(2026, 10, 8, 23, 59);
    expect(isExpired('funding_rate_7d', '2026-10-01', lateEvening), isFalse);
    final earlyMorning = DateTime(2026, 10, 9, 0, 1);
    expect(isExpired('funding_rate_7d', '2026-10-01', earlyMorning), isTrue);
  });

  test('an expired reading stays visible but does not count', () {
    final stale = LatestReading(
      indicator: 'funding_rate_7d',
      rawValue: 3.0,
      signal: 'GREEN',
      readingDate: '2026-10-01', // 15 dias: vencida
      source: 'test',
    );
    final score = computeScore('ETH', [
      stale,
      dated('2026-10-16', 'mvrv_z_score', 'GREEN'),
    ], today);
    expect((score.green, score.logged, score.total), (1, 1, 20));
    final funding = score.groups
        .expand((g) => g.indicators)
        .firstWhere((i) => i.indicator == 'funding_rate_7d');
    expect(funding.expired, isTrue);
    expect(funding.reading?.rawValue, 3.0);
    final derivatives = score.groups[2];
    expect((derivatives.green, derivatives.logged, derivatives.total), (
      0,
      0,
      5,
    ));
  });

  test(
    'history drops a reading once it expires and takes it back when logged again',
    () {
      final points = scoreHistory('ETH', [
        dated('2026-10-01', 'funding_rate_7d', 'GREEN'),
        dated('2026-10-08', 'mvrv_z_score', 'GREEN'), // funding com 7 dias: vale
        dated('2026-10-09', 'mvrv_z_score', 'GREEN'), // funding com 8 dias: venceu
        dated(
          '2026-10-10',
          'funding_rate_7d',
          'GREEN',
        ), // registrado de novo
      ]);
      expect(shape(points), [
        ('2026-10-01', 1, 1),
        ('2026-10-08', 2, 2),
        ('2026-10-09', 1, 1),
        ('2026-10-10', 2, 2),
      ]);
    },
  );

  test(
    'history takes the last reading of the day and total follows the coin',
    () {
      // Duas leituras do mesmo indicador no mesmo dia: vale a última da ordem recebida.
      final points = scoreHistory('BTC', [
        dated('2026-10-01', 'mvrv_z_score', 'RED'),
        dated('2026-10-01', 'mvrv_z_score', 'GREEN'),
      ]);
      expect(points.length, 1);
      expect((points[0].green, points[0].logged, points[0].total), (1, 1, 16));
    },
  );

  test(
    'history places a delayed reading on its own date and ignores unknown indicators',
    () {
      // O SOPR chega com a data do dado (D-7), então aparece no seu dia e não antes;
      // a ordem de entrada já é por `reading_date`, como a consulta faz.
      final points = scoreHistory('BTC', [
        dated('2026-10-12', 'sopr', 'GREEN'),
        dated('2026-10-12', 'made_up', 'GREEN'),
        dated('2026-10-19', 'mvrv_z_score', 'GREEN'),
      ]);
      expect(shape(points), [('2026-10-12', 1, 1), ('2026-10-19', 2, 2)]);
    },
  );

  test(
    'history without readings is empty and verdicts follow the green share',
    () {
      expect(scoreHistory('ETH', []), isEmpty);
      // 15 verdes de 20 = 75%: tese intacta.
      final greens = grouped(
        'ETH',
      ).take(15).map((name) => dated('2026-10-01', name, 'GREEN')).toList();
      final points = scoreHistory('ETH', greens);
      expect(points[0].green, 15);
      expect(points[0].verdict, ScoreVerdict.intact);
    },
  );

  // Só no Dart: no Rust a data ilegível vira `NaiveDate::MIN`; aqui, o equivalente.
  test('history keeps a point whose reading date is unreadable', () {
    final points = scoreHistory('ETH', [
      dated('not-a-date', 'mvrv_z_score', 'GREEN'),
    ]);
    expect(points.length, 1);
    expect(points[0].date, 'not-a-date');
  });

  test('unknown indicators are ignored', () {
    final score = computeScore('ETH', [reading('made_up', 'GREEN')], today);
    expect((score.green, score.logged), (0, 0));
  });

  test('verdict bands match the original nine-indicator ranges', () {
    // Com 15 indicadores: 11/15 = 73% → tese intacta; 10/15 = 67% e
    // 7/15 = 47% → neutro; 5/15 = 33% → reduzir risco.
    expect(verdictFor(11, 15), ScoreVerdict.intact);
    expect(verdictFor(10, 15), ScoreVerdict.neutral);
    expect(verdictFor(7, 15), ScoreVerdict.neutral);
    expect(verdictFor(5, 15), ScoreVerdict.reduceRisk);
    expect(verdictFor(0, 0), ScoreVerdict.reduceRisk);
  });

  test('every grouped indicator has a valid default', () {
    for (final coin in ['ETH', 'BTC', 'ADA', 'XRP', 'BNB']) {
      for (final name in grouped(coin)) {
        final threshold = defaultThreshold(coin, name);
        expect(threshold, isNotNull, reason: 'no default for $coin $name');
        final (green, red) = threshold!;
        expect(() => validateBoundaries(green, red), returnsNormally);
      }
      expect(defaultThreshold(coin, 'made_up'), isNull);
    }
  });

  test('defaults have no unknown indicator', () {
    for (final coin in ['ETH', 'BTC', 'ADA', 'XRP', 'BNB']) {
      final names = grouped(coin);
      expect(
        defaultThresholds(coin).every((t) => names.contains(t.$1)),
        isTrue,
        reason: coin,
      );
      expect(defaultThresholds(coin).length, names.length, reason: coin);
    }
  });

  // Os valores de fábrica espelham as migrations do desktop. Fixar alguns
  // pega um seed copiado errado, coisa que os testes de estrutura não pegam.
  test('factory values match the desktop seeds', () {
    expect(defaultThreshold('ETH', 'mvrv_z_score'), (0.0, 7.0));
    expect(defaultThreshold('ETH', 'staking_yield'), (2.0, 0.0));
    expect(defaultThreshold('ETH', 'dvol'), (55.0, 75.0));
    expect(defaultThreshold('BTC', 'dvol'), (40.0, 55.0));
    expect(defaultThreshold('BTC', 'sopr'), (0.995, 1.02));
    expect(defaultThreshold('BTC', 'nupl'), (0.25, 0.6));
    expect(defaultThreshold('BTC', 'exchange_netflow'), (-0.02, 0.03));
    expect(defaultThreshold('ADA', 'mvrv_z_score'), (0.0, 3.5));
    expect(defaultThreshold('ADA', 'long_short_ratio'), (1.8, 2.6));
    expect(defaultThreshold('XRP', 'mvrv_z_score'), (0.0, 5.0));
    expect(defaultThreshold('XRP', 'long_short_ratio'), (2.3, 3.2));
    expect(defaultThreshold('BNB', 'long_short_ratio'), (2.2, 3.0));
    expect(defaultThreshold('BNB', 'tvl_trend'), (0.0, -10.0));
  });

  test('validate boundaries rejects equal and non-finite', () {
    expect(() => validateBoundaries(1.0, 2.0), returnsNormally);
    expect(() => validateBoundaries(2.0, 1.0), returnsNormally);
    expect(() => validateBoundaries(3.0, 3.0), throwsArgumentError);
    expect(() => validateBoundaries(double.nan, 1.0), throwsArgumentError);
    expect(
      () => validateBoundaries(1.0, double.infinity),
      throwsArgumentError,
    );
  });

  test('direction follows the boundaries', () {
    expect(higherIsBetter(2.0, 0.0), isTrue);
    expect(higherIsBetter(0.0, 7.0), isFalse);
  });

  // Os 3 seeds de derivativos (migration da Fase 31): direção e sinais.
  test('options and liquidity seeds have the documented direction', () {
    // DVOL: menor é melhor. Hoje 51 → verde; 60 → neutro; 80 → vermelho.
    expect(signalOf(51.0, 55.0, 75.0), 'GREEN');
    expect(signalOf(60.0, 55.0, 75.0), 'NEUTRAL');
    expect(signalOf(80.0, 55.0, 75.0), 'RED');

    // Put/call: maior é melhor (medo é oportunidade, calma demais é topo).
    expect(signalOf(0.516, 0.7, 0.4), 'NEUTRAL');
    expect(signalOf(0.8, 0.7, 0.4), 'GREEN');
    expect(signalOf(0.3, 0.7, 0.4), 'RED');

    // Stablecoins: maior é melhor (liquidez entrando).
    expect(signalOf(0.77, 2.0, 0.0), 'NEUTRAL');
    expect(signalOf(3.0, 2.0, 0.0), 'GREEN');
    expect(signalOf(-1.0, 2.0, 0.0), 'RED');
  });

  test('derivatives seeds are lower-is-better', () {
    expect(signalOf(1.6, 5.0, 20.0), 'GREEN');
    expect(signalOf(12.0, 5.0, 20.0), 'NEUTRAL');
    expect(signalOf(35.0, 5.0, 20.0), 'RED');

    expect(signalOf(2.65, 2.0, 3.5), 'NEUTRAL');
    expect(signalOf(3.8, 2.0, 3.5), 'RED');
  });

  // Lower is better (ex.: MVRV Z-Score — green_boundary 0, red_boundary 7).
  test('lower is better: green below the green boundary', () {
    expect(signalOf(-1.0, 0.0, 7.0), 'GREEN');
  });

  test('lower is better: red above the red boundary', () {
    expect(signalOf(8.0, 0.0, 7.0), 'RED');
  });

  test('lower is better: neutral in the gap', () {
    expect(signalOf(3.0, 0.0, 7.0), 'NEUTRAL');
  });

  // Higher is better (ex.: Staking Yield — green_boundary 2, red_boundary 0).
  test('higher is better: green above the green boundary', () {
    expect(signalOf(3.0, 2.0, 0.0), 'GREEN');
  });

  test('higher is better: red below the red boundary', () {
    expect(signalOf(-0.5, 2.0, 0.0), 'RED');
  });

  test('higher is better: neutral in the gap', () {
    expect(signalOf(1.0, 2.0, 0.0), 'NEUTRAL');
  });

  // Nos limites exatos a leitura é do lado do sinal (>= e <=), nos dois sentidos.
  test('values exactly on a boundary take that boundary signal', () {
    expect(signalOf(0.0, 0.0, 7.0), 'GREEN');
    expect(signalOf(7.0, 0.0, 7.0), 'RED');
    expect(signalOf(2.0, 2.0, 0.0), 'GREEN');
    expect(signalOf(0.0, 2.0, 0.0), 'RED');
  });

  test('rejects equal boundaries', () {
    expect(() => signalOf(5.0, 5.0, 5.0), throwsArgumentError);
  });
}
