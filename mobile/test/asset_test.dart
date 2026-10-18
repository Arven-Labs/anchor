import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/models/asset.dart';

void main() {
  Asset asset({
    double? sharesOwned,
    double? totalShares,
    double? companyValuation,
  }) {
    return Asset(
      ticker: 'EMPRESA-XYZ',
      name: 'Empresa XYZ',
      assetClass: AssetClass.empresaNaoListada,
      currency: 'BRL',
      createdAt: DateTime(2026, 1, 1),
      equitySharesOwned: sharesOwned,
      equityTotalShares: totalShares,
      equityCompanyValuation: companyValuation,
    );
  }

  test(
    'equityPercentual e equityParticipationValue calculados a partir das 3 entradas',
    () {
      final a = asset(
        sharesOwned: 100,
        totalShares: 1000,
        companyValuation: 2000000,
      );

      expect(a.equityPercentual, 0.1);
      expect(a.equityParticipationValue, 200000);
    },
  );

  test('equityPercentual é null sem os 3 campos preenchidos', () {
    expect(asset().equityPercentual, isNull);
    expect(asset(sharesOwned: 100).equityPercentual, isNull);
    expect(
      asset(sharesOwned: 100, totalShares: 1000).equityParticipationValue,
      isNull,
    );
  });

  test('equityPercentual é null quando o total de cotas é zero', () {
    expect(asset(sharesOwned: 100, totalShares: 0).equityPercentual, isNull);
  });

  test('etf_renda_fixa (ETF de renda fixa da B3) é cotado, em BRL e sem campos fi_*', () {
    final etf = AssetClass.fromValue('etf_renda_fixa');

    expect(etf, AssetClass.etfRendaFixa);
    expect(etf.hasAutoQuote, isTrue);
    expect(etf.defaultCurrency, 'BRL');
    expect(etf.isFixedIncome, isFalse);
    expect(etf.tickerHint, 'AUPO11');
  });
}
