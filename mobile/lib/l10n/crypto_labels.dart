import '../domain/crypto_history_view.dart';
import '../domain/crypto_score.dart';
import '../domain/crypto_threshold_input.dart';
import 'app_localizations.dart';

// Textos da aba Cripto (ADR 0001). O domínio guarda chaves e códigos; aqui cada um vira o
// texto do idioma da tela. Um `switch` sem `default` nos enums faz o analisador acusar um
// código novo sem texto.

/// Nome do indicador, ou `null` se a chave não é conhecida.
String? indicatorNameOrNull(AppLocalizations l, String key) => switch (key) {
  'mvrv_z_score' => l.indicatorMvrvZScore,
  'nvt_ratio' => l.indicatorNvtRatio,
  'puell_multiple' => l.indicatorPuellMultiple,
  'net_issuance' => l.indicatorNetIssuance,
  'staking_yield' => l.indicatorStakingYield,
  'tvl_trend' => l.indicatorTvlTrend,
  'active_addresses_trend' => l.indicatorActiveAddressesTrend,
  'exchange_netflow' => l.indicatorExchangeNetflow,
  'fees_vs_emission' => l.indicatorFeesVsEmission,
  'funding_rate_7d' => l.indicatorFundingRate7d,
  'open_interest_change_30d' => l.indicatorOpenInterestChange30d,
  'long_short_ratio' => l.indicatorLongShortRatio,
  'dvol' => l.indicatorDvol,
  'put_call_ratio' => l.indicatorPutCallRatio,
  'stablecoin_supply_change_30d' => l.indicatorStablecoinSupplyChange30d,
  'exchange_supply_change_30d' => l.indicatorExchangeSupplyChange30d,
  'eth_btc_trend_30d' => l.indicatorEthBtcTrend30d,
  'fear_greed_index' => l.indicatorFearGreedIndex,
  'mayer_multiple' => l.indicatorMayerMultiple,
  'btc_dominance' => l.indicatorBtcDominance,
  'sopr' => l.indicatorSopr,
  'nupl' => l.indicatorNupl,
  _ => null,
};

/// Indicador desconhecido mostra a chave como veio, em vez de quebrar a tela.
String indicatorName(AppLocalizations l, String key) => indicatorNameOrNull(l, key) ?? key;

String groupName(AppLocalizations l, IndicatorGroup group) => switch (group) {
  IndicatorGroup.onChain => l.groupOnChain,
  IndicatorGroup.fundamentals => l.groupFundamentals,
  IndicatorGroup.derivatives => l.groupDerivatives,
  IndicatorGroup.liquidity => l.groupLiquidity,
  IndicatorGroup.sentiment => l.groupSentiment,
};

String verdictText(AppLocalizations l, ScoreVerdict verdict) => switch (verdict) {
  ScoreVerdict.intact => l.verdictIntact,
  ScoreVerdict.neutral => l.verdictNeutral,
  ScoreVerdict.reduceRisk => l.verdictReduceRisk,
};

String _verdictShort(AppLocalizations l, ScoreVerdict verdict) => switch (verdict) {
  ScoreVerdict.intact => l.verdictShortIntact,
  ScoreVerdict.neutral => l.verdictShortNeutral,
  ScoreVerdict.reduceRisk => l.verdictShortReduceRisk,
};

/// Texto do sinal gravado (`GREEN`, `NEUTRAL`, `RED`); a cor nunca é a única pista.
String signalText(AppLocalizations l, String wire) => switch (wire) {
  'GREEN' => l.signalGreen,
  'NEUTRAL' => l.signalNeutral,
  'RED' => l.signalRed,
  _ => '—',
};

/// "14 verdes de 20" / "14 greens of 20".
String greensOfTotalText(AppLocalizations l, int greens, int total) =>
    l.greensOfTotal(l.greenCount(greens), total);

/// Texto do ponto tocado: "8 verdes de 20 · 18 registrados · Neutro".
String historyPointText(AppLocalizations l, ScoreHistoryPoint point) => l.historyPoint(
  greensOfTotalText(l, point.green, point.total),
  l.loggedCount(point.logged),
  _verdictShort(l, point.verdict),
);

String periodLabel(AppLocalizations l, HistoryPeriod period) => switch (period) {
  HistoryPeriod.days30 => '30D',
  HistoryPeriod.days90 => '90D',
  HistoryPeriod.months12 => '12M',
  HistoryPeriod.all => l.periodAll,
};

/// "Verde ≥ 2 · Vermelho ≤ 0": o que cada faixa quer dizer. Verde acima do vermelho é
/// "maior é melhor" (mesma regra do `classify`).
String bandsText(AppLocalizations l, double green, double red) {
  final greenText = formatBoundary(green, locale: l.localeName);
  final redText = formatBoundary(red, locale: l.localeName);
  return green > red
      ? l.bandsHigherIsBetter(greenText, redText)
      : l.bandsLowerIsBetter(greenText, redText);
}

String boundaryErrorText(AppLocalizations l, BoundaryDraftError error) => switch (error) {
  BoundaryDraftError.needTwoNumbers => l.thresholdNeedTwoNumbers,
  BoundaryDraftError.needDifferentNumbers => l.thresholdNeedDifferentNumbers,
};
