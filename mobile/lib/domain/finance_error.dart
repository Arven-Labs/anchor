// Erros de Finanças com código e argumentos (ADR 0001). O texto que o usuário lê vem dos
// `.arb` pelo código (`l10n/finance_labels.dart`); a mensagem da exceção é inglês para quem
// depura. Os tipos seguem sendo `ArgumentError` e `StateError`, então `on StateError` e
// `throwsArgumentError` continuam valendo.

enum FinanceErrorCode {
  unknownCurrency,
  amountMustBePositive,
  invalidDate,
  manualTypeOnly,
  liabilityNameRequired,
  borrowerNameRequired,
  principalMustBePositive,
  termMustBePositive,
  negativeMonthlyRate,
  repaidAmountMustBePositive,
  receivedAmountNegative,
  loanCurrencyMismatch,
  categoryNameRequired,
  categoryLimitMustBePositive,
  transferAmountsMustBePositive,
  transferSameAccount,
  transferSameCurrencyAmountsDiffer,
  accountNotFound,
  accountCurrencyLocked,
  borrowerHasLoans,
  borrowerNotFound,
  loanNotFound,
  linkedTransactionLocked,
  balanceMustBeNonNegative,
  dateInFuture,
  nothingToReconcile,
  bankBalanceBelowLedger,
  selicNotCovered,
  pctOfCdiOutOfRange,
}

/// Mensagem de desenvolvedor (inglês) de cada código, com os argumentos no texto.
String _developerMessage(FinanceErrorCode code, Map<String, String> args) => switch (code) {
  FinanceErrorCode.unknownCurrency => 'Currency not in the catalog: ${args['currency']}',
  FinanceErrorCode.amountMustBePositive => 'The amount must be greater than zero',
  FinanceErrorCode.invalidDate => 'Invalid date: ${args['date']}',
  FinanceErrorCode.manualTypeOnly =>
    'Only income and expense are entered by hand; use createTransfer, createLiability or createLoan',
  FinanceErrorCode.liabilityNameRequired => 'The debt name is required',
  FinanceErrorCode.borrowerNameRequired => "The borrower's name is required",
  FinanceErrorCode.principalMustBePositive => 'The principal must be greater than zero',
  FinanceErrorCode.termMustBePositive => 'The term (in months) must be greater than zero',
  FinanceErrorCode.negativeMonthlyRate => "The monthly rate can't be negative",
  FinanceErrorCode.repaidAmountMustBePositive => 'The amount repaid must be greater than zero',
  FinanceErrorCode.receivedAmountNegative => "The amount received can't be negative",
  FinanceErrorCode.loanCurrencyMismatch =>
    'The loan is in ${args['loanCurrency']} and the account in ${args['accountCurrency']}',
  FinanceErrorCode.categoryNameRequired => 'The category name is required',
  FinanceErrorCode.categoryLimitMustBePositive => 'The monthly limit must be greater than zero',
  FinanceErrorCode.transferAmountsMustBePositive => 'The amounts must be greater than zero',
  FinanceErrorCode.transferSameAccount => 'Choose two different accounts',
  FinanceErrorCode.transferSameCurrencyAmountsDiffer =>
    'Accounts in the same currency receive exactly what left',
  FinanceErrorCode.accountNotFound => 'Account not found',
  FinanceErrorCode.accountCurrencyLocked =>
    "The currency can't change on an account that already has transactions",
  FinanceErrorCode.borrowerHasLoans => 'This borrower still has loans: delete them first',
  FinanceErrorCode.borrowerNotFound => 'Borrower not found',
  FinanceErrorCode.loanNotFound => 'Loan not found',
  FinanceErrorCode.linkedTransactionLocked =>
    'A debt installment and a loan transaction are only deleted together with the debt or the loan',
  FinanceErrorCode.balanceMustBeNonNegative => 'The bank balance must be a number of zero or more',
  FinanceErrorCode.dateInFuture => 'The balance date cannot be in the future: ${args['date']}',
  FinanceErrorCode.nothingToReconcile => 'The transactions already match the bank balance: nothing to reconcile',
  FinanceErrorCode.bankBalanceBelowLedger =>
    'The bank balance is lower than the balance from the transactions: a withdrawal may be missing, or use the balance adjustment',
  FinanceErrorCode.selicNotCovered =>
    'The Selic history does not cover ${args['from']} to ${args['to']}: fetch the rates and try again',
  FinanceErrorCode.pctOfCdiOutOfRange => 'The percentage of the CDI must be between 0 and 200%',
};

/// Argumento inválido de Finanças. [args] leva o que a frase do usuário precisa (a moeda,
/// a data), como texto.
class FinanceArgumentError extends ArgumentError {
  final FinanceErrorCode code;
  final Map<String, String> args;

  FinanceArgumentError._(this.code, this.args, String message) : super(message);

  FinanceArgumentError(FinanceErrorCode code, [Map<String, String> args = const {}])
    : this._(code, args, _developerMessage(code, args));
}

/// Estado que não permite a operação (registro que não existe, conta travada) em Finanças.
class FinanceStateError extends StateError {
  final FinanceErrorCode code;
  final Map<String, String> args;

  FinanceStateError._(this.code, this.args, String message) : super(message);

  FinanceStateError(FinanceErrorCode code, [Map<String, String> args = const {}])
    : this._(code, args, _developerMessage(code, args));
}
