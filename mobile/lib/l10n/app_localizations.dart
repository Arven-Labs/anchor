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

  /// No description provided for @cryptoTaxTitle.
  ///
  /// In en, this message translates to:
  /// **'Crypto tax'**
  String get cryptoTaxTitle;

  /// No description provided for @cryptoTaxDisclaimer.
  ///
  /// In en, this message translates to:
  /// **'Calculated only from the transactions entered in this app, at average cost and with the fee entered on each operation. The rules come from secondary sources: check with the tax authority (GCAP) before paying.'**
  String get cryptoTaxDisclaimer;

  /// No description provided for @cryptoTaxUsdNote.
  ///
  /// In en, this message translates to:
  /// **'USD operations converted at the dollar rate of the day (Yahoo, USDBRL=X).'**
  String get cryptoTaxUsdNote;

  /// No description provided for @cryptoTaxFxUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Exchange rate unavailable right now: the USD operations were left out. Try again in a moment.'**
  String get cryptoTaxFxUnavailable;

  /// No description provided for @cryptoTaxEmpty.
  ///
  /// In en, this message translates to:
  /// **'No crypto sales in reais yet.'**
  String get cryptoTaxEmpty;

  /// No description provided for @cryptoTaxMonthSales.
  ///
  /// In en, this message translates to:
  /// **'Month sales: {value}'**
  String cryptoTaxMonthSales(String value);

  /// No description provided for @cryptoTaxMonthGain.
  ///
  /// In en, this message translates to:
  /// **'Month gain: {value}'**
  String cryptoTaxMonthGain(String value);

  /// No description provided for @cryptoTaxTax.
  ///
  /// In en, this message translates to:
  /// **'Tax: {value} (rate {rate}%)'**
  String cryptoTaxTax(String value, String rate);

  /// No description provided for @cryptoTaxCarriedIn.
  ///
  /// In en, this message translates to:
  /// **'Carried from earlier months: {value}'**
  String cryptoTaxCarriedIn(String value);

  /// No description provided for @cryptoTaxSalesCount.
  ///
  /// In en, this message translates to:
  /// **'Sales ({count})'**
  String cryptoTaxSalesCount(int count);

  /// No description provided for @cryptoTaxSaleTitle.
  ///
  /// In en, this message translates to:
  /// **'{ticker} — {date}'**
  String cryptoTaxSaleTitle(String ticker, String date);

  /// No description provided for @cryptoTaxSaleDetail.
  ///
  /// In en, this message translates to:
  /// **'Sale {sale}{fee} · cost {cost} · gain {gain}'**
  String cryptoTaxSaleDetail(String sale, String fee, String cost, String gain);

  /// No description provided for @cryptoTaxSaleFee.
  ///
  /// In en, this message translates to:
  /// **' (fee {value})'**
  String cryptoTaxSaleFee(String value);

  /// No description provided for @cryptoTaxStatusExempt.
  ///
  /// In en, this message translates to:
  /// **'Exempt: sales up to {limit} in the month'**
  String cryptoTaxStatusExempt(String limit);

  /// No description provided for @cryptoTaxStatusDarf.
  ///
  /// In en, this message translates to:
  /// **'DARF {code} of {value} due {date}'**
  String cryptoTaxStatusDarf(int code, String value, String date);

  /// No description provided for @cryptoTaxStatusBelowMinimum.
  ///
  /// In en, this message translates to:
  /// **'Tax of {value} is below the {minimum} minimum: added to next month'**
  String cryptoTaxStatusBelowMinimum(String value, String minimum);

  /// No description provided for @cryptoTaxStatusNoTax.
  ///
  /// In en, this message translates to:
  /// **'No tax: the month closed with no gain'**
  String get cryptoTaxStatusNoTax;

  /// No description provided for @cryptoTaxWarnCostUnknown.
  ///
  /// In en, this message translates to:
  /// **'Sale of {ticker} on {date} exceeds the purchases entered: the gain counts only the part with known cost. Enter the missing purchases.'**
  String cryptoTaxWarnCostUnknown(String ticker, String date);

  /// No description provided for @cryptoTaxWarnSalesNonBrl.
  ///
  /// In en, this message translates to:
  /// **'Crypto sales not in BRL in {period} are not assessed here (foreign rules).'**
  String cryptoTaxWarnSalesNonBrl(String period);

  /// No description provided for @cryptoTaxWarnFxMissing.
  ///
  /// In en, this message translates to:
  /// **'USD crypto sale in {period} has no exchange rate to convert: left out of the assessment.'**
  String cryptoTaxWarnFxMissing(String period);

  /// No description provided for @swapIntro.
  ///
  /// In en, this message translates to:
  /// **'Swapping one coin for another counts as a sale of the coin given up for tax purposes: it enters the month\'s sales (R\$ 35 thousand limit) and realizes a gain over the average cost. It records one sale and one purchase for the same value.'**
  String get swapIntro;

  /// No description provided for @swapNeedTwoCoins.
  ///
  /// In en, this message translates to:
  /// **'You need a balance in one crypto and another one registered.'**
  String get swapNeedTwoCoins;

  /// No description provided for @swapCoinOut.
  ///
  /// In en, this message translates to:
  /// **'Coin out'**
  String get swapCoinOut;

  /// No description provided for @swapCoinOutItem.
  ///
  /// In en, this message translates to:
  /// **'{ticker} (balance {balance})'**
  String swapCoinOutItem(String ticker, String balance);

  /// No description provided for @swapQuantityOut.
  ///
  /// In en, this message translates to:
  /// **'Quantity out'**
  String get swapQuantityOut;

  /// No description provided for @swapCoinIn.
  ///
  /// In en, this message translates to:
  /// **'Coin in'**
  String get swapCoinIn;

  /// No description provided for @swapQuantityIn.
  ///
  /// In en, this message translates to:
  /// **'Quantity in'**
  String get swapQuantityIn;

  /// No description provided for @swapMarketValue.
  ///
  /// In en, this message translates to:
  /// **'Market value of the swap (BRL)'**
  String get swapMarketValue;

  /// No description provided for @swapUseMarketPrice.
  ///
  /// In en, this message translates to:
  /// **'Use market price'**
  String get swapUseMarketPrice;

  /// No description provided for @swapSearching.
  ///
  /// In en, this message translates to:
  /// **'Searching…'**
  String get swapSearching;

  /// No description provided for @swapFee.
  ///
  /// In en, this message translates to:
  /// **'Fee (optional, BRL)'**
  String get swapFee;

  /// No description provided for @swapCustody.
  ///
  /// In en, this message translates to:
  /// **'Custody (optional)'**
  String get swapCustody;

  /// No description provided for @swapNoCustody.
  ///
  /// In en, this message translates to:
  /// **'None'**
  String get swapNoCustody;

  /// No description provided for @swapDate.
  ///
  /// In en, this message translates to:
  /// **'Date: {date}'**
  String swapDate(String date);

  /// No description provided for @swapRegister.
  ///
  /// In en, this message translates to:
  /// **'Record swap'**
  String get swapRegister;

  /// No description provided for @swapHintNoBrlQuote.
  ///
  /// In en, this message translates to:
  /// **'No quote in reais for {ticker}.'**
  String swapHintNoBrlQuote(String ticker);

  /// No description provided for @swapHintTodayPrice.
  ///
  /// In en, this message translates to:
  /// **'Today\'s price ({date}); if the swap was on another date, adjust the value.'**
  String swapHintTodayPrice(String date);

  /// No description provided for @swapHintQuoteUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Quote unavailable right now; enter the value.'**
  String get swapHintQuoteUnavailable;

  /// No description provided for @swapHintNoQuote.
  ///
  /// In en, this message translates to:
  /// **'No quote for {ticker}; enter the value.'**
  String swapHintNoQuote(String ticker);

  /// No description provided for @swapFeeNotNumber.
  ///
  /// In en, this message translates to:
  /// **'The fee must be a number.'**
  String get swapFeeNotNumber;

  /// No description provided for @swapErrNeedTwoDifferentCoins.
  ///
  /// In en, this message translates to:
  /// **'A swap needs two different coins.'**
  String get swapErrNeedTwoDifferentCoins;

  /// No description provided for @swapErrNotCrypto.
  ///
  /// In en, this message translates to:
  /// **'{ticker} is not crypto; a swap only works between cryptocurrencies.'**
  String swapErrNotCrypto(String ticker);

  /// No description provided for @swapErrNotBrl.
  ///
  /// In en, this message translates to:
  /// **'{ticker} is in {currency}; swaps are assessed in reais.'**
  String swapErrNotBrl(String ticker, String currency);

  /// No description provided for @swapErrQuantityOutMustBePositive.
  ///
  /// In en, this message translates to:
  /// **'The quantity out must be greater than zero.'**
  String get swapErrQuantityOutMustBePositive;

  /// No description provided for @swapErrQuantityInMustBePositive.
  ///
  /// In en, this message translates to:
  /// **'The quantity in must be greater than zero.'**
  String get swapErrQuantityInMustBePositive;

  /// No description provided for @swapErrValueMustBePositive.
  ///
  /// In en, this message translates to:
  /// **'The value must be greater than zero.'**
  String get swapErrValueMustBePositive;

  /// No description provided for @swapErrNegativeFee.
  ///
  /// In en, this message translates to:
  /// **'The fee can\'t be negative.'**
  String get swapErrNegativeFee;

  /// No description provided for @swapErrInsufficientBalance.
  ///
  /// In en, this message translates to:
  /// **'Insufficient {ticker} balance: you have {available} and the swap gives {requested}.'**
  String swapErrInsufficientBalance(
    String ticker,
    String available,
    String requested,
  );

  /// No description provided for @fundTaxTitle.
  ///
  /// In en, this message translates to:
  /// **'Fund tax'**
  String get fundTaxTitle;

  /// No description provided for @fundTaxDisclaimer.
  ///
  /// In en, this message translates to:
  /// **'Estimate based on the transactions and quotas entered in this app. The semiannual quota tax and the redemption tax are withheld at the source by the fund: there is no DARF. The rules come from secondary sources; check with the tax authority and the fund statement.'**
  String get fundTaxDisclaimer;

  /// No description provided for @fundRefreshCvm.
  ///
  /// In en, this message translates to:
  /// **'Update from CVM'**
  String get fundRefreshCvm;

  /// No description provided for @fundCvmDownloadWarning.
  ///
  /// In en, this message translates to:
  /// **'Downloads a file of up to ~12 MB for each month read (usually 4 months, about 40 MB). Continue?'**
  String get fundCvmDownloadWarning;

  /// No description provided for @fundDownload.
  ///
  /// In en, this message translates to:
  /// **'Download'**
  String get fundDownload;

  /// No description provided for @fundProgressPreparing.
  ///
  /// In en, this message translates to:
  /// **'Preparing…'**
  String get fundProgressPreparing;

  /// No description provided for @fundProgressReading.
  ///
  /// In en, this message translates to:
  /// **'Reading month {done} of {total}…'**
  String fundProgressReading(int done, int total);

  /// No description provided for @fundCvmReadFailed.
  ///
  /// In en, this message translates to:
  /// **'Could not read the CVM: {error}'**
  String fundCvmReadFailed(String error);

  /// No description provided for @fundLossCreditsHeader.
  ///
  /// In en, this message translates to:
  /// **'Loss to offset against later gains and semiannual quota tax (same administrator and same type)'**
  String get fundLossCreditsHeader;

  /// No description provided for @fundLossCreditLine.
  ///
  /// In en, this message translates to:
  /// **'{amount} — {tickers} ({administrator}, {term})'**
  String fundLossCreditLine(
    String amount,
    String tickers,
    String administrator,
    String term,
  );

  /// No description provided for @fundNoAdministrator.
  ///
  /// In en, this message translates to:
  /// **'no administrator'**
  String get fundNoAdministrator;

  /// No description provided for @fundNone.
  ///
  /// In en, this message translates to:
  /// **'No funds registered yet.'**
  String get fundNone;

  /// No description provided for @fundStatusNoPosition.
  ///
  /// In en, this message translates to:
  /// **'No quotas in the portfolio.'**
  String get fundStatusNoPosition;

  /// No description provided for @fundStatusNoTerm.
  ///
  /// In en, this message translates to:
  /// **'Choose the fund type to estimate the semiannual quota tax and the redemption tax.'**
  String get fundStatusNoTerm;

  /// No description provided for @fundStatusNoQuota.
  ///
  /// In en, this message translates to:
  /// **'Enter the quota value to estimate the semiannual quota tax.'**
  String get fundStatusNoQuota;

  /// No description provided for @fundStatusExempt.
  ///
  /// In en, this message translates to:
  /// **'Equity fund or investment entity: no semiannual quota tax.'**
  String get fundStatusExempt;

  /// No description provided for @fundStatusEstimated.
  ///
  /// In en, this message translates to:
  /// **'Next semiannual quota tax on {date}: {tax} ({quotas} quotas)'**
  String fundStatusEstimated(String date, String tax, String quotas);

  /// No description provided for @fundBasis.
  ///
  /// In en, this message translates to:
  /// **'Basis per quota {basis} ({origin}) · gain {gain} × {rate}%'**
  String fundBasis(String basis, String origin, String gain, String rate);

  /// No description provided for @fundBasisLastComeCotas.
  ///
  /// In en, this message translates to:
  /// **'quota value at the last semiannual quota tax'**
  String get fundBasisLastComeCotas;

  /// No description provided for @fundBasisEstoque.
  ///
  /// In en, this message translates to:
  /// **'average cost plus the 2023 stock already taxed'**
  String get fundBasisEstoque;

  /// No description provided for @fundBasisAverageCost.
  ///
  /// In en, this message translates to:
  /// **'average cost'**
  String get fundBasisAverageCost;

  /// No description provided for @fundEstoqueNotTaxed.
  ///
  /// In en, this message translates to:
  /// **'Say how the 2023 stock was taxed to estimate the tax and the cost of the semiannual quota tax.'**
  String get fundEstoqueNotTaxed;

  /// No description provided for @fundEstoqueNeedsQuota.
  ///
  /// In en, this message translates to:
  /// **'Enter the quota of 12/31/2023 (or up to 10 days before) to estimate the stock.'**
  String get fundEstoqueNeedsQuota;

  /// No description provided for @fundEstoqueEstimated.
  ///
  /// In en, this message translates to:
  /// **'2023 stock: gain of {base} up to {date} (quota {quota}) at {rate}% = {tax}'**
  String fundEstoqueEstimated(
    String base,
    String date,
    String quota,
    String rate,
    String tax,
  );

  /// No description provided for @fundRefundable.
  ///
  /// In en, this message translates to:
  /// **'Semiannual quota tax paid above the tax due: {total}. The law does not provide a refund of this excess (IN RFB 1.585/2015, art. 15; Law 14.754/2023, art. 17, §6). It is tax paid on income that later disappeared, and it comes back as a loss to offset against later gains. The app already adds this loss to the loss credit; the excess itself is not included in the net amounts.'**
  String fundRefundable(String total);

  /// No description provided for @fundWarnStaleQuota.
  ///
  /// In en, this message translates to:
  /// **'The latest quota value is more than {days} days old: the estimate may be out of date.'**
  String fundWarnStaleQuota(int days);

  /// No description provided for @fundWarnUnrecordedDeduction.
  ///
  /// In en, this message translates to:
  /// **'The semiannual quota tax of {date} has already passed with no deduction recorded: enter the Semiannual quota tax transaction so the basis is right.'**
  String fundWarnUnrecordedDeduction(String date);

  /// No description provided for @fundWarnCreditNotInformed.
  ///
  /// In en, this message translates to:
  /// **'The semiannual quota tax of {date} was recorded with {balance} of loss credit in the group: the administrator may have used part of it, so the credit balance shown may be above the real one. Fill in the loss credit used in that transaction to resolve this.'**
  String fundWarnCreditNotInformed(String date, String balance);

  /// No description provided for @fundWarnCreditExceedsBalance.
  ///
  /// In en, this message translates to:
  /// **'The semiannual quota tax of {date} says {informed} of loss credit was used, but the group only had {balance}: only that balance was deducted. Check the amount.'**
  String fundWarnCreditExceedsBalance(
    String date,
    String informed,
    String balance,
  );

  /// No description provided for @fundWarnRedemptionUncovered.
  ///
  /// In en, this message translates to:
  /// **'Redemption of {date} is larger than the investments entered: the cost counts only the known part.'**
  String fundWarnRedemptionUncovered(String date);

  /// No description provided for @fundWarnRedemptionExcess.
  ///
  /// In en, this message translates to:
  /// **'In the redemption of {date} the semiannual quota tax paid exceeded the tax due by {amount}: it stays out of the net amount (see the note about the excess).'**
  String fundWarnRedemptionExcess(String date, String amount);

  /// No description provided for @fundCreditReduced.
  ///
  /// In en, this message translates to:
  /// **'Loss credit of {used} has already reduced the gain: the tax would be {before} without it.'**
  String fundCreditReduced(String used, String before);

  /// No description provided for @fundRedemptionCreditLine.
  ///
  /// In en, this message translates to:
  /// **'Losses from earlier redemptions in the group offset {used} of the gain: tax {ir} instead of {before}.'**
  String fundRedemptionCreditLine(String used, String ir, String before);

  /// No description provided for @fundRedemptionLossLine.
  ///
  /// In en, this message translates to:
  /// **'This redemption lost {loss}: it offsets the gain of later redemptions and semiannual quota tax in the same group (same administrator and same type).'**
  String fundRedemptionLossLine(String loss);

  /// No description provided for @fundRecordHead.
  ///
  /// In en, this message translates to:
  /// **'All the funds you entered from administrator {administrator} were redeemed by {date}.'**
  String fundRecordHead(String administrator, String date);

  /// No description provided for @fundRecordExpired.
  ///
  /// In en, this message translates to:
  /// **'{head} The administrator\'s obligation to keep the loss record ({balance}) ended on {until} (IN RFB 1.585, art. 15 §2): check that it still has the record before counting on the balance.'**
  String fundRecordExpired(String head, String balance, String until);

  /// No description provided for @fundRecordActive.
  ///
  /// In en, this message translates to:
  /// **'{head} The administrator is only required to keep the loss record ({balance}) until {until} (IN RFB 1.585, art. 15 §2). The law does not limit your right to offset, but check with the administrator.'**
  String fundRecordActive(String head, String balance, String until);

  /// No description provided for @fundCvmLatest.
  ///
  /// In en, this message translates to:
  /// **' Last CVM quota: {date}.'**
  String fundCvmLatest(String date);

  /// No description provided for @fundCvmRead.
  ///
  /// In en, this message translates to:
  /// **'Quotas read from the CVM.{latest}'**
  String fundCvmRead(String latest);

  /// No description provided for @fundCvmNoCnpj.
  ///
  /// In en, this message translates to:
  /// **'Enter the fund\'s CNPJ to read the quota from the CVM.'**
  String get fundCvmNoCnpj;

  /// No description provided for @fundCvmPending.
  ///
  /// In en, this message translates to:
  /// **'Not read yet.{latest}'**
  String fundCvmPending(String latest);

  /// No description provided for @fundCvmNotFound.
  ///
  /// In en, this message translates to:
  /// **'The CVM returned no quota for this CNPJ (or subclass) in the months read. Check the CNPJ.'**
  String get fundCvmNotFound;

  /// No description provided for @fundCvmNeedsSubclass.
  ///
  /// In en, this message translates to:
  /// **'This CNPJ only has subclasses in the report: choose one.'**
  String get fundCvmNeedsSubclass;

  /// No description provided for @fundCvmError.
  ///
  /// In en, this message translates to:
  /// **'Could not read the CVM right now ({message}). Try again in a moment.{latest}'**
  String fundCvmError(String message, String latest);

  /// No description provided for @fundCvmErrorDefault.
  ///
  /// In en, this message translates to:
  /// **'error'**
  String get fundCvmErrorDefault;

  /// No description provided for @fundTermLabel.
  ///
  /// In en, this message translates to:
  /// **'Fund type'**
  String get fundTermLabel;

  /// No description provided for @fundStructureLabel.
  ///
  /// In en, this message translates to:
  /// **'Fund structure (Law 14.754)'**
  String get fundStructureLabel;

  /// No description provided for @fundEstoqueLabel.
  ///
  /// In en, this message translates to:
  /// **'2023 stock (arts. 27 and 28)'**
  String get fundEstoqueLabel;

  /// No description provided for @fundEstoqueNotInformed.
  ///
  /// In en, this message translates to:
  /// **'Not taxed / not informed'**
  String get fundEstoqueNotInformed;

  /// No description provided for @fundAdministratorLabel.
  ///
  /// In en, this message translates to:
  /// **'Administrator / intermediary (offsets loss)'**
  String get fundAdministratorLabel;

  /// No description provided for @fundAdministratorHint.
  ///
  /// In en, this message translates to:
  /// **'e.g. XP, BTG'**
  String get fundAdministratorHint;

  /// No description provided for @fundAdministratorHelpNoTerm.
  ///
  /// In en, this message translates to:
  /// **'Choose the fund type first.'**
  String get fundAdministratorHelpNoTerm;

  /// No description provided for @fundAdministratorHelpNone.
  ///
  /// In en, this message translates to:
  /// **'Without this, the fund only offsets losses against itself.'**
  String get fundAdministratorHelpNone;

  /// No description provided for @fundAdministratorHelpSet.
  ///
  /// In en, this message translates to:
  /// **'Funds of the same type and the same administrator offset each other.'**
  String get fundAdministratorHelpSet;

  /// No description provided for @fundSaveAdministrator.
  ///
  /// In en, this message translates to:
  /// **'Save administrator'**
  String get fundSaveAdministrator;

  /// No description provided for @fundCnpjLabel.
  ///
  /// In en, this message translates to:
  /// **'Fund CNPJ (quota from the CVM)'**
  String get fundCnpjLabel;

  /// No description provided for @fundSaveCnpj.
  ///
  /// In en, this message translates to:
  /// **'Save CNPJ'**
  String get fundSaveCnpj;

  /// No description provided for @fundSubclassLabel.
  ///
  /// In en, this message translates to:
  /// **'CVM subclass'**
  String get fundSubclassLabel;

  /// No description provided for @fundSubclassNone.
  ///
  /// In en, this message translates to:
  /// **'Class (no subclass)'**
  String get fundSubclassNone;

  /// No description provided for @fundQuotasTitle.
  ///
  /// In en, this message translates to:
  /// **'Quota value ({count})'**
  String fundQuotasTitle(int count);

  /// No description provided for @fundDeleteQuota.
  ///
  /// In en, this message translates to:
  /// **'Delete quota'**
  String get fundDeleteQuota;

  /// No description provided for @fundAddQuota.
  ///
  /// In en, this message translates to:
  /// **'Enter quota'**
  String get fundAddQuota;

  /// No description provided for @fundDeductionsTitle.
  ///
  /// In en, this message translates to:
  /// **'Semiannual quota tax recorded ({count})'**
  String fundDeductionsTitle(int count);

  /// No description provided for @fundDeductionLine.
  ///
  /// In en, this message translates to:
  /// **'{quantity} quotas at {price} · tax {tax}{credit}'**
  String fundDeductionLine(
    String quantity,
    String price,
    String tax,
    String credit,
  );

  /// No description provided for @fundDeductionCredit.
  ///
  /// In en, this message translates to:
  /// **' · credit used {value}'**
  String fundDeductionCredit(String value);

  /// No description provided for @fundRedemptionsTitle.
  ///
  /// In en, this message translates to:
  /// **'Estimated redemptions and amortizations ({count})'**
  String fundRedemptionsTitle(int count);

  /// No description provided for @fundRedemptionAmortization.
  ///
  /// In en, this message translates to:
  /// **'{date} — amortization (quota at {quota}; the quotas stay)'**
  String fundRedemptionAmortization(String date, String quota);

  /// No description provided for @fundRedemptionTitle.
  ///
  /// In en, this message translates to:
  /// **'{date} — {quotas} quotas at {quota}'**
  String fundRedemptionTitle(String date, String quotas, String quota);

  /// No description provided for @fundRedemptionDetail.
  ///
  /// In en, this message translates to:
  /// **'Gross {gross} · IOF {iof} · tax {ir} · net {net}'**
  String fundRedemptionDetail(String gross, String iof, String ir, String net);

  /// No description provided for @fundRedemptionComeCotasCredit.
  ///
  /// In en, this message translates to:
  /// **'Semiannual quota tax already paid offset {value} of the tax'**
  String fundRedemptionComeCotasCredit(String value);

  /// No description provided for @fundQuotaDialogTitle.
  ///
  /// In en, this message translates to:
  /// **'Quota of {ticker}'**
  String fundQuotaDialogTitle(String ticker);

  /// No description provided for @fundQuotaValueLabel.
  ///
  /// In en, this message translates to:
  /// **'Quota value (R\$)'**
  String get fundQuotaValueLabel;

  /// No description provided for @fundDate.
  ///
  /// In en, this message translates to:
  /// **'Date: {date}'**
  String fundDate(String date);

  /// No description provided for @fundTermLongTerm.
  ///
  /// In en, this message translates to:
  /// **'Long term'**
  String get fundTermLongTerm;

  /// No description provided for @fundTermShortTerm.
  ///
  /// In en, this message translates to:
  /// **'Short term'**
  String get fundTermShortTerm;

  /// No description provided for @fundTermEquity.
  ///
  /// In en, this message translates to:
  /// **'Equity'**
  String get fundTermEquity;

  /// No description provided for @fundTermInvestmentEntity.
  ///
  /// In en, this message translates to:
  /// **'Investment entity'**
  String get fundTermInvestmentEntity;

  /// No description provided for @fundStructureOpen.
  ///
  /// In en, this message translates to:
  /// **'Open'**
  String get fundStructureOpen;

  /// No description provided for @fundStructureClosed.
  ///
  /// In en, this message translates to:
  /// **'Closed'**
  String get fundStructureClosed;

  /// No description provided for @fundStructureExclusive.
  ///
  /// In en, this message translates to:
  /// **'Exclusive'**
  String get fundStructureExclusive;

  /// No description provided for @fundEstoqueGeneral.
  ///
  /// In en, this message translates to:
  /// **'General rule (15%)'**
  String get fundEstoqueGeneral;

  /// No description provided for @fundEstoqueOption8.
  ///
  /// In en, this message translates to:
  /// **'Option (8%)'**
  String get fundEstoqueOption8;

  /// No description provided for @fundErrNeedTermBeforeStructure.
  ///
  /// In en, this message translates to:
  /// **'Choose the fund type before the structure'**
  String get fundErrNeedTermBeforeStructure;

  /// No description provided for @fundErrNeedTermBeforeEstoque.
  ///
  /// In en, this message translates to:
  /// **'Choose the fund type before the 2023 stock'**
  String get fundErrNeedTermBeforeEstoque;

  /// No description provided for @fundErrNeedTermBeforeAdministrator.
  ///
  /// In en, this message translates to:
  /// **'Choose the fund type before the administrator'**
  String get fundErrNeedTermBeforeAdministrator;

  /// No description provided for @fundErrNeedTermBeforeSubclass.
  ///
  /// In en, this message translates to:
  /// **'Choose the fund type before the CVM subclass'**
  String get fundErrNeedTermBeforeSubclass;

  /// No description provided for @fundErrCnpjMustHave14Digits.
  ///
  /// In en, this message translates to:
  /// **'The CNPJ must have 14 digits'**
  String get fundErrCnpjMustHave14Digits;

  /// No description provided for @foreignTitle.
  ///
  /// In en, this message translates to:
  /// **'Foreign tax'**
  String get foreignTitle;

  /// No description provided for @foreignIntroNote.
  ///
  /// In en, this message translates to:
  /// **'Calculated only from the transactions entered in this app, at average cost, with each operation\'s fee and in reais using the Central Bank PTAX (closing, selling rate; buying rate for tax withheld abroad), and on a day with no quote using the previous one. Where the PTAX is missing, the market rate (Yahoo) applies and the year warns that it is approximate. It goes in the annual return, with no monthly DARF. The rules come from secondary sources: check with the tax authority before paying.'**
  String get foreignIntroNote;

  /// No description provided for @foreignFxUnavailableNote.
  ///
  /// In en, this message translates to:
  /// **'Exchange rate unavailable right now for some currency (neither PTAX nor market): its operations were left out. Try again in a moment.'**
  String get foreignFxUnavailableNote;

  /// No description provided for @foreignEmpty.
  ///
  /// In en, this message translates to:
  /// **'No sales or dividends from foreign assets from 2024 on yet.'**
  String get foreignEmpty;

  /// No description provided for @foreignStatusTaxDue.
  ///
  /// In en, this message translates to:
  /// **'Estimated tax of {value} on the return'**
  String foreignStatusTaxDue(String value);

  /// No description provided for @foreignStatusCovered.
  ///
  /// In en, this message translates to:
  /// **'No tax due: the tax withheld abroad covers the {value} owed'**
  String foreignStatusCovered(String value);

  /// No description provided for @foreignStatusLoss.
  ///
  /// In en, this message translates to:
  /// **'No tax: the year closed with a loss of {value}, which offsets the following years'**
  String foreignStatusLoss(String value);

  /// No description provided for @foreignStatusFitInLoss.
  ///
  /// In en, this message translates to:
  /// **'No tax: the year\'s result fit within the loss from earlier years'**
  String get foreignStatusFitInLoss;

  /// No description provided for @foreignRatePtax.
  ///
  /// In en, this message translates to:
  /// **'Rate {rate} (PTAX)'**
  String foreignRatePtax(String rate);

  /// No description provided for @foreignRateMarket.
  ///
  /// In en, this message translates to:
  /// **'Rate {rate} (market)'**
  String foreignRateMarket(String rate);

  /// No description provided for @foreignWarnCostUnknown.
  ///
  /// In en, this message translates to:
  /// **'Sale of {ticker} on {date} exceeds the purchases entered: the gain counts only the part with known cost. Enter the missing purchases.'**
  String foreignWarnCostUnknown(String ticker, String date);

  /// No description provided for @foreignWarnFxMissing.
  ///
  /// In en, this message translates to:
  /// **'Operations or dividends of {year} have no exchange rate (neither PTAX nor market) for the date: they were left out of the assessment.'**
  String foreignWarnFxMissing(int year);

  /// No description provided for @foreignWarnFxApproximate.
  ///
  /// In en, this message translates to:
  /// **'Operations or dividends of {year} were converted at the market rate because the Central Bank PTAX was not available for them (no network, a currency the PTAX does not quote, or a long gap): the values for the year are approximate.'**
  String foreignWarnFxApproximate(int year);

  /// No description provided for @foreignCapitalGain.
  ///
  /// In en, this message translates to:
  /// **'Capital gain: {value}'**
  String foreignCapitalGain(String value);

  /// No description provided for @foreignDividends.
  ///
  /// In en, this message translates to:
  /// **'Dividends: {value}'**
  String foreignDividends(String value);

  /// No description provided for @foreignLossUsed.
  ///
  /// In en, this message translates to:
  /// **'Loss from earlier years used: {value}'**
  String foreignLossUsed(String value);

  /// No description provided for @foreignTaxableBase.
  ///
  /// In en, this message translates to:
  /// **'Calculation base: {value}'**
  String foreignTaxableBase(String value);

  /// No description provided for @foreignTax.
  ///
  /// In en, this message translates to:
  /// **'Tax: {value} (rate {rate}%)'**
  String foreignTax(String value, String rate);

  /// No description provided for @foreignWithheld.
  ///
  /// In en, this message translates to:
  /// **'Withheld abroad: {value} (credit of {credit})'**
  String foreignWithheld(String value, String credit);

  /// No description provided for @foreignLossCarriedOut.
  ///
  /// In en, this message translates to:
  /// **'Loss that offsets the following years: {value}'**
  String foreignLossCarriedOut(String value);

  /// No description provided for @foreignSalesCount.
  ///
  /// In en, this message translates to:
  /// **'Sales ({count})'**
  String foreignSalesCount(int count);

  /// No description provided for @foreignDividendsCount.
  ///
  /// In en, this message translates to:
  /// **'Dividends ({count})'**
  String foreignDividendsCount(int count);

  /// No description provided for @foreignItemTitle.
  ///
  /// In en, this message translates to:
  /// **'{ticker} — {date}'**
  String foreignItemTitle(String ticker, String date);

  /// No description provided for @foreignSaleDetail.
  ///
  /// In en, this message translates to:
  /// **'{rate} · sale {sale} · cost {cost} · gain {gain}'**
  String foreignSaleDetail(String rate, String sale, String cost, String gain);

  /// No description provided for @foreignDividendDetail.
  ///
  /// In en, this message translates to:
  /// **'{rate} · {gross}{withheld}'**
  String foreignDividendDetail(String rate, String gross, String withheld);

  /// No description provided for @foreignDividendWithheld.
  ///
  /// In en, this message translates to:
  /// **' · withheld {value}'**
  String foreignDividendWithheld(String value);

  /// No description provided for @foreignHoldingsTitle.
  ///
  /// In en, this message translates to:
  /// **'Assets and Rights (Bens e Direitos)'**
  String get foreignHoldingsTitle;

  /// No description provided for @foreignHoldingsNote.
  ///
  /// In en, this message translates to:
  /// **'Acquisition cost in reais, not market value, only of what is entered in this app. One line per custody: the cost of each is its quantity times the asset\'s average cost, so the custodies add up to the position. Group, code and country of the record are yours: fill them in from the Receita program (sources disagree and the app does not check; it only pads zeros, so 3 becomes 03). They go into the copied description and the CSV. Estimate from secondary sources: check in the Receita program.'**
  String get foreignHoldingsNote;

  /// No description provided for @foreignHoldingsLegend.
  ///
  /// In en, this message translates to:
  /// **'\"approximate\": a purchase with no PTAX used the market rate. \"incomplete\": an operation was left with no exchange rate, or a sale was larger than the purchases entered, and the cost is understated.'**
  String get foreignHoldingsLegend;

  /// No description provided for @foreignReturnOf.
  ///
  /// In en, this message translates to:
  /// **'Return of {year}'**
  String foreignReturnOf(int year);

  /// No description provided for @foreignExportCsv.
  ///
  /// In en, this message translates to:
  /// **'Export CSV'**
  String get foreignExportCsv;

  /// No description provided for @foreignNoPosition.
  ///
  /// In en, this message translates to:
  /// **'No foreign position on {previous} or {current}.'**
  String foreignNoPosition(String previous, String current);

  /// No description provided for @foreignNoCustody.
  ///
  /// In en, this message translates to:
  /// **'No custody'**
  String get foreignNoCustody;

  /// No description provided for @foreignHoldingNone.
  ///
  /// In en, this message translates to:
  /// **'{date}: —'**
  String foreignHoldingNone(String date);

  /// No description provided for @foreignHoldingValue.
  ///
  /// In en, this message translates to:
  /// **'{date}: {cost} ({quantity} units){approximate}{incomplete}'**
  String foreignHoldingValue(
    String date,
    String cost,
    String quantity,
    String approximate,
    String incomplete,
  );

  /// No description provided for @foreignApproximate.
  ///
  /// In en, this message translates to:
  /// **' · approximate'**
  String get foreignApproximate;

  /// No description provided for @foreignIncomplete.
  ///
  /// In en, this message translates to:
  /// **' · incomplete'**
  String get foreignIncomplete;

  /// No description provided for @foreignFichaLine.
  ///
  /// In en, this message translates to:
  /// **'Record: {ficha}'**
  String foreignFichaLine(String ficha);

  /// No description provided for @foreignFichaNotFilled.
  ///
  /// In en, this message translates to:
  /// **'not filled in'**
  String get foreignFichaNotFilled;

  /// No description provided for @foreignFichaGroup.
  ///
  /// In en, this message translates to:
  /// **'Group {value}'**
  String foreignFichaGroup(String value);

  /// No description provided for @foreignFichaCode.
  ///
  /// In en, this message translates to:
  /// **'Code {value}'**
  String foreignFichaCode(String value);

  /// No description provided for @foreignFichaCountry.
  ///
  /// In en, this message translates to:
  /// **'Country {value}'**
  String foreignFichaCountry(String value);

  /// No description provided for @foreignEditFichaTooltip.
  ///
  /// In en, this message translates to:
  /// **'Edit the record\'s group, code and country'**
  String get foreignEditFichaTooltip;

  /// No description provided for @foreignCopyTooltip.
  ///
  /// In en, this message translates to:
  /// **'Copy description'**
  String get foreignCopyTooltip;

  /// No description provided for @foreignDescriptionCopied.
  ///
  /// In en, this message translates to:
  /// **'Description copied.'**
  String get foreignDescriptionCopied;

  /// No description provided for @foreignFichaDialogTitle.
  ///
  /// In en, this message translates to:
  /// **'Record of {ticker}'**
  String foreignFichaDialogTitle(String ticker);

  /// No description provided for @foreignFieldGroup.
  ///
  /// In en, this message translates to:
  /// **'Group'**
  String get foreignFieldGroup;

  /// No description provided for @foreignFieldCode.
  ///
  /// In en, this message translates to:
  /// **'Code'**
  String get foreignFieldCode;

  /// No description provided for @foreignFieldCountry.
  ///
  /// In en, this message translates to:
  /// **'Country'**
  String get foreignFieldCountry;

  /// No description provided for @foreignFichaHelp.
  ///
  /// In en, this message translates to:
  /// **'Copy from the Receita program; the app does not check it. Empty clears.'**
  String get foreignFichaHelp;

  /// No description provided for @foreignFichaInvalid.
  ///
  /// In en, this message translates to:
  /// **'Invalid record. Use digits only: group and code up to 2, country up to 3.'**
  String get foreignFichaInvalid;

  /// No description provided for @foreignCsvDialogTitle.
  ///
  /// In en, this message translates to:
  /// **'Save Assets and Rights report'**
  String get foreignCsvDialogTitle;

  /// No description provided for @foreignCsvSaved.
  ///
  /// In en, this message translates to:
  /// **'{count} rows saved.'**
  String foreignCsvSaved(int count);

  /// No description provided for @foreignCsvFailed.
  ///
  /// In en, this message translates to:
  /// **'Could not save the CSV: {error}'**
  String foreignCsvFailed(String error);

  /// No description provided for @foreignDescHead.
  ///
  /// In en, this message translates to:
  /// **'{ticker} — {name}.{custody}'**
  String foreignDescHead(String ticker, String name, String custody);

  /// No description provided for @foreignDescCustody.
  ///
  /// In en, this message translates to:
  /// **' Custody: {label}.'**
  String foreignDescCustody(String label);

  /// No description provided for @foreignDescFicha.
  ///
  /// In en, this message translates to:
  /// **'Record: {ficha}.'**
  String foreignDescFicha(String ficha);

  /// No description provided for @foreignDescPosition.
  ///
  /// In en, this message translates to:
  /// **'Position on {date}: {units}, acquisition cost {cost}.'**
  String foreignDescPosition(String date, String units, String cost);

  /// No description provided for @foreignDescNoPosition.
  ///
  /// In en, this message translates to:
  /// **'no position'**
  String get foreignDescNoPosition;

  /// No description provided for @foreignDescUnits.
  ///
  /// In en, this message translates to:
  /// **'{quantity} units'**
  String foreignDescUnits(String quantity);

  /// No description provided for @foreignDescBasis.
  ///
  /// In en, this message translates to:
  /// **'Acquisition cost converted into reais at the Central Bank\'s closing (selling) rate on the date of each purchase.'**
  String get foreignDescBasis;
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
