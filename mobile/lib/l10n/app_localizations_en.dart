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
}
