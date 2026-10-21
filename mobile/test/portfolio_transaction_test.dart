import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/models/portfolio_transaction.dart';

void main() {
  PortfolioTransaction tx({double? fee}) => PortfolioTransaction(
    id: 3,
    assetId: 1,
    type: TransactionType.compra,
    quantity: 2,
    unitPrice: 50,
    totalValue: 100,
    fee: fee,
    date: DateTime(2026, 2, 10),
    createdAt: DateTime(2026, 2, 10),
  );

  test('a taxa vai e volta pelo mapa do banco', () {
    final map = tx(fee: 1.5).toMap();

    expect(map['fee'], 1.5);
    expect(PortfolioTransaction.fromMap(map).fee, 1.5);
  });

  test('sem taxa fica nula, e linha antiga sem a coluna lê como sem taxa', () {
    final map = tx().toMap();

    expect(map['fee'], isNull);
    expect(PortfolioTransaction.fromMap(map).fee, isNull);
    expect(PortfolioTransaction.fromMap(map..remove('fee')).fee, isNull);
  });
}
