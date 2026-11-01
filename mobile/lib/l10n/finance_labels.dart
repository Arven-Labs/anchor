import '../domain/finance_error.dart';
import '../models/general_transaction.dart';
import 'app_localizations.dart';

// Textos de Finanças (ADR 0001): erro por código, tipo de lançamento e nome de moeda. Um
// `switch` sem `default` nos enums faz o analisador acusar um valor novo sem texto.

String _financeCodeText(AppLocalizations l, FinanceErrorCode code, Map<String, String> args) =>
    switch (code) {
      FinanceErrorCode.unknownCurrency => l.finErrUnknownCurrency(args['currency'] ?? ''),
      FinanceErrorCode.amountMustBePositive => l.finErrAmountMustBePositive,
      FinanceErrorCode.invalidDate => l.finErrInvalidDate(args['date'] ?? ''),
      FinanceErrorCode.manualTypeOnly => l.finErrManualTypeOnly,
      FinanceErrorCode.liabilityNameRequired => l.finErrLiabilityNameRequired,
      FinanceErrorCode.borrowerNameRequired => l.finErrBorrowerNameRequired,
      FinanceErrorCode.principalMustBePositive => l.finErrPrincipalMustBePositive,
      FinanceErrorCode.termMustBePositive => l.finErrTermMustBePositive,
      FinanceErrorCode.negativeMonthlyRate => l.finErrNegativeMonthlyRate,
      FinanceErrorCode.repaidAmountMustBePositive => l.finErrRepaidAmountMustBePositive,
      FinanceErrorCode.receivedAmountNegative => l.finErrReceivedAmountNegative,
      FinanceErrorCode.loanCurrencyMismatch => l.finErrLoanCurrencyMismatch(
        args['loanCurrency'] ?? '',
        args['accountCurrency'] ?? '',
      ),
      FinanceErrorCode.categoryNameRequired => l.finErrCategoryNameRequired,
      FinanceErrorCode.categoryLimitMustBePositive => l.finErrCategoryLimitMustBePositive,
      FinanceErrorCode.transferAmountsMustBePositive => l.finErrTransferAmountsMustBePositive,
      FinanceErrorCode.transferSameAccount => l.finErrTransferSameAccount,
      FinanceErrorCode.transferSameCurrencyAmountsDiffer => l.finErrTransferSameCurrencyAmountsDiffer,
      FinanceErrorCode.accountNotFound => l.finErrAccountNotFound,
      FinanceErrorCode.accountCurrencyLocked => l.finErrAccountCurrencyLocked,
      FinanceErrorCode.borrowerHasLoans => l.finErrBorrowerHasLoans,
      FinanceErrorCode.borrowerNotFound => l.finErrBorrowerNotFound,
      FinanceErrorCode.loanNotFound => l.finErrLoanNotFound,
      FinanceErrorCode.linkedTransactionLocked => l.finErrLinkedTransactionLocked,
    };

/// O texto do erro para o usuário. Erro de Finanças vira a frase do `.arb` pelo código;
/// qualquer outro mostra a própria mensagem (erro de programador, como já era).
String financeErrorText(AppLocalizations l, Object error) => switch (error) {
  FinanceArgumentError(:final code, :final args) => _financeCodeText(l, code, args),
  FinanceStateError(:final code, :final args) => _financeCodeText(l, code, args),
  StateError(:final message) => message,
  _ => '$error',
};

String transactionTypeName(AppLocalizations l, GeneralTransactionType type) => switch (type) {
  GeneralTransactionType.receita => l.txTypeReceita,
  GeneralTransactionType.despesa => l.txTypeDespesa,
  GeneralTransactionType.transferenciaSaida => l.txTypeTransferenciaSaida,
  GeneralTransactionType.transferenciaEntrada => l.txTypeTransferenciaEntrada,
  GeneralTransactionType.parcelaDivida => l.txTypeParcelaDivida,
  GeneralTransactionType.emprestimoConcedido => l.txTypeEmprestimoConcedido,
  GeneralTransactionType.recebimentoEmprestimo => l.txTypeRecebimentoEmprestimo,
};

/// Nome da moeda no idioma da tela, ou o próprio código se não for do catálogo.
String currencyName(AppLocalizations l, String code) => switch (code) {
  'USD' => l.currencyUsd,
  'EUR' => l.currencyEur,
  'GBP' => l.currencyGbp,
  'JPY' => l.currencyJpy,
  'CHF' => l.currencyChf,
  'CAD' => l.currencyCad,
  'AUD' => l.currencyAud,
  'NZD' => l.currencyNzd,
  'CNY' => l.currencyCny,
  'BRL' => l.currencyBrl,
  'ARS' => l.currencyArs,
  'UYU' => l.currencyUyu,
  'PYG' => l.currencyPyg,
  'CLP' => l.currencyClp,
  'COP' => l.currencyCop,
  'PEN' => l.currencyPen,
  'MXN' => l.currencyMxn,
  _ => code,
};
