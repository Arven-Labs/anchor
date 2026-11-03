import '../domain/fund_tax.dart';
import '../domain/number_locale.dart';
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

/// A frase de uma amortização que não cabe no histórico do fundo.
String amortizationProblemText(AppLocalizations l, AmortizationProblem problem) =>
    switch (problem.code) {
      AmortizationProblemCode.noSharesOnDate => l.ptxAmortNoShares,
      AmortizationProblemCode.exceedsPosition => l.ptxAmortExceeds(
        formatFixed(problem.amount ?? 0, 2, locale: l.localeName),
        formatFixed(problem.positionValue ?? 0, 2, locale: l.localeName),
      ),
    };
