// Score cripto — porta de `desktop/src-tauri/src/domain/crypto_score.rs`. Só
// funções puras: classificação de um indicador pelos limites, conjunto de
// indicadores por moeda e por grupo, janela de validade, veredito e histórico.
// Nada aqui lê banco, rede nem "hoje": a data de referência (`asOf`) é sempre
// parâmetro, o que deixa o histórico recompor o score de cada dia passado.
//
// A lógica está duplicada nas duas linguagens de propósito (o celular é
// independente do desktop). O que segura a paridade são os testes, portados 1:1
// em `test/crypto_score_test.dart`. Mudou um seed, um conjunto ou uma regra lá,
// mude aqui, e o contrário também.

/// Limites de um indicador. A direção sai deles mesmos, sem flag guardada:
/// `greenBoundary > redBoundary` = "maior é melhor" (ex.: staking yield, verde em
/// 2%, vermelho em 0%); senão "menor é melhor" (ex.: MVRV Z-Score, verde abaixo
/// de 0, vermelho acima de 7). Entre os dois limites a leitura é neutra.
class Threshold {
  final double greenBoundary;
  final double redBoundary;

  const Threshold({required this.greenBoundary, required this.redBoundary});
}

enum Signal {
  green('GREEN'),
  neutral('NEUTRAL'),
  red('RED');

  /// Como o sinal é gravado em `crypto_indicators.signal`.
  final String wire;

  const Signal(this.wire);
}

/// Classifica a leitura pelos limites. Limites iguais não definem direção:
/// lança [ArgumentError] (no Rust é `AppError::EqualThresholds`).
Signal classify(double rawValue, Threshold threshold) {
  final green = threshold.greenBoundary;
  final red = threshold.redBoundary;
  if (green == red) {
    throw ArgumentError('green and red boundaries must differ');
  }

  if (higherIsBetter(green, red)) {
    if (rawValue >= green) return Signal.green;
    if (rawValue <= red) return Signal.red;
    return Signal.neutral;
  }
  if (rawValue <= green) return Signal.green;
  if (rawValue >= red) return Signal.red;
  return Signal.neutral;
}

/// Direção inferida dos limites (mesma regra de [classify]).
bool higherIsBetter(double greenBoundary, double redBoundary) =>
    greenBoundary > redBoundary;

/// Limites válidos: números finitos e diferentes (iguais não definem direção).
/// Lança [ArgumentError] se não forem.
void validateBoundaries(double greenBoundary, double redBoundary) {
  if (!greenBoundary.isFinite || !redBoundary.isFinite) {
    throw ArgumentError('thresholds must be finite numbers');
  }
  if (greenBoundary == redBoundary) {
    throw ArgumentError('green and red boundaries must differ');
  }
}

// ---------------------------------------------------------------------------
// Limites de fábrica. Espelham os seeds das migrations do desktop (que o
// celular vai semear na própria tabela) e servem ao "Reset". Ao mudar um seed,
// mude o desktop também. São por moeda; só ETH, BTC, ADA, XRP e BNB têm conjunto
// próprio, e qualquer outra moeda usa o do ETH.
// ---------------------------------------------------------------------------

const coinEth = 'ETH';
const coinBtc = 'BTC';
const coinAda = 'ADA';
const coinXrp = 'XRP';
const coinBnb = 'BNB';

/// Moeda cujo conjunto de indicadores e limites vale para [coin].
String thresholdCoin(String coin) {
  switch (coin.toUpperCase()) {
    case coinBtc:
      return coinBtc;
    case coinAda:
      return coinAda;
    case coinXrp:
      return coinXrp;
    case coinBnb:
      return coinBnb;
    default:
      return coinEth;
  }
}

/// `(indicador, verde, vermelho)`.
typedef ThresholdSeed = (String, double, double);

const List<ThresholdSeed> ethDefaultThresholds = [
  ('mvrv_z_score', 0.0, 7.0),
  ('puell_multiple', 0.5, 4.0),
  ('staking_yield', 2.0, 0.0),
  ('nvt_ratio', 0.9, 1.3),
  ('net_issuance', 0.0, 2.0),
  ('tvl_trend', 0.0, -10.0),
  ('active_addresses_trend', 0.0, -10.0),
  ('exchange_netflow', 0.0, 0.5),
  ('fees_vs_emission', 0.5, 0.1),
  ('funding_rate_7d', 5.0, 20.0),
  ('open_interest_change_30d', 0.0, 25.0),
  ('long_short_ratio', 2.0, 3.5),
  ('dvol', 55.0, 75.0),
  ('put_call_ratio', 0.7, 0.4),
  ('stablecoin_supply_change_30d', 2.0, 0.0),
  ('exchange_supply_change_30d', -1.0, 1.5),
  ('eth_btc_trend_30d', 3.0, -3.0),
  ('fear_greed_index', 25.0, 75.0),
  ('mayer_multiple', 1.3, 2.0),
  ('btc_dominance', 55.0, 65.0),
];

/// Sem staking, TVL, net issuance, fees vs emission nem ETH/BTC: não têm
/// análogo no BTC. `sopr` e `nupl` (bitcoin-data.com) são só do BTC.
const List<ThresholdSeed> btcDefaultThresholds = [
  ('mvrv_z_score', 0.0, 7.0),
  ('nvt_ratio', 0.9, 1.3),
  ('puell_multiple', 0.5, 4.0),
  ('active_addresses_trend', 0.0, -10.0),
  ('exchange_netflow', -0.02, 0.03),
  ('exchange_supply_change_30d', -1.0, 2.0),
  ('sopr', 0.995, 1.02),
  ('nupl', 0.25, 0.6),
  ('funding_rate_7d', 5.0, 15.0),
  ('open_interest_change_30d', 0.0, 25.0),
  ('long_short_ratio', 1.3, 2.0),
  ('dvol', 40.0, 55.0),
  ('put_call_ratio', 0.7, 0.4),
  ('stablecoin_supply_change_30d', 2.0, 0.0),
  ('fear_greed_index', 25.0, 75.0),
  ('mayer_multiple', 1.3, 2.0),
];

/// Sem exchange netflow nem exchange supply (o CoinMetrics Community não os tem
/// para ADA), sem DVOL nem put/call (a Deribit não lista ADA), sem fundamentos
/// de rede nem dominância do BTC (são do ETH).
const List<ThresholdSeed> adaDefaultThresholds = [
  ('mvrv_z_score', 0.0, 3.5),
  ('nvt_ratio', 0.9, 1.3),
  ('puell_multiple', 0.5, 2.5),
  ('active_addresses_trend', 0.0, -20.0),
  ('funding_rate_7d', 5.0, 10.0),
  ('open_interest_change_30d', 0.0, 25.0),
  ('long_short_ratio', 1.8, 2.6),
  ('stablecoin_supply_change_30d', 2.0, 0.0),
  ('fear_greed_index', 25.0, 75.0),
  ('mayer_multiple', 1.3, 2.0),
];

/// É o conjunto do ADA sem `puell_multiple`: o CoinMetrics Community não tem
/// `IssTotUSD` para XRP, e a Deribit não lista XRP.
const List<ThresholdSeed> xrpDefaultThresholds = [
  ('mvrv_z_score', 0.0, 5.0),
  ('nvt_ratio', 0.9, 1.3),
  ('active_addresses_trend', 0.0, -25.0),
  ('funding_rate_7d', 5.0, 10.0),
  ('open_interest_change_30d', 0.0, 25.0),
  ('long_short_ratio', 2.3, 3.2),
  ('stablecoin_supply_change_30d', 2.0, 0.0),
  ('fear_greed_index', 25.0, 75.0),
  ('mayer_multiple', 1.3, 2.0),
];

/// Score só de mercado: o CoinMetrics Community parou o on-chain do BNB em 2019
/// e não há fonte grátis sem chave. O único sinal da rede é o TVL da BSC.
const List<ThresholdSeed> bnbDefaultThresholds = [
  ('nvt_ratio', 0.9, 1.3),
  ('tvl_trend', 0.0, -10.0),
  ('funding_rate_7d', 5.0, 10.0),
  ('open_interest_change_30d', 0.0, 25.0),
  ('long_short_ratio', 2.2, 3.0),
  ('stablecoin_supply_change_30d', 2.0, 0.0),
  ('fear_greed_index', 25.0, 75.0),
  ('mayer_multiple', 1.3, 2.0),
];

/// Limites de fábrica de uma moeda: `(indicador, verde, vermelho)`.
List<ThresholdSeed> defaultThresholds(String coin) {
  switch (thresholdCoin(coin)) {
    case coinBtc:
      return btcDefaultThresholds;
    case coinAda:
      return adaDefaultThresholds;
    case coinXrp:
      return xrpDefaultThresholds;
    case coinBnb:
      return bnbDefaultThresholds;
    default:
      return ethDefaultThresholds;
  }
}

/// `(verde, vermelho)` de fábrica de um indicador numa moeda, ou `null` se a
/// moeda não tem esse indicador.
(double, double)? defaultThreshold(String coin, String indicator) {
  for (final (name, green, red) in defaultThresholds(coin)) {
    if (name == indicator) return (green, red);
  }
  return null;
}

// ---------------------------------------------------------------------------
// Score por grupo: cada grupo ganha o próprio subscore para um conjunto grande
// de indicadores não diluir o sinal.
// ---------------------------------------------------------------------------

/// A ordem dos valores é a ordem da tela.
enum IndicatorGroup {
  onChain('On-chain'),
  fundamentals('Network fundamentals'),
  derivatives('Derivatives & options'),
  liquidity('Market liquidity'),
  sentiment('Market sentiment');

  final String label;

  const IndicatorGroup(this.label);

  /// Grupos que a moeda tem (o BTC não tem fundamentos de rede), na ordem da tela.
  static List<IndicatorGroup> forCoin(String coin) =>
      values.where((g) => g.indicators(coin).isNotEmpty).toList();

  /// Chaves de `indicator_thresholds`/`crypto_indicators` de cada grupo.
  List<String> indicators(String coin) =>
      _indicatorsByCoin[thresholdCoin(coin)]![this] ?? const [];
}

const _marketDerivatives = [
  'funding_rate_7d',
  'open_interest_change_30d',
  'long_short_ratio',
];

const Map<String, Map<IndicatorGroup, List<String>>> _indicatorsByCoin = {
  coinEth: {
    IndicatorGroup.onChain: [
      'mvrv_z_score',
      'nvt_ratio',
      'puell_multiple',
      'active_addresses_trend',
      'exchange_netflow',
      'exchange_supply_change_30d',
    ],
    IndicatorGroup.fundamentals: [
      'net_issuance',
      'fees_vs_emission',
      'staking_yield',
      'tvl_trend',
    ],
    IndicatorGroup.derivatives: [..._marketDerivatives, 'dvol', 'put_call_ratio'],
    IndicatorGroup.liquidity: [
      'stablecoin_supply_change_30d',
      'eth_btc_trend_30d',
    ],
    IndicatorGroup.sentiment: [
      'fear_greed_index',
      'mayer_multiple',
      'btc_dominance',
    ],
  },
  coinBtc: {
    IndicatorGroup.onChain: [
      'mvrv_z_score',
      'nvt_ratio',
      'puell_multiple',
      'active_addresses_trend',
      'exchange_netflow',
      'exchange_supply_change_30d',
      'sopr',
      'nupl',
    ],
    IndicatorGroup.derivatives: [..._marketDerivatives, 'dvol', 'put_call_ratio'],
    IndicatorGroup.liquidity: ['stablecoin_supply_change_30d'],
    IndicatorGroup.sentiment: ['fear_greed_index', 'mayer_multiple'],
  },
  coinAda: {
    IndicatorGroup.onChain: [
      'mvrv_z_score',
      'nvt_ratio',
      'puell_multiple',
      'active_addresses_trend',
    ],
    IndicatorGroup.derivatives: _marketDerivatives,
    IndicatorGroup.liquidity: ['stablecoin_supply_change_30d'],
    IndicatorGroup.sentiment: ['fear_greed_index', 'mayer_multiple'],
  },
  coinXrp: {
    IndicatorGroup.onChain: [
      'mvrv_z_score',
      'nvt_ratio',
      'active_addresses_trend',
    ],
    IndicatorGroup.derivatives: _marketDerivatives,
    IndicatorGroup.liquidity: ['stablecoin_supply_change_30d'],
    IndicatorGroup.sentiment: ['fear_greed_index', 'mayer_multiple'],
  },
  coinBnb: {
    IndicatorGroup.onChain: ['nvt_ratio'],
    IndicatorGroup.fundamentals: ['tvl_trend'],
    IndicatorGroup.derivatives: _marketDerivatives,
    IndicatorGroup.liquidity: ['stablecoin_supply_change_30d'],
    IndicatorGroup.sentiment: ['fear_greed_index', 'mayer_multiple'],
  },
};

/// Última leitura de um indicador (a que vale para o score).
class LatestReading {
  final String indicator;
  final double rawValue;

  /// `GREEN`, `NEUTRAL` ou `RED` (ver [Signal.wire]).
  final String signal;

  /// `YYYY-MM-DD`: a data do dado, não a da coleta.
  final String readingDate;
  final String source;

  const LatestReading({
    required this.indicator,
    required this.rawValue,
    required this.signal,
    required this.readingDate,
    required this.source,
  });
}

class GroupIndicator {
  final String indicator;

  /// `null` = nunca coletado/registrado.
  final LatestReading? reading;

  /// A leitura existe mas passou do prazo ([readingMaxAgeDays]): continua
  /// visível, só que não conta como verde nem como registrada.
  final bool expired;

  const GroupIndicator({
    required this.indicator,
    required this.reading,
    required this.expired,
  });
}

/// Prazo de validade da leitura, em dias, contado da `readingDate` (a data do
/// dado, não a da coleta). Mercado e sentimento mudam todo dia; on-chain e
/// tendências de 30 dias andam mais devagar; SOPR e NUPL já chegam com 7 dias
/// de atraso da fonte grátis. Indicador desconhecido cai no prazo do on-chain.
int readingMaxAgeDays(String indicator) {
  switch (indicator) {
    case 'funding_rate_7d':
    case 'open_interest_change_30d':
    case 'long_short_ratio':
    case 'dvol':
    case 'put_call_ratio':
    case 'fear_greed_index':
    case 'btc_dominance':
    case 'mayer_multiple':
    case 'nvt_ratio':
    case 'staking_yield':
    case 'stablecoin_supply_change_30d':
      return 7;
    case 'sopr':
    case 'nupl':
      return 21;
    default:
      return 14;
  }
}

final _isoDate = RegExp(r'^\d{4}-\d{2}-\d{2}$');

/// `YYYY-MM-DD` de um dia que existe, ou `null`. Mais estrito que
/// `DateTime.tryParse`, que aceita hora junto e deixa "2026-02-30" rolar para
/// março; o `chrono` do desktop recusa os dois.
DateTime? _parseDate(String text) {
  if (!_isoDate.hasMatch(text)) return null;
  final year = int.parse(text.substring(0, 4));
  final month = int.parse(text.substring(5, 7));
  final day = int.parse(text.substring(8, 10));
  final date = DateTime.utc(year, month, day);
  if (date.year != year || date.month != month || date.day != day) return null;
  return date;
}

DateTime _dateOnly(DateTime date) => DateTime.utc(date.year, date.month, date.day);

/// A leitura passou do prazo na data [asOf]. Data ilegível conta como vencida;
/// leitura de dia futuro nunca vence. A hora de [asOf] é ignorada.
bool isExpired(String indicator, String readingDate, DateTime asOf) {
  final date = _parseDate(readingDate);
  if (date == null) return true;
  return _dateOnly(asOf).difference(date).inDays > readingMaxAgeDays(indicator);
}

class GroupScore {
  final IndicatorGroup group;
  final String label;
  final int green;
  final int logged;
  final int total;
  final List<GroupIndicator> indicators;

  const GroupScore({
    required this.group,
    required this.label,
    required this.green,
    required this.logged,
    required this.total,
    required this.indicators,
  });
}

/// Leitura sugerida do score total (faixas equivalentes às originais de
/// 9 indicadores: 7-9 verdes, 4-6, 0-3).
enum ScoreVerdict { intact, neutral, reduceRisk }

class CryptoScore {
  final List<GroupScore> groups;
  final int green;
  final int logged;
  final int total;
  final ScoreVerdict verdict;

  const CryptoScore({
    required this.groups,
    required this.green,
    required this.logged,
    required this.total,
    required this.verdict,
  });
}

const _verdictIntactMinPct = 70.0;
const _verdictNeutralMinPct = 40.0;

/// Veredito pela fração de verdes sobre o **total** do conjunto da moeda (não
/// sobre o registrado, para um score com poucas leituras não parecer melhor).
ScoreVerdict verdictFor(int green, int total) {
  if (total == 0) return ScoreVerdict.reduceRisk;
  final pct = green / total * 100.0;
  if (pct >= _verdictIntactMinPct) return ScoreVerdict.intact;
  if (pct >= _verdictNeutralMinPct) return ScoreVerdict.neutral;
  return ScoreVerdict.reduceRisk;
}

/// Monta o score a partir da última leitura de cada indicador, vista na data
/// [asOf]. Indicador sem leitura, ou com leitura vencida, conta no `total` mas
/// não como verde nem como registrado, e leitura de indicador fora dos grupos
/// é ignorada.
CryptoScore computeScore(
  String coin,
  List<LatestReading> latest,
  DateTime asOf,
) {
  final groups = <GroupScore>[];
  var green = 0;
  var logged = 0;
  var total = 0;

  for (final group in IndicatorGroup.forCoin(coin)) {
    final indicators = group.indicators(coin).map((name) {
      final reading = latest.where((r) => r.indicator == name).firstOrNull;
      final expired = reading != null && isExpired(name, reading.readingDate, asOf);
      return GroupIndicator(indicator: name, reading: reading, expired: expired);
    }).toList();

    final groupGreen = indicators
        .where((i) => !i.expired && i.reading?.signal == Signal.green.wire)
        .length;
    final groupLogged = indicators
        .where((i) => i.reading != null && !i.expired)
        .length;

    green += groupGreen;
    logged += groupLogged;
    total += indicators.length;
    groups.add(
      GroupScore(
        group: group,
        label: group.label,
        green: groupGreen,
        logged: groupLogged,
        total: indicators.length,
        indicators: indicators,
      ),
    );
  }

  return CryptoScore(
    groups: groups,
    green: green,
    logged: logged,
    total: total,
    verdict: verdictFor(green, total),
  );
}

/// O score num dia: o que [computeScore] daria com a última leitura de cada
/// indicador até aquela `readingDate`.
class ScoreHistoryPoint {
  /// `readingDate` (`YYYY-MM-DD`), não a data da coleta.
  final String date;
  final int green;
  final int logged;
  final int total;
  final ScoreVerdict verdict;

  const ScoreHistoryPoint({
    required this.date,
    required this.green,
    required this.logged,
    required this.total,
    required this.verdict,
  });
}

/// Equivalente ao `NaiveDate::MIN` do Rust: com ela nada vence.
final _beforeAnyReading = DateTime.utc(1);

/// Evolução do score: um ponto por `readingDate` distinta, na ordem das datas.
/// [ascending] são as leituras da moeda **já ordenadas** por `readingDate`,
/// `created_at` e `id` crescentes, de modo que a leitura mais nova de um
/// indicador sempre substitui a anterior.
///
/// Usa o `signal` gravado, que já reflete os limites atuais (editar um limite
/// reclassifica as leituras guardadas): o passado é visto com a régua de hoje.
/// Cada ponto respeita a janela de validade vista da própria data: um indicador
/// registrado vale nos dias seguintes só até [readingMaxAgeDays], depois deixa
/// de contar até ser registrado de novo.
List<ScoreHistoryPoint> scoreHistory(
  String coin,
  List<LatestReading> ascending,
) {
  final latest = <LatestReading>[];
  final points = <ScoreHistoryPoint>[];
  for (var i = 0; i < ascending.length; i++) {
    final reading = ascending[i];
    final slot = latest.indexWhere((l) => l.indicator == reading.indicator);
    if (slot == -1) {
      latest.add(reading);
    } else {
      latest[slot] = reading;
    }

    final dayEnds =
        i + 1 >= ascending.length ||
        ascending[i + 1].readingDate != reading.readingDate;
    if (dayEnds) {
      // `readingDate` ilegível não chega aqui na prática (a coleta grava
      // `YYYY-MM-DD`); se chegar, o ponto é visto sem janela em vez de sumir.
      final asOf = _parseDate(reading.readingDate) ?? _beforeAnyReading;
      final score = computeScore(coin, latest, asOf);
      points.add(
        ScoreHistoryPoint(
          date: reading.readingDate,
          green: score.green,
          logged: score.logged,
          total: score.total,
          verdict: score.verdict,
        ),
      );
    }
  }
  return points;
}
