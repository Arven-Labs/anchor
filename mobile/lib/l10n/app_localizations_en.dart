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

  @override
  String get navHome => 'Home';

  @override
  String get navQuote => 'Quote';

  @override
  String get navPortfolio => 'Portfolio';

  @override
  String get navFinance => 'Finances';

  @override
  String get navCrypto => 'Crypto';

  @override
  String get homeTitle => 'Home';

  @override
  String get homeCustomizeTitle => 'Customize Home';

  @override
  String get homeCustomizeTooltip => 'Customize';

  @override
  String get homeSettingsTooltip => 'Settings';

  @override
  String get settingsTitle => 'Settings';

  @override
  String get settingsLanguage => 'Language';

  @override
  String get settingsLanguageSystem => 'Follow device';

  @override
  String get homeRestoreDefault => 'Restore default';

  @override
  String get homeWidthFull => 'Full';

  @override
  String get homeWidthHalf => 'Half';

  @override
  String homeSaveFailed(String error) {
    return 'Couldn\'t save the layout: $error';
  }

  @override
  String homeLoadFailed(String error) {
    return 'Couldn\'t load the quotes right now. Pull to refresh. ($error)';
  }

  @override
  String get homeAllHidden =>
      'All cards are hidden. Tap Customize to show them.';

  @override
  String homeTotalMissing(int count) {
    return '$count left out of the sum (no quote or exchange rate)';
  }

  @override
  String homeProfitExcluded(int count) {
    return '$count left out of the calculation';
  }

  @override
  String get homeNoPositions => 'No open positions.';

  @override
  String get homeCash => 'Cash';

  @override
  String get homePortfolio => 'Portfolio';

  @override
  String get homeLoansReceivable => 'Loans';

  @override
  String get homeDebts => 'Debts';

  @override
  String homeNetWorthMissing(int count) {
    return '$count item(s) left out of the sum for lack of exchange rate or quote.';
  }

  @override
  String homeFxUnavailable(String currencies) {
    return 'Exchange rate unavailable right now for $currencies.';
  }

  @override
  String get homeWidgetTotalValue => 'Total value';

  @override
  String get homeWidgetProfit => 'Unrealized profit';

  @override
  String get homeWidgetAllocation => 'Allocation by class';

  @override
  String get homeWidgetNetWorth => 'Net worth';

  @override
  String get assetClassAcaoBr => 'BR stock';

  @override
  String get assetClassFii => 'FII';

  @override
  String get assetClassEtfBr => 'BR ETF';

  @override
  String get assetClassEtfRendaFixa => 'BR fixed-income ETF';

  @override
  String get assetClassBdr => 'BDR';

  @override
  String get assetClassAcaoInternacional => 'International stock';

  @override
  String get assetClassReit => 'REIT';

  @override
  String get assetClassEtfUs => 'US ETF';

  @override
  String get assetClassCripto => 'Crypto';

  @override
  String get assetClassMetal => 'Metal';

  @override
  String get assetClassTesouroDireto => 'Tesouro Direto';

  @override
  String get assetClassRendaFixa => 'Fixed income';

  @override
  String get assetClassImovel => 'Real estate';

  @override
  String get assetClassEmpresaNaoListada => 'Unlisted company';

  @override
  String get assetClassFundo => 'Fund';

  @override
  String get portfolioTitle => 'Portfolio';

  @override
  String get portfolioCryptoTax => 'Crypto tax';

  @override
  String get portfolioForeignTax => 'Foreign tax';

  @override
  String get portfolioFundTax => 'Fund tax';

  @override
  String get portfolioB3Tax => 'B3 tax';

  @override
  String get portfolioSwapCrypto => 'Swap crypto';

  @override
  String get portfolioCustodies => 'Custodians';

  @override
  String get portfolioHistory => 'History';

  @override
  String get portfolioAddAsset => 'Add asset';

  @override
  String get portfolioEmpty => 'No assets yet.';

  @override
  String portfolioUnits(String quantity) {
    return '$quantity units';
  }

  @override
  String portfolioAveragePrice(String value) {
    return 'Average price: $value';
  }

  @override
  String portfolioCurrentPrice(String value) {
    return 'Current price: $value';
  }

  @override
  String portfolioValue(String value) {
    return 'Value: $value';
  }

  @override
  String get portfolioPriceUnavailable => 'unavailable right now';

  @override
  String portfolioShare(String percent, String value) {
    return 'Share: $percent% — $value';
  }

  @override
  String get actionEdit => 'Edit';

  @override
  String get actionDelete => 'Delete';

  @override
  String get finTitle => 'Finances';

  @override
  String get finTransfer => 'Transfer';

  @override
  String get finLoans => 'Loans';

  @override
  String get finDebts => 'Debts';

  @override
  String get finCategories => 'Categories';

  @override
  String get finRefresh => 'Refresh';

  @override
  String get finFabAccount => 'Account';

  @override
  String get finEmptyAccounts => 'No accounts yet.';

  @override
  String get finNetWorth => 'Net worth';

  @override
  String get finCalculating => 'Calculating…';

  @override
  String finCash(String value) {
    return 'Cash $value';
  }

  @override
  String finPortfolio(String value) {
    return 'Portfolio $value';
  }

  @override
  String finLoansReceivable(String value) {
    return 'Loans receivable $value';
  }

  @override
  String finDebtsLine(String value) {
    return 'Debts −$value';
  }

  @override
  String finMissingItems(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other:
          '$count items left out of the sum for lack of exchange rate or quote.',
      one: '1 item left out of the sum for lack of exchange rate or quote.',
    );
    return '$_temp0';
  }

  @override
  String finFxBanner(String currencies) {
    return 'Exchange rate unavailable right now for $currencies. Items in those currencies are left out of net worth.';
  }

  @override
  String finNoRate(String pair) {
    return 'no exchange rate $pair';
  }

  @override
  String get finNewAccount => 'New account';

  @override
  String get finEditAccount => 'Edit account';

  @override
  String get finFieldName => 'Name';

  @override
  String get finFieldHolder => 'Holder';

  @override
  String get finFieldCurrency => 'Currency';

  @override
  String get finAccountNameHint => 'Nubank';

  @override
  String get finCurrencyLocked =>
      'Can\'t change on an account that already has transactions.';

  @override
  String finDeleteTitle(String name) {
    return 'Delete $name?';
  }

  @override
  String get finDeleteWithTx =>
      'The transactions of this account will also be deleted.';

  @override
  String get finDeleteNoTx => 'The account has no transactions.';

  @override
  String get finTransferNeedTwo => 'Add two accounts to transfer.';

  @override
  String get finReconcile => 'Reconcile balance';

  @override
  String finReconcileTitle(String account) {
    return 'Reconcile balance: $account';
  }

  @override
  String get finReconcileIntroYield =>
      'Enter the balance your bank shows on a date. The difference against the balance built from the transactions is booked as yield, including yield not redeemed yet.';

  @override
  String get finReconcileIntroAdjustment =>
      'Enter the balance your bank or broker shows on a date. The difference against the balance built from the transactions is booked as income or an expense: use it for an opening balance or cash that is missing.';

  @override
  String get finReconcileAdjustmentToggle =>
      'Balance adjustment (not yield, can go down)';

  @override
  String finReconcileBankBalance(String currency) {
    return 'Bank balance ($currency)';
  }

  @override
  String finReconcileLedger(String value) {
    return 'Balance from the transactions on that date: $value';
  }

  @override
  String finReconcileYieldToBook(String value) {
    return 'Yield to book: $value';
  }

  @override
  String finReconcileAdjustmentToBook(String value) {
    return 'Adjustment to book: $value';
  }

  @override
  String get finReconcileBookYield => 'Book yield';

  @override
  String get finReconcileBookAdjustment => 'Book adjustment';

  @override
  String get finReconcileDone => 'Reconciliation booked';

  @override
  String get finYieldEstimate => 'Estimate yield';

  @override
  String finYieldTitle(String account) {
    return 'Estimate yield: $account';
  }

  @override
  String get finYieldIntro =>
      'Estimates the yield of each redemption from the deposits and redemptions of this account and the Selic rate. It is an estimate: the daily CDI is approximated (target Selic minus 0.10 p.p.) and income tax is a flat 22.5%. Check the numbers against your bank statement before booking.';

  @override
  String get finYieldPctLabel => '% of the CDI';

  @override
  String get finYieldEstimateButton => 'Estimate';

  @override
  String finYieldRowTitle(String date, String paid) {
    return '$date: paid $paid';
  }

  @override
  String finYieldRowDetail(String gross, String iof, String ir, String net) {
    return 'Gross yield $gross · IOF $iof · IR $ir · net $net';
  }

  @override
  String get finYieldPartial =>
      'Part of this redemption has no deposit in this account: only the known part was estimated.';

  @override
  String finYieldBookSelected(int count) {
    return 'Book $count selected';
  }

  @override
  String get finYieldBooked => 'Yield booked';

  @override
  String get finYieldEmpty =>
      'Nothing to book: no redemption without yield was found.';

  @override
  String finYieldSkipped(int covered, int noYield) {
    return 'Left out: $covered on days that already have yield, $noYield with no yield.';
  }

  @override
  String get finYieldSelicUnavailable =>
      'Could not fetch the Selic rates from the central bank. Check your connection and try again.';

  @override
  String finErrSelicNotCovered(String from, String to) {
    return 'The Selic history does not cover $from to $to';
  }

  @override
  String get finErrPctOfCdiOutOfRange =>
      'The percentage of the CDI must be above 0 and at most 200';

  @override
  String get finFrom => 'From';

  @override
  String get finTo => 'To';

  @override
  String finAmountSent(String currency) {
    return 'Sent ($currency)';
  }

  @override
  String finAmount(String currency) {
    return 'Amount ($currency)';
  }

  @override
  String finAmountReceived(String currency) {
    return 'Received ($currency)';
  }

  @override
  String finEffectiveRate(String from, String rate, String to) {
    return 'Effective rate: 1 $from = $rate $to';
  }

  @override
  String get finSpreadIncluded =>
      'Spread and fees already included in the amounts.';

  @override
  String get finNotes => 'Notes';

  @override
  String get finSameAccount => 'Choose two different accounts.';

  @override
  String get finAmountsRequired => 'Enter the amounts, greater than zero.';

  @override
  String get txNewTitle => 'New transaction';

  @override
  String get txCategory => 'Category';

  @override
  String get txNoCategory => 'No category';

  @override
  String get txAmountRequired => 'Enter an amount greater than zero.';

  @override
  String get txDeleteTitle => 'Delete transaction?';

  @override
  String txDeleteBody(String type, String value, String date) {
    return '$type of $value on $date.';
  }

  @override
  String get txDeleteTransferLeg =>
      'The other leg of the transfer will also be deleted.';

  @override
  String get txFab => 'Transaction';

  @override
  String get txBalance => 'Balance';

  @override
  String get txEmpty => 'No transactions in this account yet.';

  @override
  String get txFuture => 'not in the balance yet';

  @override
  String get txTypeReceita => 'Income';

  @override
  String get txTypeDespesa => 'Expense';

  @override
  String get txTypeTransferenciaSaida => 'Transfer (out)';

  @override
  String get txTypeTransferenciaEntrada => 'Transfer (in)';

  @override
  String get txTypeParcelaDivida => 'Debt installment';

  @override
  String get txTypeEmprestimoConcedido => 'Loan granted';

  @override
  String get txTypeRecebimentoEmprestimo => 'Loan repayment';

  @override
  String genTxInstallment(String name, int number, int total) {
    return '$name — installment $number/$total';
  }

  @override
  String genTxLoanGranted(String name) {
    return 'Loan to $name';
  }

  @override
  String genTxLoanRepayment(String name) {
    return 'Repayment from $name';
  }

  @override
  String get amortizationPrice => 'Price (fixed installment)';

  @override
  String get amortizationSac => 'SAC (fixed amortization)';

  @override
  String get actionRegister => 'Add';

  @override
  String get liabTitle => 'Debts';

  @override
  String get liabFab => 'Debt';

  @override
  String get liabEmpty => 'No debts yet.';

  @override
  String get liabNeedAccount => 'Add an account before the debt.';

  @override
  String get liabNew => 'New debt';

  @override
  String get liabNameHint => 'Car financing';

  @override
  String get liabFieldAccount => 'Account';

  @override
  String liabFieldPrincipal(String currency) {
    return 'Principal ($currency)';
  }

  @override
  String get liabFieldRate => 'Rate (% per month)';

  @override
  String get liabRateHelp => '0 for a debt without interest.';

  @override
  String get liabFieldTerm => 'Term (months)';

  @override
  String get liabFieldSystem => 'System';

  @override
  String liabStart(String date) {
    return 'Start $date';
  }

  @override
  String get liabFirstDue => 'Installment 1 is due one month after the start.';

  @override
  String get liabInvalid =>
      'Enter a name, a principal and a term greater than zero, and the rate.';

  @override
  String liabDeleteTitle(String name) {
    return 'Delete $name?';
  }

  @override
  String get liabDeleteBody =>
      'This debt\'s installments are also removed from the account\'s transactions.';

  @override
  String get liabDeleteAction => 'Delete debt';

  @override
  String liabBalance(String value) {
    return 'Outstanding balance $value';
  }

  @override
  String liabPaid(int paid, int total) {
    return '$paid of $total installments paid';
  }

  @override
  String liabNext(String value, String date) {
    return 'Next: $value on $date';
  }

  @override
  String get liabSettled => 'Paid off';

  @override
  String liabSummary(String system, String rate, String value) {
    return '$system · $rate% per month · principal $value';
  }

  @override
  String liabInstallmentLine(
    String number,
    String date,
    String value,
    String interest,
  ) {
    return '$number · $date · $value (interest $interest)';
  }

  @override
  String get loanTitle => 'Loans';

  @override
  String get loanFab => 'Loan';

  @override
  String get loanEmpty => 'No loans yet.';

  @override
  String get loanNew => 'New loan';

  @override
  String get loanEdit => 'Edit loan';

  @override
  String get loanBorrower => 'Borrower';

  @override
  String get loanNewBorrowerOption => 'New borrower…';

  @override
  String get loanNewBorrowerName => 'Borrower\'s name';

  @override
  String get loanFromAccount => 'Account it came from';

  @override
  String get loanNoAccountHelp =>
      'Without an account it is just a record: no balance changes.';

  @override
  String get loanNoAccount => 'No account';

  @override
  String get loanCurrency => 'Currency';

  @override
  String loanPrincipal(String currency) {
    return 'Principal ($currency)';
  }

  @override
  String get loanRateOptional => 'Rate (% per month, optional)';

  @override
  String get loanRateHelp =>
      'Simple interest, informational only. Empty = no interest.';

  @override
  String loanDateOn(String date) {
    return 'On $date';
  }

  @override
  String get loanInclude => 'Add to net worth';

  @override
  String get loanNotes => 'Note';

  @override
  String get loanInvalid =>
      'Enter the borrower and a principal greater than zero; the rate, if any, cannot be negative.';

  @override
  String get loanRateNegative => 'The rate cannot be negative.';

  @override
  String get loanRepayTitle => 'Register repayment';

  @override
  String get loanRepayConfirm => 'Register';

  @override
  String loanReceived(String currency) {
    return 'Received ($currency)';
  }

  @override
  String get loanReceivedHelp =>
      '0 or empty for a write-off, with no money coming in.';

  @override
  String get loanWrittenOff => 'Written off the balance';

  @override
  String get loanWrittenOffHelp => 'Empty = the same as the amount received.';

  @override
  String get loanReceivingAccount => 'Account that received it';

  @override
  String get loanRepayInvalid =>
      'Enter the amount written off (greater than zero); the amount received cannot be negative.';

  @override
  String loanDeleteTitle(String name) {
    return 'Delete the loan to $name?';
  }

  @override
  String get loanDeleteBody =>
      'Its repayments and the account transactions it generated are also removed.';

  @override
  String get loanDeleteRepayTitle => 'Delete the repayment?';

  @override
  String loanDeleteRepayBody(String value, String date) {
    return 'Repayment of $value on $date. The receipt posted to the account is also removed.';
  }

  @override
  String loanToReceive(String value) {
    return 'To receive $value';
  }

  @override
  String get loanOutsideNetWorth => ' (outside net worth)';

  @override
  String loanPrincipalOn(String value, String date) {
    return 'Principal $value on $date';
  }

  @override
  String loanInterest(String value) {
    return 'Accrued interest $value';
  }

  @override
  String loanExtraMore(String value) {
    return 'Received more: $value';
  }

  @override
  String loanExtraLess(String value) {
    return 'Received less: $value';
  }

  @override
  String loanRate(String rate) {
    return 'Rate $rate% per month (simple interest)';
  }

  @override
  String get loanNoRepayments => 'No repayments yet.';

  @override
  String loanRepaymentLine(String date, String received, String writtenOff) {
    return '$date · received $received · written off $writtenOff';
  }

  @override
  String get catTitle => 'Categories';

  @override
  String get catFab => 'Category';

  @override
  String get catEmpty => 'No categories yet.';

  @override
  String get catNew => 'New category';

  @override
  String get catEdit => 'Edit category';

  @override
  String get catNameHint => 'Groceries';

  @override
  String catMonthlyLimit(String base) {
    return 'Monthly limit ($base)';
  }

  @override
  String get catNoLimitHelp => 'Empty = no limit.';

  @override
  String get catInvalid =>
      'Enter the name and, if any, a limit greater than zero.';

  @override
  String get catDeleteBody =>
      'Its transactions become uncategorized; none is deleted.';

  @override
  String catSpentMonth(String value) {
    return 'Spent this month: $value';
  }

  @override
  String catSpentOfLimit(String spent, String limit) {
    return '$spent of $limit';
  }

  @override
  String catOver(String value) {
    return 'Over by $value';
  }

  @override
  String catMissingFx(int count) {
    return '$count expense(s) left out of the sum for lack of exchange rate.';
  }

  @override
  String get currencyUsd => 'US dollar';

  @override
  String get currencyEur => 'Euro';

  @override
  String get currencyGbp => 'Pound sterling';

  @override
  String get currencyJpy => 'Yen';

  @override
  String get currencyChf => 'Swiss franc';

  @override
  String get currencyCad => 'Canadian dollar';

  @override
  String get currencyAud => 'Australian dollar';

  @override
  String get currencyNzd => 'New Zealand dollar';

  @override
  String get currencyCny => 'Yuan';

  @override
  String get currencyBrl => 'Brazilian real';

  @override
  String get currencyArs => 'Argentine peso';

  @override
  String get currencyUyu => 'Uruguayan peso';

  @override
  String get currencyPyg => 'Guarani';

  @override
  String get currencyClp => 'Chilean peso';

  @override
  String get currencyCop => 'Colombian peso';

  @override
  String get currencyPen => 'Peruvian sol';

  @override
  String get currencyMxn => 'Mexican peso';

  @override
  String finErrUnknownCurrency(String currency) {
    return 'Currency not in the catalog: $currency';
  }

  @override
  String get finErrAmountMustBePositive =>
      'The amount must be greater than zero';

  @override
  String finErrInvalidDate(String date) {
    return 'Invalid date: $date';
  }

  @override
  String get finErrManualTypeOnly =>
      'Only income and expense are entered by hand; a transfer is created with Transfer, a debt installment by creating a debt, and a loan by granting a loan';

  @override
  String get finErrLiabilityNameRequired => 'Enter the debt name';

  @override
  String get finErrBorrowerNameRequired => 'Enter the borrower\'s name';

  @override
  String get finErrPrincipalMustBePositive =>
      'The principal must be greater than zero';

  @override
  String get finErrTermMustBePositive =>
      'The term (in months) must be greater than zero';

  @override
  String get finErrNegativeMonthlyRate => 'The monthly rate can\'t be negative';

  @override
  String get finErrRepaidAmountMustBePositive =>
      'The amount repaid must be greater than zero';

  @override
  String get finErrReceivedAmountNegative =>
      'The amount received can\'t be negative';

  @override
  String finErrLoanCurrencyMismatch(
    String loanCurrency,
    String accountCurrency,
  ) {
    return 'The loan is in $loanCurrency and the account in $accountCurrency';
  }

  @override
  String get finErrCategoryNameRequired => 'Enter the category name';

  @override
  String get finErrCategoryLimitMustBePositive =>
      'The monthly limit must be greater than zero';

  @override
  String get finErrTransferAmountsMustBePositive =>
      'The amounts must be greater than zero';

  @override
  String get finErrTransferSameAccount => 'Choose two different accounts';

  @override
  String get finErrTransferSameCurrencyAmountsDiffer =>
      'Accounts in the same currency receive exactly what left';

  @override
  String get finErrAccountNotFound => 'Account not found';

  @override
  String get finErrAccountCurrencyLocked =>
      'The currency can\'t change on an account that already has transactions';

  @override
  String get finErrBorrowerHasLoans =>
      'This borrower still has loans: delete them first';

  @override
  String get finErrBorrowerNotFound => 'Borrower not found';

  @override
  String get finErrLoanNotFound => 'Loan not found';

  @override
  String get finErrLinkedTransactionLocked =>
      'A debt installment and a loan transaction are only deleted together with the debt or the loan';

  @override
  String get finErrBalanceMustBeNonNegative =>
      'The bank balance must be zero or more';

  @override
  String finErrDateInFuture(String date) {
    return 'The balance date cannot be in the future: $date';
  }

  @override
  String get finErrNothingToReconcile =>
      'The transactions already match the bank balance: nothing to reconcile';

  @override
  String get finErrBankBalanceBelowLedger =>
      'The bank balance is lower than the balance from the transactions. A withdrawal may be missing, or use the balance adjustment';

  @override
  String get cryptoTaxTitle => 'Crypto tax';

  @override
  String get cryptoTaxDisclaimer =>
      'Calculated only from the transactions entered in this app, at average cost and with the fee entered on each operation. The rules come from secondary sources: check with the tax authority (GCAP) before paying.';

  @override
  String get cryptoTaxUsdNote =>
      'USD operations converted at the dollar rate of the day (Yahoo, USDBRL=X).';

  @override
  String get cryptoTaxFxUnavailable =>
      'Exchange rate unavailable right now: the USD operations were left out. Try again in a moment.';

  @override
  String get cryptoTaxEmpty => 'No crypto sales in reais yet.';

  @override
  String cryptoTaxMonthSales(String value) {
    return 'Month sales: $value';
  }

  @override
  String cryptoTaxMonthGain(String value) {
    return 'Month gain: $value';
  }

  @override
  String cryptoTaxTax(String value, String rate) {
    return 'Tax: $value (rate $rate%)';
  }

  @override
  String cryptoTaxCarriedIn(String value) {
    return 'Carried from earlier months: $value';
  }

  @override
  String cryptoTaxSalesCount(int count) {
    return 'Sales ($count)';
  }

  @override
  String cryptoTaxSaleTitle(String ticker, String date) {
    return '$ticker — $date';
  }

  @override
  String cryptoTaxSaleDetail(
    String sale,
    String fee,
    String cost,
    String gain,
  ) {
    return 'Sale $sale$fee · cost $cost · gain $gain';
  }

  @override
  String cryptoTaxSaleFee(String value) {
    return ' (fee $value)';
  }

  @override
  String cryptoTaxStatusExempt(String limit) {
    return 'Exempt: sales up to $limit in the month';
  }

  @override
  String cryptoTaxStatusDarf(int code, String value, String date) {
    return 'DARF $code of $value due $date';
  }

  @override
  String cryptoTaxStatusBelowMinimum(String value, String minimum) {
    return 'Tax of $value is below the $minimum minimum: added to next month';
  }

  @override
  String get cryptoTaxStatusNoTax => 'No tax: the month closed with no gain';

  @override
  String cryptoTaxWarnCostUnknown(String ticker, String date) {
    return 'Sale of $ticker on $date exceeds the purchases entered: the gain counts only the part with known cost. Enter the missing purchases.';
  }

  @override
  String cryptoTaxWarnSalesNonBrl(String period) {
    return 'Crypto sales not in BRL in $period are not assessed here (foreign rules).';
  }

  @override
  String cryptoTaxWarnFxMissing(String period) {
    return 'USD crypto sale in $period has no exchange rate to convert: left out of the assessment.';
  }

  @override
  String get b3TaxTitle => 'B3 tax';

  @override
  String get b3TaxDisclaimer =>
      'An estimate from the transactions entered in this app, at average cost and with the fee entered on each operation. Day trade is a purchase and a sale of the same asset, on the same day, in the same custody. Options, splits and bonus shares are not covered. Rates and the exemption limit come from secondary sources: check with the tax authority and your broker\'s notes before paying.';

  @override
  String get b3TaxEmpty => 'No sales on B3 yet.';

  @override
  String b3TaxMonthStockSales(String value) {
    return 'Stock sales (swing): $value';
  }

  @override
  String b3TaxMonthExemptGain(String value) {
    return 'Exempt gain: $value';
  }

  @override
  String b3TaxBucketLine(
    String name,
    String result,
    String used,
    String tax,
    String rate,
  ) {
    return '$name: result $result, loss offset $used, tax $tax ($rate%)';
  }

  @override
  String b3TaxIrrf(String value) {
    return 'Withholding tax (estimated): $value';
  }

  @override
  String b3TaxCarriedIn(String value) {
    return 'Carried from earlier months: $value';
  }

  @override
  String get b3TaxBucketCommon => 'Common';

  @override
  String get b3TaxBucketDayTrade => 'Day trade';

  @override
  String get b3TaxBucketFii => 'REITs (FII)';

  @override
  String b3TaxSalesCount(int count) {
    return 'Sales ($count)';
  }

  @override
  String b3TaxSaleTitle(String ticker, String date) {
    return '$ticker — $date';
  }

  @override
  String b3TaxSaleDetail(String bucket, String sale, String cost, String gain) {
    return '$bucket · sale $sale · cost $cost · gain $gain';
  }

  @override
  String b3TaxStatusExempt(String limit) {
    return 'Exempt: stock sales up to $limit in the month';
  }

  @override
  String b3TaxStatusDarf(int code, String value, String date) {
    return 'DARF $code of $value due $date';
  }

  @override
  String b3TaxStatusBelowMinimum(String value, String minimum) {
    return 'Tax of $value is below the $minimum minimum: added to next month';
  }

  @override
  String get b3TaxStatusNoTax => 'No tax: nothing to pay this month';

  @override
  String b3TaxWarnCostUnknown(String ticker, String date) {
    return 'Sale of $ticker on $date exceeds the purchases entered: the gain counts only the part with known cost. Enter the missing purchases.';
  }

  @override
  String get valuationButton => 'Valuation';

  @override
  String valuationTitle(String ticker) {
    return 'Valuation — $ticker';
  }

  @override
  String valuationCurrentPrice(String value) {
    return 'Current price: $value';
  }

  @override
  String get valuationModelLabel => 'Model';

  @override
  String get valuationDisclaimer =>
      'Only the arithmetic of the assumptions you enter, not a recommendation. Rates are typed in percent. Amounts and shares must use the same scale: if the amounts are in R\$ millions, enter the shares in millions.';

  @override
  String get valuationCalculate => 'Calculate';

  @override
  String get valuationSave => 'Save';

  @override
  String get valuationSaved => 'Calculation saved';

  @override
  String valuationFairPrice(String value) {
    return 'Fair price: $value';
  }

  @override
  String valuationSafetyMargin(String value) {
    return 'Safety margin: $value';
  }

  @override
  String get valuationVerdictCheap =>
      'Cheap: the price is below the fair price';

  @override
  String get valuationVerdictExpensive =>
      'Expensive: the price is at or above the fair price';

  @override
  String get valuationVerdictCheapShort => 'Cheap';

  @override
  String get valuationVerdictExpensiveShort => 'Expensive';

  @override
  String get valuationSavedList => 'Saved valuations';

  @override
  String valuationSavedEmpty(String ticker) {
    return 'No saved valuations for $ticker yet.';
  }

  @override
  String valuationSavedDetail(
    String price,
    String fair,
    String margin,
    String verdict,
    String date,
  ) {
    return 'Price $price · fair $fair · margin $margin · $verdict · $date';
  }

  @override
  String valuationCompareButton(int count) {
    return 'Compare ($count)';
  }

  @override
  String valuationCompareTitle(String ticker) {
    return '$ticker: comparison';
  }

  @override
  String valuationCompareColumn(int n) {
    return 'Calculation $n';
  }

  @override
  String get valuationCompareDiffers => 'differs';

  @override
  String get valuationCompareRowModel => 'Model';

  @override
  String get valuationCompareRowReferenceYear => 'Reference year';

  @override
  String get valuationCompareRowCurrentPrice => 'Current price';

  @override
  String get valuationCompareRowFairPrice => 'Fair price';

  @override
  String get valuationCompareRowSafetyMargin => 'Safety margin';

  @override
  String get valuationCompareRowVerdict => 'Verdict';

  @override
  String get valuationDelete => 'Delete';

  @override
  String get valuationModelBazin => 'Bazin (dividend price ceiling)';

  @override
  String get valuationModelGraham => 'Graham (intrinsic value)';

  @override
  String get valuationModelGordon => 'Gordon (dividend growth)';

  @override
  String get valuationModelDcf => 'DCF (discounted cash flow)';

  @override
  String get valuationModelBanks => 'Banks (P/B via ROE)';

  @override
  String get valuationModelRim => 'RIM (residual income)';

  @override
  String get valuationModelRnav => 'RNAV (net asset value)';

  @override
  String get valuationModelProjectedCeiling => 'Projected ceiling';

  @override
  String get valuationFieldAverageDividend =>
      'Average annual dividend per share (R\$)';

  @override
  String get valuationFieldDesiredYield => 'Desired yield (%)';

  @override
  String get valuationFieldEps => 'Earnings per share, EPS (R\$)';

  @override
  String get valuationFieldBookValuePerShare => 'Book value per share (R\$)';

  @override
  String get valuationFieldCurrentDividend =>
      'Current annual dividend per share (R\$)';

  @override
  String get valuationFieldExpectedGrowth => 'Expected growth (%)';

  @override
  String get valuationFieldKe => 'Cost of equity, Ke (%)';

  @override
  String get valuationFieldEbit => 'EBIT (R\$)';

  @override
  String get valuationFieldTaxRate => 'Tax rate (%)';

  @override
  String get valuationFieldDepreciationAmortization =>
      'Depreciation and amortization (R\$)';

  @override
  String get valuationFieldCapex => 'Capex (R\$)';

  @override
  String get valuationFieldNwcChange => 'Change in working capital (R\$)';

  @override
  String get valuationFieldTotalDebt => 'Total debt (R\$)';

  @override
  String get valuationFieldCash => 'Cash (R\$)';

  @override
  String get valuationFieldSharesOutstanding => 'Shares outstanding (millions)';

  @override
  String get valuationFieldBeta => 'Beta';

  @override
  String get valuationFieldRiskFreeRate => 'Risk-free rate (%)';

  @override
  String get valuationFieldMarketRiskPremium => 'Market risk premium (%)';

  @override
  String get valuationFieldKd => 'Cost of debt, Kd (%)';

  @override
  String get valuationFieldPerpetuityGrowth => 'Perpetuity growth (%)';

  @override
  String get valuationFieldRoe => 'ROE (%)';

  @override
  String get valuationFieldPayout => 'Payout (%)';

  @override
  String get valuationFieldRoeCurrent => 'Current ROE (%)';

  @override
  String get valuationFieldFadeYears => 'Years for ROE to converge to Ke';

  @override
  String get valuationFieldLandbank => 'Landbank (R\$)';

  @override
  String get valuationFieldInventoryAtMarketValue =>
      'Inventory at market value (R\$)';

  @override
  String get valuationFieldNetCash => 'Net cash (R\$)';

  @override
  String get valuationFieldProjectionYears => 'Projection years';

  @override
  String valuationErrMissingInput(String field) {
    return 'Fill in the field: $field';
  }

  @override
  String valuationErrInputNotFinite(String field) {
    return 'Invalid number in: $field';
  }

  @override
  String get valuationErrPriceMustBePositive =>
      'The current price must be greater than zero';

  @override
  String get valuationErrDesiredYieldMustBePositive =>
      'The desired yield must be greater than zero';

  @override
  String get valuationErrEpsAndBookValueMustBePositive =>
      'EPS and book value per share must both be greater than zero';

  @override
  String get valuationErrKeMustExceedGrowth =>
      'The cost of equity (Ke) must be greater than the growth rate';

  @override
  String get valuationErrWaccMustExceedGrowth =>
      'The discount rate (WACC) must be greater than the perpetuity growth';

  @override
  String get valuationErrSharesMustBePositive =>
      'Shares outstanding must be greater than zero';

  @override
  String get valuationErrFadeYearsMustBePositive =>
      'The years to converge must be at least 1';

  @override
  String valuationErrWholeNumberRequired(String field) {
    return '$field must be a whole number';
  }

  @override
  String get valuationErrProjectionYearsNegative =>
      'Projection years cannot be negative';

  @override
  String get valuationErrFairPriceMustBePositive =>
      'These assumptions give a fair price of zero or less, so there is no valuation. Review the numbers.';

  @override
  String get swapIntro =>
      'Swapping one coin for another counts as a sale of the coin given up for tax purposes: it enters the month\'s sales (R\$ 35 thousand limit) and realizes a gain over the average cost. It records one sale and one purchase for the same value.';

  @override
  String get swapNeedTwoCoins =>
      'You need a balance in one crypto and another one registered.';

  @override
  String get swapCoinOut => 'Coin out';

  @override
  String swapCoinOutItem(String ticker, String balance) {
    return '$ticker (balance $balance)';
  }

  @override
  String get swapQuantityOut => 'Quantity out';

  @override
  String get swapCoinIn => 'Coin in';

  @override
  String get swapQuantityIn => 'Quantity in';

  @override
  String get swapMarketValue => 'Market value of the swap (BRL)';

  @override
  String get swapUseMarketPrice => 'Use market price';

  @override
  String get swapSearching => 'Searching…';

  @override
  String get swapFee => 'Fee (optional, BRL)';

  @override
  String get swapCustody => 'Custody (optional)';

  @override
  String get swapNoCustody => 'None';

  @override
  String swapDate(String date) {
    return 'Date: $date';
  }

  @override
  String get swapRegister => 'Record swap';

  @override
  String swapHintNoBrlQuote(String ticker) {
    return 'No quote in reais for $ticker.';
  }

  @override
  String swapHintTodayPrice(String date) {
    return 'Today\'s price ($date); if the swap was on another date, adjust the value.';
  }

  @override
  String get swapHintQuoteUnavailable =>
      'Quote unavailable right now; enter the value.';

  @override
  String swapHintNoQuote(String ticker) {
    return 'No quote for $ticker; enter the value.';
  }

  @override
  String get swapFeeNotNumber => 'The fee must be a number.';

  @override
  String get swapErrNeedTwoDifferentCoins =>
      'A swap needs two different coins.';

  @override
  String swapErrNotCrypto(String ticker) {
    return '$ticker is not crypto; a swap only works between cryptocurrencies.';
  }

  @override
  String swapErrNotBrl(String ticker, String currency) {
    return '$ticker is in $currency; swaps are assessed in reais.';
  }

  @override
  String get swapErrQuantityOutMustBePositive =>
      'The quantity out must be greater than zero.';

  @override
  String get swapErrQuantityInMustBePositive =>
      'The quantity in must be greater than zero.';

  @override
  String get swapErrValueMustBePositive =>
      'The value must be greater than zero.';

  @override
  String get swapErrNegativeFee => 'The fee can\'t be negative.';

  @override
  String swapErrInsufficientBalance(
    String ticker,
    String available,
    String requested,
  ) {
    return 'Insufficient $ticker balance: you have $available and the swap gives $requested.';
  }

  @override
  String get fundTaxTitle => 'Fund tax';

  @override
  String get fundTaxDisclaimer =>
      'Estimate based on the transactions and quotas entered in this app. The semiannual quota tax and the redemption tax are withheld at the source by the fund: there is no DARF. The rules come from secondary sources; check with the tax authority and the fund statement.';

  @override
  String get fundRefreshCvm => 'Update from CVM';

  @override
  String get fundCvmDownloadWarning =>
      'Downloads a file of up to ~12 MB for each month read (usually 4 months, about 40 MB). Continue?';

  @override
  String get fundDownload => 'Download';

  @override
  String get fundProgressPreparing => 'Preparing…';

  @override
  String fundProgressReading(int done, int total) {
    return 'Reading month $done of $total…';
  }

  @override
  String fundCvmReadFailed(String error) {
    return 'Could not read the CVM: $error';
  }

  @override
  String get fundLossCreditsHeader =>
      'Loss to offset against later gains and semiannual quota tax (same administrator and same type)';

  @override
  String fundLossCreditLine(
    String amount,
    String tickers,
    String administrator,
    String term,
  ) {
    return '$amount — $tickers ($administrator, $term)';
  }

  @override
  String get fundNoAdministrator => 'no administrator';

  @override
  String get fundNone => 'No funds registered yet.';

  @override
  String get fundStatusNoPosition => 'No quotas in the portfolio.';

  @override
  String get fundStatusNoTerm =>
      'Choose the fund type to estimate the semiannual quota tax and the redemption tax.';

  @override
  String get fundStatusNoQuota =>
      'Enter the quota value to estimate the semiannual quota tax.';

  @override
  String get fundStatusExempt =>
      'Equity fund or investment entity: no semiannual quota tax.';

  @override
  String fundStatusEstimated(String date, String tax, String quotas) {
    return 'Next semiannual quota tax on $date: $tax ($quotas quotas)';
  }

  @override
  String fundBasis(String basis, String origin, String gain, String rate) {
    return 'Basis per quota $basis ($origin) · gain $gain × $rate%';
  }

  @override
  String get fundBasisLastComeCotas =>
      'quota value at the last semiannual quota tax';

  @override
  String get fundBasisEstoque =>
      'average cost plus the 2023 stock already taxed';

  @override
  String get fundBasisAverageCost => 'average cost';

  @override
  String get fundEstoqueNotTaxed =>
      'Say how the 2023 stock was taxed to estimate the tax and the cost of the semiannual quota tax.';

  @override
  String get fundEstoqueNeedsQuota =>
      'Enter the quota of 12/31/2023 (or up to 10 days before) to estimate the stock.';

  @override
  String fundEstoqueEstimated(
    String base,
    String date,
    String quota,
    String rate,
    String tax,
  ) {
    return '2023 stock: gain of $base up to $date (quota $quota) at $rate% = $tax';
  }

  @override
  String fundRefundable(String total) {
    return 'Semiannual quota tax paid above the tax due: $total. The law does not provide a refund of this excess (IN RFB 1.585/2015, art. 15; Law 14.754/2023, art. 17, §6). It is tax paid on income that later disappeared, and it comes back as a loss to offset against later gains. The app already adds this loss to the loss credit; the excess itself is not included in the net amounts.';
  }

  @override
  String fundWarnStaleQuota(int days) {
    return 'The latest quota value is more than $days days old: the estimate may be out of date.';
  }

  @override
  String fundWarnUnrecordedDeduction(String date) {
    return 'The semiannual quota tax of $date has already passed with no deduction recorded: enter the Semiannual quota tax transaction so the basis is right.';
  }

  @override
  String fundWarnCreditNotInformed(String date, String balance) {
    return 'The semiannual quota tax of $date was recorded with $balance of loss credit in the group: the administrator may have used part of it, so the credit balance shown may be above the real one. Fill in the loss credit used in that transaction to resolve this.';
  }

  @override
  String fundWarnCreditExceedsBalance(
    String date,
    String informed,
    String balance,
  ) {
    return 'The semiannual quota tax of $date says $informed of loss credit was used, but the group only had $balance: only that balance was deducted. Check the amount.';
  }

  @override
  String fundWarnRedemptionUncovered(String date) {
    return 'Redemption of $date is larger than the investments entered: the cost counts only the known part.';
  }

  @override
  String fundWarnRedemptionExcess(String date, String amount) {
    return 'In the redemption of $date the semiannual quota tax paid exceeded the tax due by $amount: it stays out of the net amount (see the note about the excess).';
  }

  @override
  String fundCreditReduced(String used, String before) {
    return 'Loss credit of $used has already reduced the gain: the tax would be $before without it.';
  }

  @override
  String fundRedemptionCreditLine(String used, String ir, String before) {
    return 'Losses from earlier redemptions in the group offset $used of the gain: tax $ir instead of $before.';
  }

  @override
  String fundRedemptionLossLine(String loss) {
    return 'This redemption lost $loss: it offsets the gain of later redemptions and semiannual quota tax in the same group (same administrator and same type).';
  }

  @override
  String fundRecordHead(String administrator, String date) {
    return 'All the funds you entered from administrator $administrator were redeemed by $date.';
  }

  @override
  String fundRecordExpired(String head, String balance, String until) {
    return '$head The administrator\'s obligation to keep the loss record ($balance) ended on $until (IN RFB 1.585, art. 15 §2): check that it still has the record before counting on the balance.';
  }

  @override
  String fundRecordActive(String head, String balance, String until) {
    return '$head The administrator is only required to keep the loss record ($balance) until $until (IN RFB 1.585, art. 15 §2). The law does not limit your right to offset, but check with the administrator.';
  }

  @override
  String fundCvmLatest(String date) {
    return ' Last CVM quota: $date.';
  }

  @override
  String fundCvmRead(String latest) {
    return 'Quotas read from the CVM.$latest';
  }

  @override
  String get fundCvmNoCnpj =>
      'Enter the fund\'s CNPJ to read the quota from the CVM.';

  @override
  String fundCvmPending(String latest) {
    return 'Not read yet.$latest';
  }

  @override
  String get fundCvmNotFound =>
      'The CVM returned no quota for this CNPJ (or subclass) in the months read. Check the CNPJ.';

  @override
  String get fundCvmNeedsSubclass =>
      'This CNPJ only has subclasses in the report: choose one.';

  @override
  String fundCvmError(String message, String latest) {
    return 'Could not read the CVM right now ($message). Try again in a moment.$latest';
  }

  @override
  String get fundCvmErrorDefault => 'error';

  @override
  String get fundTermLabel => 'Fund type';

  @override
  String get fundStructureLabel => 'Fund structure (Law 14.754)';

  @override
  String get fundEstoqueLabel => '2023 stock (arts. 27 and 28)';

  @override
  String get fundEstoqueNotInformed => 'Not taxed / not informed';

  @override
  String get fundAdministratorLabel =>
      'Administrator / intermediary (offsets loss)';

  @override
  String get fundAdministratorHint => 'e.g. XP, BTG';

  @override
  String get fundAdministratorHelpNoTerm => 'Choose the fund type first.';

  @override
  String get fundAdministratorHelpNone =>
      'Without this, the fund only offsets losses against itself.';

  @override
  String get fundAdministratorHelpSet =>
      'Funds of the same type and the same administrator offset each other.';

  @override
  String get fundSaveAdministrator => 'Save administrator';

  @override
  String get fundCnpjLabel => 'Fund CNPJ (quota from the CVM)';

  @override
  String get fundSaveCnpj => 'Save CNPJ';

  @override
  String get fundSubclassLabel => 'CVM subclass';

  @override
  String get fundSubclassNone => 'Class (no subclass)';

  @override
  String fundQuotasTitle(int count) {
    return 'Quota value ($count)';
  }

  @override
  String get fundDeleteQuota => 'Delete quota';

  @override
  String get fundAddQuota => 'Enter quota';

  @override
  String fundDeductionsTitle(int count) {
    return 'Semiannual quota tax recorded ($count)';
  }

  @override
  String fundDeductionLine(
    String quantity,
    String price,
    String tax,
    String credit,
  ) {
    return '$quantity quotas at $price · tax $tax$credit';
  }

  @override
  String fundDeductionCredit(String value) {
    return ' · credit used $value';
  }

  @override
  String fundRedemptionsTitle(int count) {
    return 'Estimated redemptions and amortizations ($count)';
  }

  @override
  String fundRedemptionAmortization(String date, String quota) {
    return '$date — amortization (quota at $quota; the quotas stay)';
  }

  @override
  String fundRedemptionTitle(String date, String quotas, String quota) {
    return '$date — $quotas quotas at $quota';
  }

  @override
  String fundRedemptionDetail(String gross, String iof, String ir, String net) {
    return 'Gross $gross · IOF $iof · tax $ir · net $net';
  }

  @override
  String fundRedemptionComeCotasCredit(String value) {
    return 'Semiannual quota tax already paid offset $value of the tax';
  }

  @override
  String fundQuotaDialogTitle(String ticker) {
    return 'Quota of $ticker';
  }

  @override
  String get fundQuotaValueLabel => 'Quota value (R\$)';

  @override
  String fundDate(String date) {
    return 'Date: $date';
  }

  @override
  String get fundTermLongTerm => 'Long term';

  @override
  String get fundTermShortTerm => 'Short term';

  @override
  String get fundTermEquity => 'Equity';

  @override
  String get fundTermInvestmentEntity => 'Investment entity';

  @override
  String get fundStructureOpen => 'Open';

  @override
  String get fundStructureClosed => 'Closed';

  @override
  String get fundStructureExclusive => 'Exclusive';

  @override
  String get fundEstoqueGeneral => 'General rule (15%)';

  @override
  String get fundEstoqueOption8 => 'Option (8%)';

  @override
  String get fundErrNeedTermBeforeStructure =>
      'Choose the fund type before the structure';

  @override
  String get fundErrNeedTermBeforeEstoque =>
      'Choose the fund type before the 2023 stock';

  @override
  String get fundErrNeedTermBeforeAdministrator =>
      'Choose the fund type before the administrator';

  @override
  String get fundErrNeedTermBeforeSubclass =>
      'Choose the fund type before the CVM subclass';

  @override
  String get fundErrCnpjMustHave14Digits => 'The CNPJ must have 14 digits';

  @override
  String get foreignTitle => 'Foreign tax';

  @override
  String get foreignIntroNote =>
      'Calculated only from the transactions entered in this app, at average cost, with each operation\'s fee and in reais using the Central Bank PTAX (closing, selling rate; buying rate for tax withheld abroad), and on a day with no quote using the previous one. Where the PTAX is missing, the market rate (Yahoo) applies and the year warns that it is approximate. It goes in the annual return, with no monthly DARF. The rules come from secondary sources: check with the tax authority before paying.';

  @override
  String get foreignFxUnavailableNote =>
      'Exchange rate unavailable right now for some currency (neither PTAX nor market): its operations were left out. Try again in a moment.';

  @override
  String get foreignEmpty =>
      'No sales or dividends from foreign assets from 2024 on yet.';

  @override
  String foreignStatusTaxDue(String value) {
    return 'Estimated tax of $value on the return';
  }

  @override
  String foreignStatusCovered(String value) {
    return 'No tax due: the tax withheld abroad covers the $value owed';
  }

  @override
  String foreignStatusLoss(String value) {
    return 'No tax: the year closed with a loss of $value, which offsets the following years';
  }

  @override
  String get foreignStatusFitInLoss =>
      'No tax: the year\'s result fit within the loss from earlier years';

  @override
  String foreignRatePtax(String rate) {
    return 'Rate $rate (PTAX)';
  }

  @override
  String foreignRateMarket(String rate) {
    return 'Rate $rate (market)';
  }

  @override
  String foreignWarnCostUnknown(String ticker, String date) {
    return 'Sale of $ticker on $date exceeds the purchases entered: the gain counts only the part with known cost. Enter the missing purchases.';
  }

  @override
  String foreignWarnFxMissing(int year) {
    return 'Operations or dividends of $year have no exchange rate (neither PTAX nor market) for the date: they were left out of the assessment.';
  }

  @override
  String foreignWarnFxApproximate(int year) {
    return 'Operations or dividends of $year were converted at the market rate because the Central Bank PTAX was not available for them (no network, a currency the PTAX does not quote, or a long gap): the values for the year are approximate.';
  }

  @override
  String foreignCapitalGain(String value) {
    return 'Capital gain: $value';
  }

  @override
  String foreignDividends(String value) {
    return 'Dividends: $value';
  }

  @override
  String foreignLossUsed(String value) {
    return 'Loss from earlier years used: $value';
  }

  @override
  String foreignTaxableBase(String value) {
    return 'Calculation base: $value';
  }

  @override
  String foreignTax(String value, String rate) {
    return 'Tax: $value (rate $rate%)';
  }

  @override
  String foreignWithheld(String value, String credit) {
    return 'Withheld abroad: $value (credit of $credit)';
  }

  @override
  String foreignLossCarriedOut(String value) {
    return 'Loss that offsets the following years: $value';
  }

  @override
  String foreignSalesCount(int count) {
    return 'Sales ($count)';
  }

  @override
  String foreignDividendsCount(int count) {
    return 'Dividends ($count)';
  }

  @override
  String foreignItemTitle(String ticker, String date) {
    return '$ticker — $date';
  }

  @override
  String foreignSaleDetail(String rate, String sale, String cost, String gain) {
    return '$rate · sale $sale · cost $cost · gain $gain';
  }

  @override
  String foreignDividendDetail(String rate, String gross, String withheld) {
    return '$rate · $gross$withheld';
  }

  @override
  String foreignDividendWithheld(String value) {
    return ' · withheld $value';
  }

  @override
  String get foreignHoldingsTitle => 'Assets and Rights (Bens e Direitos)';

  @override
  String get foreignHoldingsNote =>
      'Acquisition cost in reais, not market value, only of what is entered in this app. One line per custody: the cost of each is its quantity times the asset\'s average cost, so the custodies add up to the position. Group, code and country of the record are yours: fill them in from the Receita program (sources disagree and the app does not check; it only pads zeros, so 3 becomes 03). They go into the copied description and the CSV. Estimate from secondary sources: check in the Receita program.';

  @override
  String get foreignHoldingsLegend =>
      '\"approximate\": a purchase with no PTAX used the market rate. \"incomplete\": an operation was left with no exchange rate, or a sale was larger than the purchases entered, and the cost is understated.';

  @override
  String foreignReturnOf(int year) {
    return 'Return of $year';
  }

  @override
  String get foreignExportCsv => 'Export CSV';

  @override
  String foreignNoPosition(String previous, String current) {
    return 'No foreign position on $previous or $current.';
  }

  @override
  String get foreignNoCustody => 'No custody';

  @override
  String foreignHoldingNone(String date) {
    return '$date: —';
  }

  @override
  String foreignHoldingValue(
    String date,
    String cost,
    String quantity,
    String approximate,
    String incomplete,
  ) {
    return '$date: $cost ($quantity units)$approximate$incomplete';
  }

  @override
  String get foreignApproximate => ' · approximate';

  @override
  String get foreignIncomplete => ' · incomplete';

  @override
  String foreignFichaLine(String ficha) {
    return 'Record: $ficha';
  }

  @override
  String get foreignFichaNotFilled => 'not filled in';

  @override
  String foreignFichaGroup(String value) {
    return 'Group $value';
  }

  @override
  String foreignFichaCode(String value) {
    return 'Code $value';
  }

  @override
  String foreignFichaCountry(String value) {
    return 'Country $value';
  }

  @override
  String get foreignEditFichaTooltip =>
      'Edit the record\'s group, code and country';

  @override
  String get foreignCopyTooltip => 'Copy description';

  @override
  String get foreignDescriptionCopied => 'Description copied.';

  @override
  String foreignFichaDialogTitle(String ticker) {
    return 'Record of $ticker';
  }

  @override
  String get foreignFieldGroup => 'Group';

  @override
  String get foreignFieldCode => 'Code';

  @override
  String get foreignFieldCountry => 'Country';

  @override
  String get foreignFichaHelp =>
      'Copy from the Receita program; the app does not check it. Empty clears.';

  @override
  String get foreignFichaInvalid =>
      'Invalid record. Use digits only: group and code up to 2, country up to 3.';

  @override
  String get foreignCsvDialogTitle => 'Save Assets and Rights report';

  @override
  String foreignCsvSaved(int count) {
    return '$count rows saved.';
  }

  @override
  String foreignCsvFailed(String error) {
    return 'Could not save the CSV: $error';
  }

  @override
  String foreignDescHead(String ticker, String name, String custody) {
    return '$ticker — $name.$custody';
  }

  @override
  String foreignDescCustody(String label) {
    return ' Custody: $label.';
  }

  @override
  String foreignDescFicha(String ficha) {
    return 'Record: $ficha.';
  }

  @override
  String foreignDescPosition(String date, String units, String cost) {
    return 'Position on $date: $units, acquisition cost $cost.';
  }

  @override
  String get foreignDescNoPosition => 'no position';

  @override
  String foreignDescUnits(String quantity) {
    return '$quantity units';
  }

  @override
  String get foreignDescBasis =>
      'Acquisition cost converted into reais at the Central Bank\'s closing (selling) rate on the date of each purchase.';

  @override
  String tickerLabel(String assetClass) {
    return 'Ticker ($assetClass)';
  }

  @override
  String get classLabel => 'Class';

  @override
  String get searchButton => 'Search';

  @override
  String get quoteSourceUnavailable =>
      'Quote source unavailable right now (limit or network) — try again in a moment.';

  @override
  String quoteSearchFailed(String ticker, String assetClass) {
    return 'Could not fetch the quote for $ticker ($assetClass)';
  }

  @override
  String get hintRealEstate => 'Downtown apartment';

  @override
  String get hintUnlistedCompany => 'Company XYZ';

  @override
  String get hintFund => 'XYZ FIC FIM fund';

  @override
  String get hintFixedIncome => 'CDB Bank X';

  @override
  String get addAssetNameLabel => 'Name';

  @override
  String addAssetLookupFailed(String ticker, String assetClass) {
    return 'Could not look up $ticker ($assetClass) — check the ticker.';
  }

  @override
  String get addAssetFundTerm => 'Fund type (for the semiannual quota tax)';

  @override
  String get addAssetEquity => 'Equity stake';

  @override
  String addAssetEquityPercent(String title, String percent) {
    return '$title ($percent%)';
  }

  @override
  String addAssetEquityPercentValue(
    String title,
    String percent,
    String value,
  ) {
    return '$title ($percent% — $value)';
  }

  @override
  String get addAssetSharesOwned => 'Shares owned';

  @override
  String get addAssetTotalShares => 'Company\'s total shares';

  @override
  String addAssetValuation(String currency) {
    return 'Company valuation ($currency)';
  }

  @override
  String get custodianInstitution => 'Institution';

  @override
  String get custodianHolder => 'Holder';

  @override
  String get custodianAdd => 'Add';

  @override
  String get custodianNone => 'No custodians yet.';

  @override
  String get custodianEditTitle => 'Edit custodian';

  @override
  String get ptxTitleNew => 'New transaction';

  @override
  String get ptxTitleEdit => 'Edit transaction';

  @override
  String get ptxType => 'Type';

  @override
  String get ptxNeedFund => 'Add a fund asset first.';

  @override
  String get ptxNeedAsset => 'Add an asset first, in the Portfolio tab.';

  @override
  String get ptxAsset => 'Asset';

  @override
  String get ptxIssuer => 'Issuer (optional)';

  @override
  String get ptxIndex => 'Index';

  @override
  String get ptxRate => 'Contracted rate (%)';

  @override
  String ptxMaturity(String date) {
    return 'Maturity: $date';
  }

  @override
  String get ptxMaturityOptional => 'Maturity (optional)';

  @override
  String get ptxLiquidity => 'Liquidity';

  @override
  String get ptxQuantity => 'Quantity';

  @override
  String get ptxQuotaValue => 'Quota value';

  @override
  String get ptxUnitPrice => 'Unit price';

  @override
  String get ptxFee => 'Fee (optional)';

  @override
  String ptxTotal(String value) {
    return 'Total: $value';
  }

  @override
  String get ptxAmount => 'Amount';

  @override
  String ptxForeignTax(String currency) {
    return 'Foreign tax withheld ($currency, optional)';
  }

  @override
  String ptxTaxPaidOn(String date) {
    return 'Tax paid on: $date';
  }

  @override
  String get ptxTaxPaidOnHint =>
      'Tax payment date (optional; default: the income date)';

  @override
  String get ptxLossCredit => 'Loss credit used (income, R\$, optional)';

  @override
  String get ptxLossCreditHelp => 'Empty = unknown; 0 = no credit was used.';

  @override
  String get ptxCustodianFrom => 'Custodian (from)';

  @override
  String get ptxCustodianOptional => 'Custodian (optional)';

  @override
  String get ptxCustodianNone => 'None';

  @override
  String get ptxCustodianTo => 'Custodian (to)';

  @override
  String ptxDate(String date) {
    return 'Date: $date';
  }

  @override
  String get ptxSave => 'Save';

  @override
  String get ptxTypeCompra => 'Buy';

  @override
  String get ptxTypeVenda => 'Sell';

  @override
  String get ptxTypeAporte => 'Deposit';

  @override
  String get ptxTypeRetirada => 'Withdrawal';

  @override
  String get ptxTypeProvento => 'Income';

  @override
  String get ptxTypeTransferencia => 'Transfer';

  @override
  String get ptxTypeComeCotas => 'Fund tax withholding';

  @override
  String get ptxTypeAmortizacao => 'Amortization';

  @override
  String get ptxLiqDaily => 'Daily';

  @override
  String get ptxLiqAtMaturity => 'At maturity';

  @override
  String get ptxLiqOther => 'Other';

  @override
  String get ptxIdxFixed => 'Fixed rate';

  @override
  String get ptxIdxOther => 'Other';

  @override
  String get ptxAmortNoShares =>
      'An amortization needs fund quotas in the portfolio on that date.';

  @override
  String ptxAmortExceeds(String amount, String position) {
    return 'The amortized amount ($amount) exceeds the position value on that date ($position).';
  }

  @override
  String get madTitleCompany => 'Company details';

  @override
  String get madTitleProperty => 'Property details';

  @override
  String get madEquityTitle => 'Equity stake';

  @override
  String madEquityPct(String percent) {
    return 'Equity stake ($percent%)';
  }

  @override
  String madEquityPctValue(String percent, String value, String currency) {
    return 'Equity stake ($percent% — $value $currency)';
  }

  @override
  String get madEquityNote =>
      'The percentage is always calculated from shares owned / total company shares — never typed directly.';

  @override
  String get madSharesOwned => 'Shares owned';

  @override
  String get madTotalShares => 'Total shares';

  @override
  String madCompanyValuation(String currency) {
    return 'Company valuation ($currency)';
  }

  @override
  String get madSaveEquity => 'Save stake';

  @override
  String get madHistory => 'Valuation history';

  @override
  String madHistoryLatest(String value, String currency) {
    return 'Valuation history (latest: $value $currency)';
  }

  @override
  String get madNoValuations => 'No valuations recorded yet.';

  @override
  String get madPickDate => 'Pick a date';

  @override
  String madDate(String date) {
    return 'Date: $date';
  }

  @override
  String madValue(String currency) {
    return 'Value ($currency)';
  }

  @override
  String get madNotes => 'Notes (optional)';

  @override
  String get madAddValuation => 'Add valuation';

  @override
  String get madAttachments => 'Attachments';

  @override
  String get madNoAttachments => 'No attachments yet.';

  @override
  String get madDocType => 'Document type (optional)';

  @override
  String get madDocTypeHint => 'Deed, transfer tax, cap table...';

  @override
  String get madAddAttachment => 'Add attachment';

  @override
  String get txhEmpty => 'No transactions logged yet.';

  @override
  String get txhImportTooltip => 'Import CSV from desktop';

  @override
  String get txhExportTooltip => 'Export CSV to desktop';

  @override
  String get txhExportDialogTitle => 'Save transactions for the desktop';

  @override
  String txhExported(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count rows saved.',
      one: '1 row saved.',
      zero: '0 rows saved.',
    );
    return '$_temp0';
  }

  @override
  String txhExportedSkipped(int count) {
    return '$count left out (transfer, amortization or removed asset).';
  }

  @override
  String txhExportFailed(String error) {
    return 'Couldn\'t save the CSV: $error';
  }

  @override
  String txhImported(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count transactions imported.',
      one: '1 transaction imported.',
      zero: '0 transactions imported.',
    );
    return '$_temp0';
  }

  @override
  String txhImportFailed(String error) {
    return 'Couldn\'t import the CSV: $error';
  }

  @override
  String txhFee(String value) {
    return 'fee $value';
  }

  @override
  String txhWithheld(String value) {
    return 'withheld $value';
  }

  @override
  String txhPaidOn(String date) {
    return 'paid on $date';
  }

  @override
  String txhLossCreditUsed(String value) {
    return 'loss credit used $value';
  }

  @override
  String get txhPreviewTitle => 'Import from desktop';

  @override
  String txhPreviewNewTransactions(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count new transactions',
      one: '1 new transaction',
      zero: '0 new transactions',
    );
    return '$_temp0';
  }

  @override
  String txhPreviewNewAssets(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count new assets',
      one: '1 new asset',
      zero: '0 new assets',
    );
    return '$_temp0';
  }

  @override
  String txhPreviewNewCustodians(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count new custodians',
      one: '1 new custodian',
      zero: '0 new custodians',
    );
    return '$_temp0';
  }

  @override
  String txhPreviewDuplicates(int count) {
    return '$count already exist (ignored)';
  }

  @override
  String txhPreviewSkipped(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count transfers skipped (the CSV has no destination)',
      one: '1 transfer skipped (the CSV has no destination)',
      zero: '0 transfers skipped (the CSV has no destination)',
    );
    return '$_temp0';
  }

  @override
  String txhPreviewIssues(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count rows with problems (not imported):',
      one: '1 row with a problem (not imported):',
      zero: '0 rows with problems (not imported):',
    );
    return '$_temp0';
  }

  @override
  String txhPreviewIssueLine(int line, String message) {
    return 'line $line: $message';
  }

  @override
  String txhPreviewMore(int count) {
    return '… and $count more';
  }

  @override
  String get txhCancel => 'Cancel';

  @override
  String get txhImport => 'Import';

  @override
  String get importIssueEmptyFile => 'empty file';

  @override
  String importIssueMissingColumn(String name) {
    return 'column \'$name\' missing from the header';
  }

  @override
  String importIssueInvalidDate(String raw) {
    return 'invalid date: \'$raw\' (expected YYYY-MM-DD or DD/MM/YYYY)';
  }

  @override
  String importIssueUnknownType(String raw) {
    return 'unknown type: \'$raw\'';
  }

  @override
  String importIssueInvalidNumber(String name, String raw) {
    return 'invalid number in \'$name\': \'$raw\'';
  }

  @override
  String get importIssueTotalEmpty => 'total_value is empty';

  @override
  String importIssueUnknownClass(String raw) {
    return 'unknown class: \'$raw\'';
  }

  @override
  String importIssueTickerRequired(String type) {
    return 'ticker required for $type';
  }

  @override
  String importIssueNewAssetNeedsClass(String ticker) {
    return 'new asset \'$ticker\': provide asset_class';
  }

  @override
  String importIssueFundOnly(String type) {
    return '$type only applies to fund-class assets';
  }

  @override
  String importIssueQuantityRequired(String type) {
    return 'quantity required for $type';
  }

  @override
  String importIssueUnitPriceRequired(String type) {
    return 'unit_price required for $type';
  }

  @override
  String get finErrWatchlistNameRequired => 'Enter the watchlist name';

  @override
  String get finErrTargetPriceMustBePositive =>
      'The target price must be greater than zero';

  @override
  String get finErrAssetAlreadyInWatchlist =>
      'This asset is already in the watchlist';

  @override
  String get portfolioWatchlists => 'Watchlists';

  @override
  String get watchlistsTitle => 'Watchlists';

  @override
  String get watchlistsFavorites => 'Favorites';

  @override
  String get watchlistsFavoritesEmpty => 'No favorites yet.';

  @override
  String get watchlistsEmpty => 'No watchlists yet.';

  @override
  String get watchlistsSelect => 'Watchlist';

  @override
  String get watchlistsNewName => 'New watchlist';

  @override
  String get watchlistsCreate => 'Create';

  @override
  String get watchlistsConfirmDelete => 'Confirm?';

  @override
  String get watchlistsAddAsset => 'Add asset';

  @override
  String get watchlistsAssetField => 'Asset';

  @override
  String get watchlistsTargetField => 'Target price (optional)';

  @override
  String get watchlistsNotesField => 'Notes (optional)';

  @override
  String get watchlistsNoAssets => 'Register an asset in Portfolio first.';

  @override
  String watchlistsTarget(String value) {
    return 'Target: $value';
  }

  @override
  String get watchlistsNoTarget => 'No target';

  @override
  String get watchlistsRemove => 'Remove';

  @override
  String get favoriteAdd => 'Favorite';

  @override
  String get favoriteRemove => 'Remove favorite';

  @override
  String get portfolioProfitability => 'Profitability';

  @override
  String get profitabilityTitle => 'Profitability';

  @override
  String get profitabilityBeforeTax => 'Return before taxes';

  @override
  String get profitabilityRefresh => 'Update price history';

  @override
  String get profitabilityEmpty =>
      'No buys, sells or income on stocks, REITs, ETFs, BDRs or metals to measure.';

  @override
  String profitabilityCumulative(String value) {
    return 'Cumulative: $value';
  }

  @override
  String profitabilityTrailing12(String value) {
    return 'Last 12 months: $value';
  }

  @override
  String profitabilityFinalValue(String value) {
    return 'Final value: $value';
  }

  @override
  String profitabilityCumulativeShort(String value) {
    return 'Cumulative $value';
  }

  @override
  String profitabilityRefreshed(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count assets updated',
      one: '1 asset updated',
    );
    return '$_temp0';
  }

  @override
  String profitabilityFailed(String tickers) {
    return 'Failed: $tickers';
  }

  @override
  String get profitabilityExcludedTitle => 'Left out (stock split)';

  @override
  String get profitabilityExcludedNote =>
      'The app does not record splits; these assets are left out so the value is not wrong.';

  @override
  String profitabilityExcludedLine(String ticker, String date) {
    return '$ticker: split on $date';
  }

  @override
  String profitErrMissingMonthPrice(String ticker, String month) {
    return 'No price for $ticker in $month. Update the price history.';
  }

  @override
  String profitErrMissingFx(String currency) {
    return 'No exchange rate for $currency. Check the connection and try again.';
  }

  @override
  String get profitErrZeroDenominator =>
      'There is no base to compute this month\'s return.';

  @override
  String get finErrThesisTitleRequired => 'Enter the thesis title';

  @override
  String get finErrThesisNotFound => 'Thesis not found';

  @override
  String get portfolioTheses => 'Theses';

  @override
  String get portfolioMore => 'More';

  @override
  String get thesesTitle => 'Theses';

  @override
  String get thesesEmpty => 'No theses yet.';

  @override
  String get thesesGlobal => 'Global';

  @override
  String thesesSubtitle(String label, String date) {
    return '$label · $date';
  }

  @override
  String get thesesNew => 'New thesis';

  @override
  String get thesisEditTitle => 'Edit thesis';

  @override
  String get thesisFieldTitle => 'Title';

  @override
  String get thesisFieldAsset => 'Asset (optional)';

  @override
  String get thesisAssetGlobal => 'Global (no asset)';

  @override
  String get thesisFieldContent => 'Text (Markdown)';

  @override
  String get thesisDeleteTitle => 'Delete this thesis?';

  @override
  String get thesisDeleteBody =>
      'The thesis text will be deleted and cannot be recovered.';
}
