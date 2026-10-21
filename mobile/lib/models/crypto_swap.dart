import 'asset.dart';
import 'portfolio_transaction.dart';

/// Recusa de uma permuta, com a mensagem pronta pra mostrar na tela.
class CryptoSwapException implements Exception {
  final String message;
  const CryptoSwapException(this.message);

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
    throw const CryptoSwapException(
      'A permuta precisa de duas moedas diferentes.',
    );
  }
  for (final asset in [from, to]) {
    if (asset.assetClass != AssetClass.cripto) {
      throw CryptoSwapException(
        '${asset.ticker} não é cripto; a permuta só vale entre criptomoedas.',
      );
    }
    if (asset.currency != 'BRL') {
      throw CryptoSwapException(
        '${asset.ticker} está em ${asset.currency}; a permuta é apurada em reais.',
      );
    }
  }
  for (final (name, number) in [
    ('quantidade que sai', quantityOut),
    ('quantidade que entra', quantityIn),
    ('valor', valueBrl),
  ]) {
    if (!number.isFinite || number <= 0) {
      throw CryptoSwapException('O campo $name deve ser maior que zero.');
    }
  }
  if (fee != null && (!fee.isFinite || fee < 0)) {
    throw const CryptoSwapException('A taxa não pode ser negativa.');
  }
  if (quantityOut > available + 1e-9) {
    throw CryptoSwapException(
      'Saldo insuficiente de ${from.ticker}: tem $available e a troca entrega $quantityOut.',
    );
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
