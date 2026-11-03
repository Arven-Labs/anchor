import 'asset.dart';
import 'portfolio_transaction.dart';

enum CryptoSwapErrorCode {
  needTwoDifferentCoins,
  notCrypto,
  notBrl,
  quantityOutMustBePositive,
  quantityInMustBePositive,
  valueMustBePositive,
  negativeFee,
  insufficientBalance,
}

/// Mensagem de desenvolvedor (inglês) de cada código; o texto da tela vem dos `.arb`
/// (`l10n/crypto_tax_labels.dart`), ADR 0001.
String _developerMessage(CryptoSwapErrorCode code, Map<String, String> args) => switch (code) {
  CryptoSwapErrorCode.needTwoDifferentCoins => 'A swap needs two different coins',
  CryptoSwapErrorCode.notCrypto => '${args['ticker']} is not crypto',
  CryptoSwapErrorCode.notBrl => '${args['ticker']} is in ${args['currency']}, not BRL',
  CryptoSwapErrorCode.quantityOutMustBePositive => 'The quantity out must be greater than zero',
  CryptoSwapErrorCode.quantityInMustBePositive => 'The quantity in must be greater than zero',
  CryptoSwapErrorCode.valueMustBePositive => 'The value must be greater than zero',
  CryptoSwapErrorCode.negativeFee => "The fee can't be negative",
  CryptoSwapErrorCode.insufficientBalance =>
    'Insufficient ${args['ticker']} balance: ${args['available']} available, ${args['requested']} requested',
};

/// Recusa de uma permuta: [code] e [args] dizem o motivo; [message] é para quem depura.
class CryptoSwapException implements Exception {
  final CryptoSwapErrorCode code;
  final Map<String, String> args;
  final String message;

  CryptoSwapException(this.code, [this.args = const {}]) : message = _developerMessage(code, args);

  @override
  String toString() => message;
}

/// Permuta cripto↔cripto, mesma regra de `record_crypto_swap`
/// (`desktop/src-tauri/src/commands/transaction.rs`): pra Receita é a venda da
/// moeda entregue pelo valor de mercado em reais, então vira uma venda dela e
/// uma compra da moeda recebida, no mesmo valor e na mesma data. Função pura —
/// quem grava as duas juntas é `PortfolioRepository.insertTransactions`.
///
/// `available` é o saldo atual da moeda entregue (venda sem saldo é recusada,
/// como no desktop). A taxa (`fee`, em reais) vai só na perna da venda, onde
/// reduz o ganho, como em `record_crypto_swap`.
({PortfolioTransaction sale, PortfolioTransaction purchase}) buildCryptoSwap({
  required Asset from,
  required Asset to,
  required double quantityOut,
  required double quantityIn,
  required double valueBrl,
  required DateTime date,
  required double available,
  double? fee,
  int? custodiaId,
  DateTime? now,
}) {
  if (from.id == null || to.id == null || from.id == to.id) {
    throw CryptoSwapException(CryptoSwapErrorCode.needTwoDifferentCoins);
  }
  for (final asset in [from, to]) {
    if (asset.assetClass != AssetClass.cripto) {
      throw CryptoSwapException(CryptoSwapErrorCode.notCrypto, {'ticker': asset.ticker});
    }
    if (asset.currency != 'BRL') {
      throw CryptoSwapException(CryptoSwapErrorCode.notBrl, {
        'ticker': asset.ticker,
        'currency': asset.currency,
      });
    }
  }
  for (final (code, number) in [
    (CryptoSwapErrorCode.quantityOutMustBePositive, quantityOut),
    (CryptoSwapErrorCode.quantityInMustBePositive, quantityIn),
    (CryptoSwapErrorCode.valueMustBePositive, valueBrl),
  ]) {
    if (!number.isFinite || number <= 0) throw CryptoSwapException(code);
  }
  if (fee != null && (!fee.isFinite || fee < 0)) {
    throw CryptoSwapException(CryptoSwapErrorCode.negativeFee);
  }
  if (quantityOut > available + 1e-9) {
    throw CryptoSwapException(CryptoSwapErrorCode.insufficientBalance, {
      'ticker': from.ticker,
      'available': '$available',
      'requested': '$quantityOut',
    });
  }

  final createdAt = now ?? DateTime.now();
  final notes = 'Swap ${from.ticker} → ${to.ticker}';
  PortfolioTransaction leg(
    Asset asset,
    TransactionType type,
    double quantity, {
    double? legFee,
  }) {
    return PortfolioTransaction(
      assetId: asset.id,
      type: type,
      quantity: quantity,
      unitPrice: valueBrl / quantity,
      totalValue: valueBrl,
      fee: legFee,
      custodiaId: custodiaId,
      date: date,
      notes: notes,
      createdAt: createdAt,
    );
  }

  return (
    sale: leg(from, TransactionType.venda, quantityOut, legFee: fee),
    purchase: leg(to, TransactionType.compra, quantityIn),
  );
}
