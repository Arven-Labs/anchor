// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class AppLocalizationsEn extends AppLocalizations {
  AppLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get cryptoScoreTitle => 'Crypto score';

  @override
  String get cryptoThresholdsTooltip => 'Thresholds';

  @override
  String get cryptoRefresh => 'Refresh';

  @override
  String get cryptoRefreshing => 'Refreshing…';

  @override
  String get cryptoNoReadings => 'No readings yet. Tap Refresh to collect.';

  @override
  String cryptoUnexpectedError(String detail) {
    return 'Unexpected error: $detail';
  }

  @override
  String get cryptoFailuresTitle => 'Last collection failures';

  @override
  String cryptoValidReadings(int count) {
    return '$count with a valid reading';
  }

  @override
  String get cryptoStatusNoReading => 'No reading';

  @override
  String get cryptoStatusExpired => 'Expired';

  @override
  String greenCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count greens',
      one: '1 green',
      zero: '0 greens',
    );
    return '$_temp0';
  }

  @override
  String loggedCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count logged',
      one: '1 logged',
      zero: '0 logged',
    );
    return '$_temp0';
  }

  @override
  String greensOfTotal(String greens, int total) {
    return '$greens of $total';
  }

  @override
  String get historyTitle => 'History';

  @override
  String get historyEmpty => 'No history yet.';

  @override
  String get historyExplainer =>
      'Share of indicators that are green on each date. A reading stops counting once it expires.';

  @override
  String historyDateCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count dates',
      one: '1 date',
      zero: '0 dates',
    );
    return '$_temp0';
  }

  @override
  String historySummary(String dates, String greensOfTotal, String date) {
    return '$dates · latest: $greensOfTotal on $date';
  }

  @override
  String historyDetail(String date, String text) {
    return '$date — $text';
  }

  @override
  String historyPoint(String greensOfTotal, String logged, String verdict) {
    return '$greensOfTotal · $logged · $verdict';
  }

  @override
  String get periodAll => 'All';

  @override
  String get verdictIntact => 'Thesis intact — hold / keep contributing';

  @override
  String get verdictNeutral => 'Neutral — monitor closely';

  @override
  String get verdictReduceRisk => 'Reduce risk';

  @override
  String get verdictShortIntact => 'Thesis intact';

  @override
  String get verdictShortNeutral => 'Neutral';

  @override
  String get verdictShortReduceRisk => 'Reduce risk';

  @override
  String get signalGreen => 'Green';

  @override
  String get signalNeutral => 'Neutral';

  @override
  String get signalRed => 'Red';

  @override
  String get groupOnChain => 'On-chain';

  @override
  String get groupFundamentals => 'Network fundamentals';

  @override
  String get groupDerivatives => 'Derivatives & options';

  @override
  String get groupLiquidity => 'Market liquidity';

  @override
  String get groupSentiment => 'Market sentiment';

  @override
  String get indicatorMvrvZScore => 'MVRV Z-Score';

  @override
  String get indicatorNvtRatio => 'NVT (vs. 90-day average)';

  @override
  String get indicatorPuellMultiple => 'Puell Multiple';

  @override
  String get indicatorNetIssuance => 'Net issuance (issuance − burn)';

  @override
  String get indicatorStakingYield => 'Real staking yield';

  @override
  String get indicatorTvlTrend => 'TVL trend (30d)';

  @override
  String get indicatorActiveAddressesTrend => 'Active addresses (30d)';

  @override
  String get indicatorExchangeNetflow => 'Exchange netflow';

  @override
  String get indicatorFeesVsEmission => 'Fees vs. issuance';

  @override
  String get indicatorFundingRate7d => 'Perpetual funding (7d, annualized)';

  @override
  String get indicatorOpenInterestChange30d => 'Open interest (30d change)';

  @override
  String get indicatorLongShortRatio => 'Long/short ratio (7d)';

  @override
  String get indicatorDvol => 'DVOL (implied volatility)';

  @override
  String get indicatorPutCallRatio => 'Options put/call';

  @override
  String get indicatorStablecoinSupplyChange30d => 'Stablecoin supply (30d)';

  @override
  String get indicatorExchangeSupplyChange30d => 'Exchange supply (30d)';

  @override
  String get indicatorEthBtcTrend30d => 'ETH/BTC (30d)';

  @override
  String get indicatorFearGreedIndex => 'Fear & Greed (0–100)';

  @override
  String get indicatorMayerMultiple => 'Mayer Multiple';

  @override
  String get indicatorBtcDominance => 'BTC dominance';

  @override
  String get indicatorSopr => 'SOPR';

  @override
  String get indicatorNupl => 'NUPL';

  @override
  String thresholdsTitle(String coin) {
    return 'Thresholds · $coin';
  }

  @override
  String get thresholdDefault => 'Default';

  @override
  String get thresholdEdited => 'Edited';

  @override
  String get thresholdGreen => 'Green';

  @override
  String get thresholdRed => 'Red';

  @override
  String bandsHigherIsBetter(String green, String red) {
    return 'Green ≥ $green · Red ≤ $red';
  }

  @override
  String bandsLowerIsBetter(String green, String red) {
    return 'Green ≤ $green · Red ≥ $red';
  }

  @override
  String get thresholdHigherIsBetter => 'Higher is better';

  @override
  String get thresholdLowerIsBetter => 'Lower is better';

  @override
  String thresholdDefaultValues(String green, String red) {
    return 'Default: green $green, red $red';
  }

  @override
  String get thresholdSaveWarning =>
      'Saving reclassifies the stored readings and changes the history: the past is viewed with these thresholds.';

  @override
  String get thresholdNeedTwoNumbers => 'Enter two numbers.';

  @override
  String get thresholdNeedDifferentNumbers => 'Use two different numbers.';

  @override
  String get actionReset => 'Reset';

  @override
  String get actionCancel => 'Cancel';

  @override
  String get actionSave => 'Save';

  @override
  String get actionSaving => 'Saving…';
}
