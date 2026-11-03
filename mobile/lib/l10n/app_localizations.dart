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

  /// No description provided for @actionEdit.
  ///
  /// In en, this message translates to:
  /// **'Edit'**
  String get actionEdit;

  /// No description provided for @actionDelete.
  ///
  /// In en, this message translates to:
  /// **'Delete'**
  String get actionDelete;

  /// No description provided for @finTitle.
  ///
  /// In en, this message translates to:
  /// **'Finances'**
  String get finTitle;

  /// No description provided for @finTransfer.
  ///
  /// In en, this message translates to:
  /// **'Transfer'**
  String get finTransfer;

  /// No description provided for @finLoans.
  ///
  /// In en, this message translates to:
  /// **'Loans'**
  String get finLoans;

  /// No description provided for @finDebts.
  ///
  /// In en, this message translates to:
  /// **'Debts'**
  String get finDebts;

  /// No description provided for @finCategories.
  ///
  /// In en, this message translates to:
  /// **'Categories'**
  String get finCategories;

  /// No description provided for @finRefresh.
  ///
  /// In en, this message translates to:
  /// **'Refresh'**
  String get finRefresh;

  /// No description provided for @finFabAccount.
  ///
  /// In en, this message translates to:
  /// **'Account'**
  String get finFabAccount;

  /// No description provided for @finEmptyAccounts.
  ///
  /// In en, this message translates to:
  /// **'No accounts yet.'**
  String get finEmptyAccounts;

  /// No description provided for @finNetWorth.
  ///
  /// In en, this message translates to:
  /// **'Net worth'**
  String get finNetWorth;

  /// No description provided for @finCalculating.
  ///
  /// In en, this message translates to:
  /// **'Calculating…'**
  String get finCalculating;

  /// No description provided for @finCash.
  ///
  /// In en, this message translates to:
  /// **'Cash {value}'**
  String finCash(String value);

  /// No description provided for @finPortfolio.
  ///
  /// In en, this message translates to:
  /// **'Portfolio {value}'**
  String finPortfolio(String value);

  /// No description provided for @finLoansReceivable.
  ///
  /// In en, this message translates to:
  /// **'Loans receivable {value}'**
  String finLoansReceivable(String value);

  /// No description provided for @finDebtsLine.
  ///
  /// In en, this message translates to:
  /// **'Debts −{value}'**
  String finDebtsLine(String value);

  /// No description provided for @finMissingItems.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 item left out of the sum for lack of exchange rate or quote.} other{{count} items left out of the sum for lack of exchange rate or quote.}}'**
  String finMissingItems(int count);

  /// No description provided for @finFxBanner.
  ///
  /// In en, this message translates to:
  /// **'Exchange rate unavailable right now for {currencies}. Items in those currencies are left out of net worth.'**
  String finFxBanner(String currencies);

  /// No description provided for @finNoRate.
  ///
  /// In en, this message translates to:
  /// **'no exchange rate {pair}'**
  String finNoRate(String pair);

  /// No description provided for @finNewAccount.
  ///
  /// In en, this message translates to:
  /// **'New account'**
  String get finNewAccount;

  /// No description provided for @finEditAccount.
  ///
  /// In en, this message translates to:
  /// **'Edit account'**
  String get finEditAccount;

  /// No description provided for @finFieldName.
  ///
  /// In en, this message translates to:
  /// **'Name'**
  String get finFieldName;

  /// No description provided for @finFieldHolder.
  ///
  /// In en, this message translates to:
  /// **'Holder'**
  String get finFieldHolder;

  /// No description provided for @finFieldCurrency.
  ///
  /// In en, this message translates to:
  /// **'Currency'**
  String get finFieldCurrency;

  /// No description provided for @finAccountNameHint.
  ///
  /// In en, this message translates to:
  /// **'Nubank'**
  String get finAccountNameHint;

  /// No description provided for @finCurrencyLocked.
  ///
  /// In en, this message translates to:
  /// **'Can\'t change on an account that already has transactions.'**
  String get finCurrencyLocked;

  /// No description provided for @finDeleteTitle.
  ///
  /// In en, this message translates to:
  /// **'Delete {name}?'**
  String finDeleteTitle(String name);

  /// No description provided for @finDeleteWithTx.
  ///
  /// In en, this message translates to:
  /// **'The transactions of this account will also be deleted.'**
  String get finDeleteWithTx;

  /// No description provided for @finDeleteNoTx.
  ///
  /// In en, this message translates to:
  /// **'The account has no transactions.'**
  String get finDeleteNoTx;

  /// No description provided for @finTransferNeedTwo.
  ///
  /// In en, this message translates to:
  /// **'Add two accounts to transfer.'**
  String get finTransferNeedTwo;

  /// No description provided for @finFrom.
  ///
  /// In en, this message translates to:
  /// **'From'**
  String get finFrom;

  /// No description provided for @finTo.
  ///
  /// In en, this message translates to:
  /// **'To'**
  String get finTo;

  /// No description provided for @finAmountSent.
  ///
  /// In en, this message translates to:
  /// **'Sent ({currency})'**
  String finAmountSent(String currency);

  /// No description provided for @finAmount.
  ///
  /// In en, this message translates to:
  /// **'Amount ({currency})'**
  String finAmount(String currency);

  /// No description provided for @finAmountReceived.
  ///
  /// In en, this message translates to:
  /// **'Received ({currency})'**
  String finAmountReceived(String currency);

  /// No description provided for @finEffectiveRate.
  ///
  /// In en, this message translates to:
  /// **'Effective rate: 1 {from} = {rate} {to}'**
  String finEffectiveRate(String from, String rate, String to);

  /// No description provided for @finSpreadIncluded.
  ///
  /// In en, this message translates to:
  /// **'Spread and fees already included in the amounts.'**
  String get finSpreadIncluded;

  /// No description provided for @finNotes.
  ///
  /// In en, this message translates to:
  /// **'Notes'**
  String get finNotes;

  /// No description provided for @finSameAccount.
  ///
  /// In en, this message translates to:
  /// **'Choose two different accounts.'**
  String get finSameAccount;

  /// No description provided for @finAmountsRequired.
  ///
  /// In en, this message translates to:
  /// **'Enter the amounts, greater than zero.'**
  String get finAmountsRequired;

  /// No description provided for @txNewTitle.
  ///
  /// In en, this message translates to:
  /// **'New transaction'**
  String get txNewTitle;

  /// No description provided for @txCategory.
  ///
  /// In en, this message translates to:
  /// **'Category'**
  String get txCategory;

  /// No description provided for @txNoCategory.
  ///
  /// In en, this message translates to:
  /// **'No category'**
  String get txNoCategory;

  /// No description provided for @txAmountRequired.
  ///
  /// In en, this message translates to:
  /// **'Enter an amount greater than zero.'**
  String get txAmountRequired;

  /// No description provided for @txDeleteTitle.
  ///
  /// In en, this message translates to:
  /// **'Delete transaction?'**
  String get txDeleteTitle;

  /// No description provided for @txDeleteBody.
  ///
  /// In en, this message translates to:
  /// **'{type} of {value} on {date}.'**
  String txDeleteBody(String type, String value, String date);

  /// No description provided for @txDeleteTransferLeg.
  ///
  /// In en, this message translates to:
  /// **'The other leg of the transfer will also be deleted.'**
  String get txDeleteTransferLeg;

  /// No description provided for @txFab.
  ///
  /// In en, this message translates to:
  /// **'Transaction'**
  String get txFab;

  /// No description provided for @txBalance.
  ///
  /// In en, this message translates to:
  /// **'Balance'**
  String get txBalance;

  /// No description provided for @txEmpty.
  ///
  /// In en, this message translates to:
  /// **'No transactions in this account yet.'**
  String get txEmpty;

  /// No description provided for @txFuture.
  ///
  /// In en, this message translates to:
  /// **'not in the balance yet'**
  String get txFuture;

  /// No description provided for @txTypeReceita.
  ///
  /// In en, this message translates to:
  /// **'Income'**
  String get txTypeReceita;

  /// No description provided for @txTypeDespesa.
  ///
  /// In en, this message translates to:
  /// **'Expense'**
  String get txTypeDespesa;

  /// No description provided for @txTypeTransferenciaSaida.
  ///
  /// In en, this message translates to:
  /// **'Transfer (out)'**
  String get txTypeTransferenciaSaida;

  /// No description provided for @txTypeTransferenciaEntrada.
  ///
  /// In en, this message translates to:
  /// **'Transfer (in)'**
  String get txTypeTransferenciaEntrada;

  /// No description provided for @txTypeParcelaDivida.
  ///
  /// In en, this message translates to:
  /// **'Debt installment'**
  String get txTypeParcelaDivida;

  /// No description provided for @txTypeEmprestimoConcedido.
  ///
  /// In en, this message translates to:
  /// **'Loan granted'**
  String get txTypeEmprestimoConcedido;

  /// No description provided for @txTypeRecebimentoEmprestimo.
  ///
  /// In en, this message translates to:
  /// **'Loan repayment'**
  String get txTypeRecebimentoEmprestimo;

  /// No description provided for @genTxInstallment.
  ///
  /// In en, this message translates to:
  /// **'{name} — installment {number}/{total}'**
  String genTxInstallment(String name, int number, int total);

  /// No description provided for @genTxLoanGranted.
  ///
  /// In en, this message translates to:
  /// **'Loan to {name}'**
  String genTxLoanGranted(String name);

  /// No description provided for @genTxLoanRepayment.
  ///
  /// In en, this message translates to:
  /// **'Repayment from {name}'**
  String genTxLoanRepayment(String name);

  /// No description provided for @amortizationPrice.
  ///
  /// In en, this message translates to:
  /// **'Price (fixed installment)'**
  String get amortizationPrice;

  /// No description provided for @amortizationSac.
  ///
  /// In en, this message translates to:
  /// **'SAC (fixed amortization)'**
  String get amortizationSac;

  /// No description provided for @actionRegister.
  ///
  /// In en, this message translates to:
  /// **'Add'**
  String get actionRegister;

  /// No description provided for @liabTitle.
  ///
  /// In en, this message translates to:
  /// **'Debts'**
  String get liabTitle;

  /// No description provided for @liabFab.
  ///
  /// In en, this message translates to:
  /// **'Debt'**
  String get liabFab;

  /// No description provided for @liabEmpty.
  ///
  /// In en, this message translates to:
  /// **'No debts yet.'**
  String get liabEmpty;

  /// No description provided for @liabNeedAccount.
  ///
  /// In en, this message translates to:
  /// **'Add an account before the debt.'**
  String get liabNeedAccount;

  /// No description provided for @liabNew.
  ///
  /// In en, this message translates to:
  /// **'New debt'**
  String get liabNew;

  /// No description provided for @liabNameHint.
  ///
  /// In en, this message translates to:
  /// **'Car financing'**
  String get liabNameHint;

  /// No description provided for @liabFieldAccount.
  ///
  /// In en, this message translates to:
  /// **'Account'**
  String get liabFieldAccount;

  /// No description provided for @liabFieldPrincipal.
  ///
  /// In en, this message translates to:
  /// **'Principal ({currency})'**
  String liabFieldPrincipal(String currency);

  /// No description provided for @liabFieldRate.
  ///
  /// In en, this message translates to:
  /// **'Rate (% per month)'**
  String get liabFieldRate;

  /// No description provided for @liabRateHelp.
  ///
  /// In en, this message translates to:
  /// **'0 for a debt without interest.'**
  String get liabRateHelp;

  /// No description provided for @liabFieldTerm.
  ///
  /// In en, this message translates to:
  /// **'Term (months)'**
  String get liabFieldTerm;

  /// No description provided for @liabFieldSystem.
  ///
  /// In en, this message translates to:
  /// **'System'**
  String get liabFieldSystem;

  /// No description provided for @liabStart.
  ///
  /// In en, this message translates to:
  /// **'Start {date}'**
  String liabStart(String date);

  /// No description provided for @liabFirstDue.
  ///
  /// In en, this message translates to:
  /// **'Installment 1 is due one month after the start.'**
  String get liabFirstDue;

  /// No description provided for @liabInvalid.
  ///
  /// In en, this message translates to:
  /// **'Enter a name, a principal and a term greater than zero, and the rate.'**
  String get liabInvalid;

  /// No description provided for @liabDeleteTitle.
  ///
  /// In en, this message translates to:
  /// **'Delete {name}?'**
  String liabDeleteTitle(String name);

  /// No description provided for @liabDeleteBody.
  ///
  /// In en, this message translates to:
  /// **'This debt\'s installments are also removed from the account\'s transactions.'**
  String get liabDeleteBody;

  /// No description provided for @liabDeleteAction.
  ///
  /// In en, this message translates to:
  /// **'Delete debt'**
  String get liabDeleteAction;

  /// No description provided for @liabBalance.
  ///
  /// In en, this message translates to:
  /// **'Outstanding balance {value}'**
  String liabBalance(String value);

  /// No description provided for @liabPaid.
  ///
  /// In en, this message translates to:
  /// **'{paid} of {total} installments paid'**
  String liabPaid(int paid, int total);

  /// No description provided for @liabNext.
  ///
  /// In en, this message translates to:
  /// **'Next: {value} on {date}'**
  String liabNext(String value, String date);

  /// No description provided for @liabSettled.
  ///
  /// In en, this message translates to:
  /// **'Paid off'**
  String get liabSettled;

  /// No description provided for @liabSummary.
  ///
  /// In en, this message translates to:
  /// **'{system} · {rate}% per month · principal {value}'**
  String liabSummary(String system, String rate, String value);

  /// No description provided for @liabInstallmentLine.
  ///
  /// In en, this message translates to:
  /// **'{number} · {date} · {value} (interest {interest})'**
  String liabInstallmentLine(
    String number,
    String date,
    String value,
    String interest,
  );

  /// No description provided for @loanTitle.
  ///
  /// In en, this message translates to:
  /// **'Loans'**
  String get loanTitle;

  /// No description provided for @loanFab.
  ///
  /// In en, this message translates to:
  /// **'Loan'**
  String get loanFab;

  /// No description provided for @loanEmpty.
  ///
  /// In en, this message translates to:
  /// **'No loans yet.'**
  String get loanEmpty;

  /// No description provided for @loanNew.
  ///
  /// In en, this message translates to:
  /// **'New loan'**
  String get loanNew;

  /// No description provided for @loanEdit.
  ///
  /// In en, this message translates to:
  /// **'Edit loan'**
  String get loanEdit;

  /// No description provided for @loanBorrower.
  ///
  /// In en, this message translates to:
  /// **'Borrower'**
  String get loanBorrower;

  /// No description provided for @loanNewBorrowerOption.
  ///
  /// In en, this message translates to:
  /// **'New borrower…'**
  String get loanNewBorrowerOption;

  /// No description provided for @loanNewBorrowerName.
  ///
  /// In en, this message translates to:
  /// **'Borrower\'s name'**
  String get loanNewBorrowerName;

  /// No description provided for @loanFromAccount.
  ///
  /// In en, this message translates to:
  /// **'Account it came from'**
  String get loanFromAccount;

  /// No description provided for @loanNoAccountHelp.
  ///
  /// In en, this message translates to:
  /// **'Without an account it is just a record: no balance changes.'**
  String get loanNoAccountHelp;

  /// No description provided for @loanNoAccount.
  ///
  /// In en, this message translates to:
  /// **'No account'**
  String get loanNoAccount;

  /// No description provided for @loanCurrency.
  ///
  /// In en, this message translates to:
  /// **'Currency'**
  String get loanCurrency;

  /// No description provided for @loanPrincipal.
  ///
  /// In en, this message translates to:
  /// **'Principal ({currency})'**
  String loanPrincipal(String currency);

  /// No description provided for @loanRateOptional.
  ///
  /// In en, this message translates to:
  /// **'Rate (% per month, optional)'**
  String get loanRateOptional;

  /// No description provided for @loanRateHelp.
  ///
  /// In en, this message translates to:
  /// **'Simple interest, informational only. Empty = no interest.'**
  String get loanRateHelp;

  /// No description provided for @loanDateOn.
  ///
  /// In en, this message translates to:
  /// **'On {date}'**
  String loanDateOn(String date);

  /// No description provided for @loanInclude.
  ///
  /// In en, this message translates to:
  /// **'Add to net worth'**
  String get loanInclude;

  /// No description provided for @loanNotes.
  ///
  /// In en, this message translates to:
  /// **'Note'**
  String get loanNotes;

  /// No description provided for @loanInvalid.
  ///
  /// In en, this message translates to:
  /// **'Enter the borrower and a principal greater than zero; the rate, if any, cannot be negative.'**
  String get loanInvalid;

  /// No description provided for @loanRateNegative.
  ///
  /// In en, this message translates to:
  /// **'The rate cannot be negative.'**
  String get loanRateNegative;

  /// No description provided for @loanRepayTitle.
  ///
  /// In en, this message translates to:
  /// **'Register repayment'**
  String get loanRepayTitle;

  /// No description provided for @loanRepayConfirm.
  ///
  /// In en, this message translates to:
  /// **'Register'**
  String get loanRepayConfirm;

  /// No description provided for @loanReceived.
  ///
  /// In en, this message translates to:
  /// **'Received ({currency})'**
  String loanReceived(String currency);

  /// No description provided for @loanReceivedHelp.
  ///
  /// In en, this message translates to:
  /// **'0 or empty for a write-off, with no money coming in.'**
  String get loanReceivedHelp;

  /// No description provided for @loanWrittenOff.
  ///
  /// In en, this message translates to:
  /// **'Written off the balance'**
  String get loanWrittenOff;

  /// No description provided for @loanWrittenOffHelp.
  ///
  /// In en, this message translates to:
  /// **'Empty = the same as the amount received.'**
  String get loanWrittenOffHelp;

  /// No description provided for @loanReceivingAccount.
  ///
  /// In en, this message translates to:
  /// **'Account that received it'**
  String get loanReceivingAccount;

  /// No description provided for @loanRepayInvalid.
  ///
  /// In en, this message translates to:
  /// **'Enter the amount written off (greater than zero); the amount received cannot be negative.'**
  String get loanRepayInvalid;

  /// No description provided for @loanDeleteTitle.
  ///
  /// In en, this message translates to:
  /// **'Delete the loan to {name}?'**
  String loanDeleteTitle(String name);

  /// No description provided for @loanDeleteBody.
  ///
  /// In en, this message translates to:
  /// **'Its repayments and the account transactions it generated are also removed.'**
  String get loanDeleteBody;

  /// No description provided for @loanDeleteRepayTitle.
  ///
  /// In en, this message translates to:
  /// **'Delete the repayment?'**
  String get loanDeleteRepayTitle;

  /// No description provided for @loanDeleteRepayBody.
  ///
  /// In en, this message translates to:
  /// **'Repayment of {value} on {date}. The receipt posted to the account is also removed.'**
  String loanDeleteRepayBody(String value, String date);

  /// No description provided for @loanToReceive.
  ///
  /// In en, this message translates to:
  /// **'To receive {value}'**
  String loanToReceive(String value);

  /// No description provided for @loanOutsideNetWorth.
  ///
  /// In en, this message translates to:
  /// **' (outside net worth)'**
  String get loanOutsideNetWorth;

  /// No description provided for @loanPrincipalOn.
  ///
  /// In en, this message translates to:
  /// **'Principal {value} on {date}'**
  String loanPrincipalOn(String value, String date);

  /// No description provided for @loanInterest.
  ///
  /// In en, this message translates to:
  /// **'Accrued interest {value}'**
  String loanInterest(String value);

  /// No description provided for @loanExtraMore.
  ///
  /// In en, this message translates to:
  /// **'Received more: {value}'**
  String loanExtraMore(String value);

  /// No description provided for @loanExtraLess.
  ///
  /// In en, this message translates to:
  /// **'Received less: {value}'**
  String loanExtraLess(String value);

  /// No description provided for @loanRate.
  ///
  /// In en, this message translates to:
  /// **'Rate {rate}% per month (simple interest)'**
  String loanRate(String rate);

  /// No description provided for @loanNoRepayments.
  ///
  /// In en, this message translates to:
  /// **'No repayments yet.'**
  String get loanNoRepayments;

  /// No description provided for @loanRepaymentLine.
  ///
  /// In en, this message translates to:
  /// **'{date} · received {received} · written off {writtenOff}'**
  String loanRepaymentLine(String date, String received, String writtenOff);

  /// No description provided for @catTitle.
  ///
  /// In en, this message translates to:
  /// **'Categories'**
  String get catTitle;

  /// No description provided for @catFab.
  ///
  /// In en, this message translates to:
  /// **'Category'**
  String get catFab;

  /// No description provided for @catEmpty.
  ///
  /// In en, this message translates to:
  /// **'No categories yet.'**
  String get catEmpty;

  /// No description provided for @catNew.
  ///
  /// In en, this message translates to:
  /// **'New category'**
  String get catNew;

  /// No description provided for @catEdit.
  ///
  /// In en, this message translates to:
  /// **'Edit category'**
  String get catEdit;

  /// No description provided for @catNameHint.
  ///
  /// In en, this message translates to:
  /// **'Groceries'**
  String get catNameHint;

  /// No description provided for @catMonthlyLimit.
  ///
  /// In en, this message translates to:
  /// **'Monthly limit ({base})'**
  String catMonthlyLimit(String base);

  /// No description provided for @catNoLimitHelp.
  ///
  /// In en, this message translates to:
  /// **'Empty = no limit.'**
  String get catNoLimitHelp;

  /// No description provided for @catInvalid.
  ///
  /// In en, this message translates to:
  /// **'Enter the name and, if any, a limit greater than zero.'**
  String get catInvalid;

  /// No description provided for @catDeleteBody.
  ///
  /// In en, this message translates to:
  /// **'Its transactions become uncategorized; none is deleted.'**
  String get catDeleteBody;

  /// No description provided for @catSpentMonth.
  ///
  /// In en, this message translates to:
  /// **'Spent this month: {value}'**
  String catSpentMonth(String value);

  /// No description provided for @catSpentOfLimit.
  ///
  /// In en, this message translates to:
  /// **'{spent} of {limit}'**
  String catSpentOfLimit(String spent, String limit);

  /// No description provided for @catOver.
  ///
  /// In en, this message translates to:
  /// **'Over by {value}'**
  String catOver(String value);

  /// No description provided for @catMissingFx.
  ///
  /// In en, this message translates to:
  /// **'{count} expense(s) left out of the sum for lack of exchange rate.'**
  String catMissingFx(int count);

  /// No description provided for @currencyUsd.
  ///
  /// In en, this message translates to:
  /// **'US dollar'**
  String get currencyUsd;

  /// No description provided for @currencyEur.
  ///
  /// In en, this message translates to:
  /// **'Euro'**
  String get currencyEur;

  /// No description provided for @currencyGbp.
  ///
  /// In en, this message translates to:
  /// **'Pound sterling'**
  String get currencyGbp;

  /// No description provided for @currencyJpy.
  ///
  /// In en, this message translates to:
  /// **'Yen'**
  String get currencyJpy;

  /// No description provided for @currencyChf.
  ///
  /// In en, this message translates to:
  /// **'Swiss franc'**
  String get currencyChf;

  /// No description provided for @currencyCad.
  ///
  /// In en, this message translates to:
  /// **'Canadian dollar'**
  String get currencyCad;

  /// No description provided for @currencyAud.
  ///
  /// In en, this message translates to:
  /// **'Australian dollar'**
  String get currencyAud;

  /// No description provided for @currencyNzd.
  ///
  /// In en, this message translates to:
  /// **'New Zealand dollar'**
  String get currencyNzd;

  /// No description provided for @currencyCny.
  ///
  /// In en, this message translates to:
  /// **'Yuan'**
  String get currencyCny;

  /// No description provided for @currencyBrl.
  ///
  /// In en, this message translates to:
  /// **'Brazilian real'**
  String get currencyBrl;

  /// No description provided for @currencyArs.
  ///
  /// In en, this message translates to:
  /// **'Argentine peso'**
  String get currencyArs;

  /// No description provided for @currencyUyu.
  ///
  /// In en, this message translates to:
  /// **'Uruguayan peso'**
  String get currencyUyu;

  /// No description provided for @currencyPyg.
  ///
  /// In en, this message translates to:
  /// **'Guarani'**
  String get currencyPyg;

  /// No description provided for @currencyClp.
  ///
  /// In en, this message translates to:
  /// **'Chilean peso'**
  String get currencyClp;

  /// No description provided for @currencyCop.
  ///
  /// In en, this message translates to:
  /// **'Colombian peso'**
  String get currencyCop;

  /// No description provided for @currencyPen.
  ///
  /// In en, this message translates to:
  /// **'Peruvian sol'**
  String get currencyPen;

  /// No description provided for @currencyMxn.
  ///
  /// In en, this message translates to:
  /// **'Mexican peso'**
  String get currencyMxn;

  /// No description provided for @finErrUnknownCurrency.
  ///
  /// In en, this message translates to:
  /// **'Currency not in the catalog: {currency}'**
  String finErrUnknownCurrency(String currency);

  /// No description provided for @finErrAmountMustBePositive.
  ///
  /// In en, this message translates to:
  /// **'The amount must be greater than zero'**
  String get finErrAmountMustBePositive;

  /// No description provided for @finErrInvalidDate.
  ///
  /// In en, this message translates to:
  /// **'Invalid date: {date}'**
  String finErrInvalidDate(String date);

  /// No description provided for @finErrManualTypeOnly.
  ///
  /// In en, this message translates to:
  /// **'Only income and expense are entered by hand; a transfer is created with Transfer, a debt installment by creating a debt, and a loan by granting a loan'**
  String get finErrManualTypeOnly;

  /// No description provided for @finErrLiabilityNameRequired.
  ///
  /// In en, this message translates to:
  /// **'Enter the debt name'**
  String get finErrLiabilityNameRequired;

  /// No description provided for @finErrBorrowerNameRequired.
  ///
  /// In en, this message translates to:
  /// **'Enter the borrower\'s name'**
  String get finErrBorrowerNameRequired;

  /// No description provided for @finErrPrincipalMustBePositive.
  ///
  /// In en, this message translates to:
  /// **'The principal must be greater than zero'**
  String get finErrPrincipalMustBePositive;

  /// No description provided for @finErrTermMustBePositive.
  ///
  /// In en, this message translates to:
  /// **'The term (in months) must be greater than zero'**
  String get finErrTermMustBePositive;

  /// No description provided for @finErrNegativeMonthlyRate.
  ///
  /// In en, this message translates to:
  /// **'The monthly rate can\'t be negative'**
  String get finErrNegativeMonthlyRate;

  /// No description provided for @finErrRepaidAmountMustBePositive.
  ///
  /// In en, this message translates to:
  /// **'The amount repaid must be greater than zero'**
  String get finErrRepaidAmountMustBePositive;

  /// No description provided for @finErrReceivedAmountNegative.
  ///
  /// In en, this message translates to:
  /// **'The amount received can\'t be negative'**
  String get finErrReceivedAmountNegative;

  /// No description provided for @finErrLoanCurrencyMismatch.
  ///
  /// In en, this message translates to:
  /// **'The loan is in {loanCurrency} and the account in {accountCurrency}'**
  String finErrLoanCurrencyMismatch(
    String loanCurrency,
    String accountCurrency,
  );

  /// No description provided for @finErrCategoryNameRequired.
  ///
  /// In en, this message translates to:
  /// **'Enter the category name'**
  String get finErrCategoryNameRequired;

  /// No description provided for @finErrCategoryLimitMustBePositive.
  ///
  /// In en, this message translates to:
  /// **'The monthly limit must be greater than zero'**
  String get finErrCategoryLimitMustBePositive;

  /// No description provided for @finErrTransferAmountsMustBePositive.
  ///
  /// In en, this message translates to:
  /// **'The amounts must be greater than zero'**
  String get finErrTransferAmountsMustBePositive;

  /// No description provided for @finErrTransferSameAccount.
  ///
  /// In en, this message translates to:
  /// **'Choose two different accounts'**
  String get finErrTransferSameAccount;

  /// No description provided for @finErrTransferSameCurrencyAmountsDiffer.
  ///
  /// In en, this message translates to:
  /// **'Accounts in the same currency receive exactly what left'**
  String get finErrTransferSameCurrencyAmountsDiffer;

  /// No description provided for @finErrAccountNotFound.
  ///
  /// In en, this message translates to:
  /// **'Account not found'**
  String get finErrAccountNotFound;

  /// No description provided for @finErrAccountCurrencyLocked.
  ///
  /// In en, this message translates to:
  /// **'The currency can\'t change on an account that already has transactions'**
  String get finErrAccountCurrencyLocked;

  /// No description provided for @finErrBorrowerHasLoans.
  ///
  /// In en, this message translates to:
  /// **'This borrower still has loans: delete them first'**
  String get finErrBorrowerHasLoans;

  /// No description provided for @finErrBorrowerNotFound.
  ///
  /// In en, this message translates to:
  /// **'Borrower not found'**
  String get finErrBorrowerNotFound;

  /// No description provided for @finErrLoanNotFound.
  ///
  /// In en, this message translates to:
  /// **'Loan not found'**
  String get finErrLoanNotFound;

  /// No description provided for @finErrLinkedTransactionLocked.
  ///
  /// In en, this message translates to:
  /// **'A debt installment and a loan transaction are only deleted together with the debt or the loan'**
  String get finErrLinkedTransactionLocked;
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
