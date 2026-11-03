import '../domain/fund_tax.dart';
import '../domain/number_locale.dart';
import '../domain/transactions_csv_import.dart' show ImportIssue, ImportIssueCode;
import '../models/portfolio_transaction.dart';
import 'app_localizations.dart';

// Textos da nova transação (ADR 0001). Um `switch` sem `default` faz o analisador acusar
// um tipo novo sem texto. O valor gravado no banco (`TransactionType.value`) não muda.

String transactionTypeLabel(AppLocalizations l, TransactionType type) => switch (type) {
  TransactionType.compra => l.ptxTypeCompra,
  TransactionType.venda => l.ptxTypeVenda,
  TransactionType.aporte => l.ptxTypeAporte,
  TransactionType.retirada => l.ptxTypeRetirada,
  TransactionType.provento => l.ptxTypeProvento,
  TransactionType.transferencia => l.ptxTypeTransferencia,
  TransactionType.comeCotas => l.ptxTypeComeCotas,
  TransactionType.amortizacao => l.ptxTypeAmortizacao,
};

/// Nome da liquidez de renda fixa (`fiLiquidezOptions`); código desconhecido volta como veio.
String fiLiquidityName(AppLocalizations l, String code) => switch (code) {
  'diaria' => l.ptxLiqDaily,
  'no_vencimento' => l.ptxLiqAtMaturity,
  'outro' => l.ptxLiqOther,
  _ => code,
};

/// Nome do indexador (`fiIndexadores`): `CDI`, `IPCA` e `SELIC` são siglas e ficam iguais.
String fiIndexName(AppLocalizations l, String code) => switch (code) {
  'PREFIXADO' => l.ptxIdxFixed,
  'OUTRO' => l.ptxIdxOther,
  _ => code,
};

/// A frase de um problema do import de CSV. O tipo e o ticker citados ficam como estão no
/// arquivo (o usuário os procura lá).
String importIssueText(AppLocalizations l, ImportIssue issue) {
  final args = issue.args;
  return switch (issue.code) {
    ImportIssueCode.emptyFile => l.importIssueEmptyFile,
    ImportIssueCode.missingColumn => l.importIssueMissingColumn(args['name'] ?? ''),
    ImportIssueCode.invalidDate => l.importIssueInvalidDate(args['raw'] ?? ''),
    ImportIssueCode.unknownType => l.importIssueUnknownType(args['raw'] ?? ''),
    ImportIssueCode.invalidNumber => l.importIssueInvalidNumber(
      args['name'] ?? '',
      args['raw'] ?? '',
    ),
    ImportIssueCode.totalEmpty => l.importIssueTotalEmpty,
    ImportIssueCode.unknownClass => l.importIssueUnknownClass(args['raw'] ?? ''),
    ImportIssueCode.tickerRequired => l.importIssueTickerRequired(args['type'] ?? ''),
    ImportIssueCode.newAssetNeedsClass => l.importIssueNewAssetNeedsClass(args['ticker'] ?? ''),
    ImportIssueCode.fundOnly => l.importIssueFundOnly(args['type'] ?? ''),
    ImportIssueCode.quantityRequired => l.importIssueQuantityRequired(args['type'] ?? ''),
    ImportIssueCode.unitPriceRequired => l.importIssueUnitPriceRequired(args['type'] ?? ''),
  };
}

/// A frase de uma amortização que não cabe no histórico do fundo.
String amortizationProblemText(AppLocalizations l, AmortizationProblem problem) =>
    switch (problem.code) {
      AmortizationProblemCode.noSharesOnDate => l.ptxAmortNoShares,
      AmortizationProblemCode.exceedsPosition => l.ptxAmortExceeds(
        formatFixed(problem.amount ?? 0, 2, locale: l.localeName),
        formatFixed(problem.positionValue ?? 0, 2, locale: l.localeName),
      ),
    };
