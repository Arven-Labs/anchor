import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'app_localizations_en.dart';
import 'app_localizations_pt.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of AppLocalizations
/// returned by `AppLocalizations.of(context)`.
///
/// Applications need to include `AppLocalizations.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'l10n/app_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: AppLocalizations.localizationsDelegates,
///   supportedLocales: AppLocalizations.supportedLocales,
///   home: MyApplicationHome(),
/// );
/// ```
///
/// ## Update pubspec.yaml
///
/// Please make sure to update your pubspec.yaml to include the following
/// packages:
///
/// ```yaml
/// dependencies:
///   # Internationalization support.
///   flutter_localizations:
///     sdk: flutter
///   intl: any # Use the pinned version from flutter_localizations
///
///   # Rest of dependencies
/// ```
///
/// ## iOS Applications
///
/// iOS applications define key application metadata, including supported
/// locales, in an Info.plist file that is built into the application bundle.
/// To configure the locales supported by your app, you’ll need to edit this
/// file.
///
/// First, open your project’s ios/Runner.xcworkspace Xcode workspace file.
/// Then, in the Project Navigator, open the Info.plist file under the Runner
/// project’s Runner folder.
///
/// Next, select the Information Property List item, select Add Item from the
/// Editor menu, then select Localizations from the pop-up menu.
///
/// Select and expand the newly-created Localizations item then, for each
/// locale your application supports, add a new item and select the locale
/// you wish to add from the pop-up menu in the Value field. This list should
/// be consistent with the languages listed in the AppLocalizations.supportedLocales
/// property.
abstract class AppLocalizations {
  AppLocalizations(String locale)
    : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static AppLocalizations of(BuildContext context) {
    return Localizations.of<AppLocalizations>(context, AppLocalizations)!;
  }

  static const LocalizationsDelegate<AppLocalizations> delegate =
      _AppLocalizationsDelegate();

  /// A list of this localizations delegate along with the default localizations
  /// delegates.
  ///
  /// Returns a list of localizations delegates containing this delegate along with
  /// GlobalMaterialLocalizations.delegate, GlobalCupertinoLocalizations.delegate,
  /// and GlobalWidgetsLocalizations.delegate.
  ///
  /// Additional delegates can be added by appending to this list in
  /// MaterialApp. This list does not have to be used at all if a custom list
  /// of delegates is preferred or required.
  static const List<LocalizationsDelegate<dynamic>> localizationsDelegates =
      <LocalizationsDelegate<dynamic>>[
        delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
      ];

  /// A list of this localizations delegate's supported locales.
  static const List<Locale> supportedLocales = <Locale>[
    Locale('en'),
    Locale('pt'),
  ];

  /// No description provided for @cryptoScoreTitle.
  ///
  /// In en, this message translates to:
  /// **'Crypto score'**
  String get cryptoScoreTitle;

  /// No description provided for @cryptoThresholdsTooltip.
  ///
  /// In en, this message translates to:
  /// **'Thresholds'**
  String get cryptoThresholdsTooltip;

  /// No description provided for @cryptoRefresh.
  ///
  /// In en, this message translates to:
  /// **'Refresh'**
  String get cryptoRefresh;

  /// No description provided for @cryptoRefreshing.
  ///
  /// In en, this message translates to:
  /// **'Refreshing…'**
  String get cryptoRefreshing;

  /// No description provided for @cryptoNoReadings.
  ///
  /// In en, this message translates to:
  /// **'No readings yet. Tap Refresh to collect.'**
  String get cryptoNoReadings;

  /// No description provided for @cryptoUnexpectedError.
  ///
  /// In en, this message translates to:
  /// **'Unexpected error: {detail}'**
  String cryptoUnexpectedError(String detail);

  /// No description provided for @cryptoFailuresTitle.
  ///
  /// In en, this message translates to:
  /// **'Last collection failures'**
  String get cryptoFailuresTitle;

  /// No description provided for @cryptoValidReadings.
  ///
  /// In en, this message translates to:
  /// **'{count} with a valid reading'**
  String cryptoValidReadings(int count);

  /// No description provided for @cryptoStatusNoReading.
  ///
  /// In en, this message translates to:
  /// **'No reading'**
  String get cryptoStatusNoReading;

  /// No description provided for @cryptoStatusExpired.
  ///
  /// In en, this message translates to:
  /// **'Expired'**
  String get cryptoStatusExpired;

  /// No description provided for @greenCount.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =0{0 greens} =1{1 green} other{{count} greens}}'**
  String greenCount(int count);

  /// No description provided for @loggedCount.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =0{0 logged} =1{1 logged} other{{count} logged}}'**
  String loggedCount(int count);

  /// No description provided for @greensOfTotal.
  ///
  /// In en, this message translates to:
  /// **'{greens} of {total}'**
  String greensOfTotal(String greens, int total);

  /// No description provided for @historyTitle.
  ///
  /// In en, this message translates to:
  /// **'History'**
  String get historyTitle;

  /// No description provided for @historyEmpty.
  ///
  /// In en, this message translates to:
  /// **'No history yet.'**
  String get historyEmpty;

  /// No description provided for @historyExplainer.
  ///
  /// In en, this message translates to:
  /// **'Share of indicators that are green on each date. A reading stops counting once it expires.'**
  String get historyExplainer;

  /// No description provided for @historyDateCount.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =0{0 dates} =1{1 date} other{{count} dates}}'**
  String historyDateCount(int count);

  /// No description provided for @historySummary.
  ///
  /// In en, this message translates to:
  /// **'{dates} · latest: {greensOfTotal} on {date}'**
  String historySummary(String dates, String greensOfTotal, String date);

  /// No description provided for @historyDetail.
  ///
  /// In en, this message translates to:
  /// **'{date} — {text}'**
  String historyDetail(String date, String text);

  /// No description provided for @historyPoint.
  ///
  /// In en, this message translates to:
  /// **'{greensOfTotal} · {logged} · {verdict}'**
  String historyPoint(String greensOfTotal, String logged, String verdict);

  /// No description provided for @periodAll.
  ///
  /// In en, this message translates to:
  /// **'All'**
  String get periodAll;

  /// No description provided for @verdictIntact.
  ///
  /// In en, this message translates to:
  /// **'Thesis intact — hold / keep contributing'**
  String get verdictIntact;

  /// No description provided for @verdictNeutral.
  ///
  /// In en, this message translates to:
  /// **'Neutral — monitor closely'**
  String get verdictNeutral;

  /// No description provided for @verdictReduceRisk.
  ///
  /// In en, this message translates to:
  /// **'Reduce risk'**
  String get verdictReduceRisk;

  /// No description provided for @verdictShortIntact.
  ///
  /// In en, this message translates to:
  /// **'Thesis intact'**
  String get verdictShortIntact;

  /// No description provided for @verdictShortNeutral.
  ///
  /// In en, this message translates to:
  /// **'Neutral'**
  String get verdictShortNeutral;

  /// No description provided for @verdictShortReduceRisk.
  ///
  /// In en, this message translates to:
  /// **'Reduce risk'**
  String get verdictShortReduceRisk;

  /// No description provided for @signalGreen.
  ///
  /// In en, this message translates to:
  /// **'Green'**
  String get signalGreen;

  /// No description provided for @signalNeutral.
  ///
  /// In en, this message translates to:
  /// **'Neutral'**
  String get signalNeutral;

  /// No description provided for @signalRed.
  ///
  /// In en, this message translates to:
  /// **'Red'**
  String get signalRed;

  /// No description provided for @groupOnChain.
  ///
  /// In en, this message translates to:
  /// **'On-chain'**
  String get groupOnChain;

  /// No description provided for @groupFundamentals.
  ///
  /// In en, this message translates to:
  /// **'Network fundamentals'**
  String get groupFundamentals;

  /// No description provided for @groupDerivatives.
  ///
  /// In en, this message translates to:
  /// **'Derivatives & options'**
  String get groupDerivatives;

  /// No description provided for @groupLiquidity.
  ///
  /// In en, this message translates to:
  /// **'Market liquidity'**
  String get groupLiquidity;

  /// No description provided for @groupSentiment.
  ///
  /// In en, this message translates to:
  /// **'Market sentiment'**
  String get groupSentiment;

  /// No description provided for @indicatorMvrvZScore.
  ///
  /// In en, this message translates to:
  /// **'MVRV Z-Score'**
  String get indicatorMvrvZScore;

  /// No description provided for @indicatorNvtRatio.
  ///
  /// In en, this message translates to:
  /// **'NVT (vs. 90-day average)'**
  String get indicatorNvtRatio;

  /// No description provided for @indicatorPuellMultiple.
  ///
  /// In en, this message translates to:
  /// **'Puell Multiple'**
  String get indicatorPuellMultiple;

  /// No description provided for @indicatorNetIssuance.
  ///
  /// In en, this message translates to:
  /// **'Net issuance (issuance − burn)'**
  String get indicatorNetIssuance;

  /// No description provided for @indicatorStakingYield.
  ///
  /// In en, this message translates to:
  /// **'Real staking yield'**
  String get indicatorStakingYield;

  /// No description provided for @indicatorTvlTrend.
  ///
  /// In en, this message translates to:
  /// **'TVL trend (30d)'**
  String get indicatorTvlTrend;

  /// No description provided for @indicatorActiveAddressesTrend.
  ///
  /// In en, this message translates to:
  /// **'Active addresses (30d)'**
  String get indicatorActiveAddressesTrend;

  /// No description provided for @indicatorExchangeNetflow.
  ///
  /// In en, this message translates to:
  /// **'Exchange netflow'**
  String get indicatorExchangeNetflow;

  /// No description provided for @indicatorFeesVsEmission.
  ///
  /// In en, this message translates to:
  /// **'Fees vs. issuance'**
  String get indicatorFeesVsEmission;

  /// No description provided for @indicatorFundingRate7d.
  ///
  /// In en, this message translates to:
  /// **'Perpetual funding (7d, annualized)'**
  String get indicatorFundingRate7d;

  /// No description provided for @indicatorOpenInterestChange30d.
  ///
  /// In en, this message translates to:
  /// **'Open interest (30d change)'**
  String get indicatorOpenInterestChange30d;

  /// No description provided for @indicatorLongShortRatio.
  ///
  /// In en, this message translates to:
  /// **'Long/short ratio (7d)'**
  String get indicatorLongShortRatio;

  /// No description provided for @indicatorDvol.
  ///
  /// In en, this message translates to:
  /// **'DVOL (implied volatility)'**
  String get indicatorDvol;

  /// No description provided for @indicatorPutCallRatio.
  ///
  /// In en, this message translates to:
  /// **'Options put/call'**
  String get indicatorPutCallRatio;

  /// No description provided for @indicatorStablecoinSupplyChange30d.
  ///
  /// In en, this message translates to:
  /// **'Stablecoin supply (30d)'**
  String get indicatorStablecoinSupplyChange30d;

  /// No description provided for @indicatorExchangeSupplyChange30d.
  ///
  /// In en, this message translates to:
  /// **'Exchange supply (30d)'**
  String get indicatorExchangeSupplyChange30d;

  /// No description provided for @indicatorEthBtcTrend30d.
  ///
  /// In en, this message translates to:
  /// **'ETH/BTC (30d)'**
  String get indicatorEthBtcTrend30d;

  /// No description provided for @indicatorFearGreedIndex.
  ///
  /// In en, this message translates to:
  /// **'Fear & Greed (0–100)'**
  String get indicatorFearGreedIndex;

  /// No description provided for @indicatorMayerMultiple.
  ///
  /// In en, this message translates to:
  /// **'Mayer Multiple'**
  String get indicatorMayerMultiple;

  /// No description provided for @indicatorBtcDominance.
  ///
  /// In en, this message translates to:
  /// **'BTC dominance'**
  String get indicatorBtcDominance;

  /// No description provided for @indicatorSopr.
  ///
  /// In en, this message translates to:
  /// **'SOPR'**
  String get indicatorSopr;

  /// No description provided for @indicatorNupl.
  ///
  /// In en, this message translates to:
  /// **'NUPL'**
  String get indicatorNupl;

  /// No description provided for @thresholdsTitle.
  ///
  /// In en, this message translates to:
  /// **'Thresholds · {coin}'**
  String thresholdsTitle(String coin);

  /// No description provided for @thresholdDefault.
  ///
  /// In en, this message translates to:
  /// **'Default'**
  String get thresholdDefault;

  /// No description provided for @thresholdEdited.
  ///
  /// In en, this message translates to:
  /// **'Edited'**
  String get thresholdEdited;

  /// No description provided for @thresholdGreen.
  ///
  /// In en, this message translates to:
  /// **'Green'**
  String get thresholdGreen;

  /// No description provided for @thresholdRed.
  ///
  /// In en, this message translates to:
  /// **'Red'**
  String get thresholdRed;

  /// No description provided for @bandsHigherIsBetter.
  ///
  /// In en, this message translates to:
  /// **'Green ≥ {green} · Red ≤ {red}'**
  String bandsHigherIsBetter(String green, String red);

  /// No description provided for @bandsLowerIsBetter.
  ///
  /// In en, this message translates to:
  /// **'Green ≤ {green} · Red ≥ {red}'**
  String bandsLowerIsBetter(String green, String red);

  /// No description provided for @thresholdHigherIsBetter.
  ///
  /// In en, this message translates to:
  /// **'Higher is better'**
  String get thresholdHigherIsBetter;

  /// No description provided for @thresholdLowerIsBetter.
  ///
  /// In en, this message translates to:
  /// **'Lower is better'**
  String get thresholdLowerIsBetter;

  /// No description provided for @thresholdDefaultValues.
  ///
  /// In en, this message translates to:
  /// **'Default: green {green}, red {red}'**
  String thresholdDefaultValues(String green, String red);

  /// No description provided for @thresholdSaveWarning.
  ///
  /// In en, this message translates to:
  /// **'Saving reclassifies the stored readings and changes the history: the past is viewed with these thresholds.'**
  String get thresholdSaveWarning;

  /// No description provided for @thresholdNeedTwoNumbers.
  ///
  /// In en, this message translates to:
  /// **'Enter two numbers.'**
  String get thresholdNeedTwoNumbers;

  /// No description provided for @thresholdNeedDifferentNumbers.
  ///
  /// In en, this message translates to:
  /// **'Use two different numbers.'**
  String get thresholdNeedDifferentNumbers;

  /// No description provided for @actionReset.
  ///
  /// In en, this message translates to:
  /// **'Reset'**
  String get actionReset;

  /// No description provided for @actionCancel.
  ///
  /// In en, this message translates to:
  /// **'Cancel'**
  String get actionCancel;

  /// No description provided for @actionSave.
  ///
  /// In en, this message translates to:
  /// **'Save'**
  String get actionSave;

  /// No description provided for @actionSaving.
  ///
  /// In en, this message translates to:
  /// **'Saving…'**
  String get actionSaving;

  /// No description provided for @navHome.
  ///
  /// In en, this message translates to:
  /// **'Home'**
  String get navHome;

  /// No description provided for @navQuote.
  ///
  /// In en, this message translates to:
  /// **'Quote'**
  String get navQuote;

  /// No description provided for @navPortfolio.
  ///
  /// In en, this message translates to:
  /// **'Portfolio'**
  String get navPortfolio;

  /// No description provided for @navFinance.
  ///
  /// In en, this message translates to:
  /// **'Finances'**
  String get navFinance;

  /// No description provided for @navCrypto.
  ///
  /// In en, this message translates to:
  /// **'Crypto'**
  String get navCrypto;

  /// No description provided for @homeTitle.
  ///
  /// In en, this message translates to:
  /// **'Home'**
  String get homeTitle;

  /// No description provided for @homeCustomizeTitle.
  ///
  /// In en, this message translates to:
  /// **'Customize Home'**
  String get homeCustomizeTitle;

  /// No description provided for @homeCustomizeTooltip.
  ///
  /// In en, this message translates to:
  /// **'Customize'**
  String get homeCustomizeTooltip;

  /// No description provided for @homeRestoreDefault.
  ///
  /// In en, this message translates to:
  /// **'Restore default'**
  String get homeRestoreDefault;

  /// No description provided for @homeWidthFull.
  ///
  /// In en, this message translates to:
  /// **'Full'**
  String get homeWidthFull;

  /// No description provided for @homeWidthHalf.
  ///
  /// In en, this message translates to:
  /// **'Half'**
  String get homeWidthHalf;

  /// No description provided for @homeSaveFailed.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t save the layout: {error}'**
  String homeSaveFailed(String error);

  /// No description provided for @homeLoadFailed.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t load the quotes right now. Pull to refresh. ({error})'**
  String homeLoadFailed(String error);

  /// No description provided for @homeAllHidden.
  ///
  /// In en, this message translates to:
  /// **'All cards are hidden. Tap Customize to show them.'**
  String get homeAllHidden;

  /// No description provided for @homeTotalMissing.
  ///
  /// In en, this message translates to:
  /// **'{count} left out of the sum (no quote or exchange rate)'**
  String homeTotalMissing(int count);

  /// No description provided for @homeProfitExcluded.
  ///
  /// In en, this message translates to:
  /// **'{count} left out of the calculation'**
  String homeProfitExcluded(int count);

  /// No description provided for @homeNoPositions.
  ///
  /// In en, this message translates to:
  /// **'No open positions.'**
  String get homeNoPositions;

  /// No description provided for @homeCash.
  ///
  /// In en, this message translates to:
  /// **'Cash'**
  String get homeCash;

  /// No description provided for @homePortfolio.
  ///
  /// In en, this message translates to:
  /// **'Portfolio'**
  String get homePortfolio;

  /// No description provided for @homeLoansReceivable.
  ///
  /// In en, this message translates to:
  /// **'Loans'**
  String get homeLoansReceivable;

  /// No description provided for @homeDebts.
  ///
  /// In en, this message translates to:
  /// **'Debts'**
  String get homeDebts;

  /// No description provided for @homeNetWorthMissing.
  ///
  /// In en, this message translates to:
  /// **'{count} item(s) left out of the sum for lack of exchange rate or quote.'**
  String homeNetWorthMissing(int count);

  /// No description provided for @homeFxUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Exchange rate unavailable right now for {currencies}.'**
  String homeFxUnavailable(String currencies);

  /// No description provided for @homeWidgetTotalValue.
  ///
  /// In en, this message translates to:
  /// **'Total value'**
  String get homeWidgetTotalValue;

  /// No description provided for @homeWidgetProfit.
  ///
  /// In en, this message translates to:
  /// **'Unrealized profit'**
  String get homeWidgetProfit;

  /// No description provided for @homeWidgetAllocation.
  ///
  /// In en, this message translates to:
  /// **'Allocation by class'**
  String get homeWidgetAllocation;

  /// No description provided for @homeWidgetNetWorth.
  ///
  /// In en, this message translates to:
  /// **'Net worth'**
  String get homeWidgetNetWorth;

  /// No description provided for @assetClassAcaoBr.
  ///
  /// In en, this message translates to:
  /// **'BR stock'**
  String get assetClassAcaoBr;

  /// No description provided for @assetClassFii.
  ///
  /// In en, this message translates to:
  /// **'FII'**
  String get assetClassFii;

  /// No description provided for @assetClassEtfBr.
  ///
  /// In en, this message translates to:
  /// **'BR ETF'**
  String get assetClassEtfBr;

  /// No description provided for @assetClassEtfRendaFixa.
  ///
  /// In en, this message translates to:
  /// **'BR fixed-income ETF'**
  String get assetClassEtfRendaFixa;

  /// No description provided for @assetClassBdr.
  ///
  /// In en, this message translates to:
  /// **'BDR'**
  String get assetClassBdr;

  /// No description provided for @assetClassAcaoInternacional.
  ///
  /// In en, this message translates to:
  /// **'International stock'**
  String get assetClassAcaoInternacional;

  /// No description provided for @assetClassReit.
  ///
  /// In en, this message translates to:
  /// **'REIT'**
  String get assetClassReit;

  /// No description provided for @assetClassEtfUs.
  ///
  /// In en, this message translates to:
  /// **'US ETF'**
  String get assetClassEtfUs;

  /// No description provided for @assetClassCripto.
  ///
  /// In en, this message translates to:
  /// **'Crypto'**
  String get assetClassCripto;

  /// No description provided for @assetClassMetal.
  ///
  /// In en, this message translates to:
  /// **'Metal'**
  String get assetClassMetal;

  /// No description provided for @assetClassTesouroDireto.
  ///
  /// In en, this message translates to:
  /// **'Tesouro Direto'**
  String get assetClassTesouroDireto;

  /// No description provided for @assetClassRendaFixa.
  ///
  /// In en, this message translates to:
  /// **'Fixed income'**
  String get assetClassRendaFixa;

  /// No description provided for @assetClassImovel.
  ///
  /// In en, this message translates to:
  /// **'Real estate'**
  String get assetClassImovel;

  /// No description provided for @assetClassEmpresaNaoListada.
  ///
  /// In en, this message translates to:
  /// **'Unlisted company'**
  String get assetClassEmpresaNaoListada;

  /// No description provided for @assetClassFundo.
  ///
  /// In en, this message translates to:
  /// **'Fund'**
  String get assetClassFundo;

  /// No description provided for @portfolioTitle.
  ///
  /// In en, this message translates to:
  /// **'Portfolio'**
  String get portfolioTitle;

  /// No description provided for @portfolioCryptoTax.
  ///
  /// In en, this message translates to:
  /// **'Crypto tax'**
  String get portfolioCryptoTax;

  /// No description provided for @portfolioForeignTax.
  ///
  /// In en, this message translates to:
  /// **'Foreign tax'**
  String get portfolioForeignTax;

  /// No description provided for @portfolioFundTax.
  ///
  /// In en, this message translates to:
  /// **'Fund tax'**
  String get portfolioFundTax;

  /// No description provided for @portfolioSwapCrypto.
  ///
  /// In en, this message translates to:
  /// **'Swap crypto'**
  String get portfolioSwapCrypto;

  /// No description provided for @portfolioCustodies.
  ///
  /// In en, this message translates to:
  /// **'Custodians'**
  String get portfolioCustodies;

  /// No description provided for @portfolioHistory.
  ///
  /// In en, this message translates to:
  /// **'History'**
  String get portfolioHistory;

  /// No description provided for @portfolioAddAsset.
  ///
  /// In en, this message translates to:
  /// **'Add asset'**
  String get portfolioAddAsset;

  /// No description provided for @portfolioEmpty.
  ///
  /// In en, this message translates to:
  /// **'No assets yet.'**
  String get portfolioEmpty;

  /// No description provided for @portfolioUnits.
  ///
  /// In en, this message translates to:
  /// **'{quantity} units'**
  String portfolioUnits(String quantity);

  /// No description provided for @portfolioAveragePrice.
  ///
  /// In en, this message translates to:
  /// **'Average price: {value}'**
  String portfolioAveragePrice(String value);

  /// No description provided for @portfolioCurrentPrice.
  ///
  /// In en, this message translates to:
  /// **'Current price: {value}'**
  String portfolioCurrentPrice(String value);

  /// No description provided for @portfolioValue.
  ///
  /// In en, this message translates to:
  /// **'Value: {value}'**
  String portfolioValue(String value);

  /// No description provided for @portfolioPriceUnavailable.
  ///
  /// In en, this message translates to:
  /// **'unavailable right now'**
  String get portfolioPriceUnavailable;

  /// No description provided for @portfolioShare.
  ///
  /// In en, this message translates to:
  /// **'Share: {percent}% — {value}'**
  String portfolioShare(String percent, String value);
}

class _AppLocalizationsDelegate
    extends LocalizationsDelegate<AppLocalizations> {
  const _AppLocalizationsDelegate();

  @override
  Future<AppLocalizations> load(Locale locale) {
    return SynchronousFuture<AppLocalizations>(lookupAppLocalizations(locale));
  }

  @override
  bool isSupported(Locale locale) =>
      <String>['en', 'pt'].contains(locale.languageCode);

  @override
  bool shouldReload(_AppLocalizationsDelegate old) => false;
}

AppLocalizations lookupAppLocalizations(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'en':
      return AppLocalizationsEn();
    case 'pt':
      return AppLocalizationsPt();
  }

  throw FlutterError(
    'AppLocalizations.delegate failed to load unsupported locale "$locale". This is likely '
    'an issue with the localizations generation tool. Please file an issue '
    'on GitHub with a reproducible sample app and the gen-l10n configuration '
    'that was used.',
  );
}
