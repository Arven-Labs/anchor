import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/models/asset.dart';
import 'package:anchor_mobile/models/crypto_swap.dart';
import 'package:anchor_mobile/models/portfolio_transaction.dart';

void main() {
  Asset coin(int id, String ticker, {String currency = 'BRL', AssetClass assetClass = AssetClass.cripto}) => Asset(
    id: id,
    ticker: ticker,
    name: ticker,
    assetClass: assetClass,
    currency: currency,
    createdAt: DateTime(2026, 1, 1),
  );

  final btc = coin(1, 'BTC');
  final eth = coin(2, 'ETH');
  final date = DateTime(2026, 2, 10);

  ({PortfolioTransaction sale, PortfolioTransaction purchase}) swap({
    Asset? from,
    Asset? to,
    double quantityOut = 1,
    double quantityIn = 20,
    double value = 40000,
    double available = 1,
    double? fee,
    int? custodiaId,
  }) => buildCryptoSwap(
    from: from ?? btc,
    to: to ?? eth,
    quantityOut: quantityOut,
    quantityIn: quantityIn,
    valueBrl: value,
    date: date,
    available: available,
    fee: fee,
    custodiaId: custodiaId,
    now: DateTime(2026, 2, 10, 12),
  );

  test('a permuta é uma venda do que sai e uma compra do que entra, pelo mesmo valor', () {
    final result = swap(custodiaId: 7);

    expect(result.sale.type, TransactionType.venda);
    expect(result.purchase.type, TransactionType.compra);
    expect((result.sale.assetId, result.purchase.assetId), (1, 2));
    expect((result.sale.quantity, result.purchase.quantity), (1.0, 20.0));
    expect(result.sale.totalValue, 40000);
    expect(result.purchase.totalValue, 40000);
    expect(result.sale.unitPrice, 40000);
    expect(result.purchase.unitPrice, 2000);
    expect((result.sale.date, result.purchase.date), (date, date));
    expect((result.sale.custodiaId, result.purchase.custodiaId), (7, 7));
    expect(result.sale.notes, 'Swap BTC → ETH');
    expect(result.purchase.notes, 'Swap BTC → ETH');
  });

  test('a taxa vai só na perna da venda', () {
    final result = swap(fee: 150);

    expect(result.sale.fee, 150);
    expect(result.purchase.fee, isNull);
    expect(swap().sale.fee, isNull, reason: 'sem taxa informada');
  });

  test('cada recusa leva o código e os argumentos que a tela traduz', () {
    CryptoSwapException refusal(void Function() build) {
      try {
        build();
      } on CryptoSwapException catch (e) {
        return e;
      }
      fail('devia recusar');
    }

    expect(refusal(() => swap(to: btc)).code, CryptoSwapErrorCode.needTwoDifferentCoins);
    final notCrypto = refusal(() => swap(to: coin(3, 'PETR4', assetClass: AssetClass.acaoBr)));
    expect((notCrypto.code, notCrypto.args['ticker']), (CryptoSwapErrorCode.notCrypto, 'PETR4'));
    final notBrl = refusal(() => swap(to: coin(3, 'SOL', currency: 'USD')));
    expect(notBrl.code, CryptoSwapErrorCode.notBrl);
    expect((notBrl.args['ticker'], notBrl.args['currency']), ('SOL', 'USD'));
    expect(refusal(() => swap(quantityOut: 0)).code, CryptoSwapErrorCode.quantityOutMustBePositive);
    expect(refusal(() => swap(quantityIn: 0)).code, CryptoSwapErrorCode.quantityInMustBePositive);
    expect(refusal(() => swap(value: 0)).code, CryptoSwapErrorCode.valueMustBePositive);
    expect(refusal(() => swap(fee: -1)).code, CryptoSwapErrorCode.negativeFee);
    final balance = refusal(() => swap(quantityOut: 2, available: 1.5));
    expect(balance.code, CryptoSwapErrorCode.insufficientBalance);
    expect(
      (balance.args['ticker'], balance.args['available'], balance.args['requested']),
      ('BTC', '1.5', '2.0'),
    );
  });

  test('taxa negativa ou inválida é recusada', () {
    expect(() => swap(fee: -1), throwsA(isA<CryptoSwapException>()));
    expect(() => swap(fee: double.nan), throwsA(isA<CryptoSwapException>()));
    expect(() => swap(fee: 0), returnsNormally);
  });

  test('entregar exatamente o saldo é permitido', () {
    expect(() => swap(quantityOut: 1, available: 1), returnsNormally);
  });

  test('recusa o que não é uma permuta válida entre criptos em reais', () {
    void refused(void Function() build, String reason) =>
        expect(build, throwsA(isA<CryptoSwapException>()), reason: reason);

    refused(() => swap(to: btc), 'mesma moeda');
    refused(() => swap(to: coin(2, 'ETH').copyIdless()), 'ativo sem id');
    refused(() => swap(to: coin(3, 'PETR4', assetClass: AssetClass.acaoBr)), 'não é cripto');
    refused(() => swap(to: coin(3, 'SOL', currency: 'USD')), 'moeda do ativo que entra não é BRL');
    refused(() => swap(from: coin(3, 'SOL', currency: 'USD'), available: 5), 'moeda do ativo que sai não é BRL');
    refused(() => swap(value: 0), 'valor zero');
    refused(() => swap(quantityOut: -1), 'quantidade que sai negativa');
    refused(() => swap(quantityIn: 0), 'quantidade que entra zero');
    refused(() => swap(value: double.nan), 'valor inválido');
    refused(() => swap(quantityOut: 1.5, available: 1), 'saldo insuficiente');
  });
}

extension on Asset {
  /// Cópia sem `id`, pra simular um ativo ainda não salvo.
  Asset copyIdless() => Asset(
    ticker: ticker,
    name: name,
    assetClass: assetClass,
    currency: currency,
    createdAt: createdAt,
  );
}
