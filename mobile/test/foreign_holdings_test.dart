import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/foreign_tax.dart';
import 'package:anchor_mobile/domain/fx_history.dart';
import 'package:anchor_mobile/domain/ptax_series.dart';
import 'package:anchor_mobile/models/asset.dart';
import 'package:anchor_mobile/models/portfolio_transaction.dart';

void main() {
  Asset stock(
    int id,
    String ticker, {
    String currency = 'USD',
    AssetClass assetClass = AssetClass.acaoInternacional,
  }) => Asset(
    id: id,
    ticker: ticker,
    name: ticker,
    assetClass: assetClass,
    currency: currency,
    createdAt: DateTime(2024, 1, 1),
  );

  final assets = [stock(1, 'AAPL'), stock(2, 'MSFT')];

  var nextId = 0;
  PortfolioTransaction trade(
    int assetId,
    String date,
    TransactionType type,
    double quantity,
    double gross, {
    double? fee,
  }) => PortfolioTransaction(
    id: ++nextId,
    assetId: assetId,
    type: type,
    quantity: quantity,
    unitPrice: gross / quantity,
    totalValue: gross,
    fee: fee,
    date: DateTime.parse(date),
    createdAt: DateTime(2024, 1, 1),
  );

  PortfolioTransaction buy(
    int asset,
    String date,
    double quantity,
    double gross, {
    double? fee,
  }) => trade(asset, date, TransactionType.compra, quantity, gross, fee: fee);
  PortfolioTransaction sell(
    int asset,
    String date,
    double quantity,
    double gross,
  ) => trade(asset, date, TransactionType.venda, quantity, gross);

  // PTAX do dólar a R$ 5 em 2024, R$ 6 em 2025 e R$ 4 em 2026, um ponto por
  // mês no dia 15 (mais de 10 dias depois do último ponto fica sem PTAX).
  PtaxSeries yearlyPtax() => PtaxSeries([
    (DateTime.utc(2023, 12, 15), const PtaxQuote(buy: 5.0, sell: 5.0)),
    for (final (year, rate) in [(2024, 5.0), (2025, 6.0), (2026, 4.0)])
      for (var m = 1; m <= 12; m++)
        (DateTime.utc(year, m, 15), PtaxQuote(buy: rate, sell: rate)),
  ]);

  FxHistory marketRates() => FxHistory([
    for (final (year, rate) in [(2024, 5.0), (2025, 6.0), (2026, 4.0)])
      for (var m = 1; m <= 12; m++) (DateTime.utc(year, m, 15), rate),
  ]);

  List<ForeignHolding> run(
    List<PortfolioTransaction> transactions, {
    List<Asset>? withAssets,
    Map<String, PtaxSeries>? ptax,
    Map<String, FxHistory>? fx,
    int through = 2026,
  }) => foreignHoldings(
    withAssets ?? assets,
    transactions,
    ptax: ptax ?? {'USD': yearlyPtax()},
    fx: fx ?? const {},
    throughYear: through,
  );

  ForeignHolding? at(List<ForeignHolding> all, int assetId, int year) {
    for (final h in all) {
      if (h.assetId == assetId && h.year == year) return h;
    }
    return null;
  }

  Matcher near(double expected) => closeTo(expected, 1e-6);

  test('a posição em cada 31/12 leva o custo médio em reais', () {
    // US$ 1.000 a R$ 5 em 2024 e US$ 1.200 a R$ 6 em 2025
    final all = run([
      buy(1, '2024-03-15', 10, 1000),
      buy(1, '2025-03-15', 10, 1200),
    ]);

    expect(at(all, 1, 2023), isNull);
    expect(at(all, 1, 2024)!.quantity, near(10));
    expect(at(all, 1, 2024)!.costBrl, near(5000));
    expect(at(all, 1, 2025)!.quantity, near(20));
    expect(at(all, 1, 2025)!.costBrl, near(12200));
    expect(at(all, 1, 2026)!.costBrl, near(12200));
    expect(all.any((h) => h.fxApproximate || h.incomplete), isFalse);
  });

  test('a taxa soma ao custo', () {
    final all = run([buy(1, '2024-03-15', 10, 1000, fee: 20)]);

    expect(at(all, 1, 2024)!.costBrl, near(5100));
  });

  test('a venda baixa pelo custo médio', () {
    final all = run([
      buy(1, '2024-03-15', 10, 1000),
      sell(1, '2025-03-15', 4, 600),
    ]);

    expect(at(all, 1, 2025)!.quantity, near(6));
    expect(at(all, 1, 2025)!.costBrl, near(3000));
  });

  test('ano sem posição não gera linha', () {
    final all = run([
      buy(1, '2024-03-15', 10, 1000),
      sell(1, '2025-03-15', 10, 1200),
    ]);

    expect(at(all, 1, 2024), isNotNull);
    expect(at(all, 1, 2025), isNull);
    expect(at(all, 1, 2026), isNull);
  });

  test('compra de 2023 forma o custo da foto de 31/12/2023', () {
    final all = run([buy(1, '2023-12-15', 10, 1000)]);

    expect(at(all, 1, 2023)!.costBrl, near(5000));
    expect(at(all, 1, 2024)!.costBrl, near(5000));
  });

  test('o câmbio de mercado de reserva marca aproximado, daí em diante', () {
    final all = run(
      [buy(1, '2024-03-15', 10, 1000)],
      ptax: const {},
      fx: {'USD': marketRates()},
    );

    expect(at(all, 1, 2024)!.fxApproximate, isTrue);
    expect(at(all, 1, 2025)!.fxApproximate, isTrue);
    expect(at(all, 1, 2024)!.incomplete, isFalse);
  });

  test('operação sem câmbio fica de fora e marca incompleto', () {
    final all = run(
      [buy(1, '2024-03-15', 10, 1000), buy(1, '2025-03-15', 10, 1200)],
      ptax: {
        'USD': PtaxSeries([
          (DateTime.utc(2024, 3, 15), const PtaxQuote(buy: 5.0, sell: 5.0)),
        ]),
      },
    );

    expect(at(all, 1, 2024)!.incomplete, isFalse);
    expect(at(all, 1, 2025)!.quantity, near(10));
    expect(at(all, 1, 2025)!.incomplete, isTrue);
  });

  test('venda maior que as compras lançadas marca incompleto', () {
    final all = run([
      buy(1, '2024-03-15', 10, 1000),
      sell(1, '2025-03-15', 15, 1800),
      buy(1, '2025-06-15', 5, 500),
    ]);

    expect(at(all, 1, 2025)!.incomplete, isTrue);
    expect(at(all, 1, 2025)!.quantity, near(5));
  });

  test('ativo em BRL vale 1 e a classe nacional é ignorada', () {
    final local = [
      stock(1, 'XYZ', currency: 'BRL'),
      stock(2, 'PETR4', assetClass: AssetClass.acaoBr, currency: 'BRL'),
    ];
    final all = run([
      buy(1, '2024-03-15', 10, 1000),
      buy(2, '2024-03-15', 10, 1000),
    ], withAssets: local);

    expect(at(all, 1, 2024)!.costBrl, near(1000));
    expect(all.any((h) => h.assetId == 2), isFalse);
  });

  test('vários ativos saem ordenados por ano e por ativo', () {
    final all = run([
      buy(2, '2024-03-15', 1, 100),
      buy(1, '2024-03-15', 1, 100),
    ], through: 2025);

    expect(all.map((h) => (h.year, h.assetId)).toList(), [
      (2024, 1),
      (2024, 2),
      (2025, 1),
      (2025, 2),
    ]);
  });
}
