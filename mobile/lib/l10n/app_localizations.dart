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
