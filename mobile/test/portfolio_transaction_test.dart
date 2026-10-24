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

  test('o imposto retido no exterior vai e volta pelo mapa do banco', () {
    final withheld = PortfolioTransaction(
      id: 4,
      assetId: 1,
      type: TransactionType.provento,
      totalValue: 100,
      foreignTaxWithheld: 30,
      date: DateTime(2026, 2, 10),
      createdAt: DateTime(2026, 2, 10),
    );
    final map = withheld.toMap();

    expect(map['foreign_tax_withheld'], 30);
    expect(PortfolioTransaction.fromMap(map).foreignTaxWithheld, 30);
    expect(
      PortfolioTransaction.fromMap(map..remove('foreign_tax_withheld'))
          .foreignTaxWithheld,
      isNull,
      reason: 'linha antiga sem a coluna lê como sem retenção',
    );
    expect(tx().toMap()['foreign_tax_withheld'], isNull);
  });

  test('o crédito de prejuízo usado vai e volta; nulo e 0 são respostas diferentes', () {
    PortfolioTransaction comeCotas(double? used) => PortfolioTransaction(
      id: 5,
      assetId: 1,
      type: TransactionType.comeCotas,
      quantity: 5,
      unitPrice: 12,
      totalValue: 60,
      lossCreditUsed: used,
      date: DateTime(2025, 11, 30),
      createdAt: DateTime(2025, 11, 30),
    );

    final map = comeCotas(150).toMap();
    expect(map['loss_credit_used'], 150);
    expect(PortfolioTransaction.fromMap(map).lossCreditUsed, 150);
    // 0 = "nenhum crédito usado" e não vira nulo
    expect(PortfolioTransaction.fromMap(comeCotas(0).toMap()).lossCreditUsed, 0);
    // nulo = não informado, e linha antiga sem a coluna lê assim
    expect(comeCotas(null).toMap()['loss_credit_used'], isNull);
    expect(
      PortfolioTransaction.fromMap(map..remove('loss_credit_used')).lossCreditUsed,
      isNull,
    );
  });

  test('sem taxa fica nula, e linha antiga sem a coluna lê como sem taxa', () {
    final map = tx().toMap();

    expect(map['fee'], isNull);
    expect(PortfolioTransaction.fromMap(map).fee, isNull);
    expect(PortfolioTransaction.fromMap(map..remove('fee')).fee, isNull);
  });
}
