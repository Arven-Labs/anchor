import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/foreign_tax.dart';
import 'package:anchor_mobile/domain/fx_history.dart';
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
  PortfolioTransaction tx(
    int assetId,
    String date,
    TransactionType type,
    double quantity,
    double gross, {
    double? fee,
    double? withheld,
  }) => PortfolioTransaction(
    id: ++nextId,
    assetId: assetId,
    type: type,
    quantity: type == TransactionType.provento ? null : quantity,
    unitPrice: type == TransactionType.provento ? null : gross / quantity,
    totalValue: gross,
    fee: fee,
    foreignTaxWithheld: withheld,
    date: DateTime.parse(date),
    createdAt: DateTime(2024, 1, 1),
  );

  PortfolioTransaction buy(int asset, String date, double quantity, double gross, {double? fee}) =>
      tx(asset, date, TransactionType.compra, quantity, gross, fee: fee);
  PortfolioTransaction sell(int asset, String date, double quantity, double gross, {double? fee}) =>
      tx(asset, date, TransactionType.venda, quantity, gross, fee: fee);
  PortfolioTransaction dividend(int asset, String date, double gross, {double? withheld}) =>
      tx(asset, date, TransactionType.provento, 1, gross, withheld: withheld);

  // Dólar a R$ 5 em 2024, R$ 6 em 2025 e R$ 4 em 2026, com um ponto por mês
  // (o dia mais próximo vale, então a série precisa cobrir os dias testados;
  // 2023 e 2027 ficam sem cotação).
  FxHistory yearlyRates() => FxHistory([
    for (var m = 1; m <= 12; m++) (DateTime.utc(2024, m, 15), 5.0),
    for (var m = 1; m <= 12; m++) (DateTime.utc(2025, m, 15), 6.0),
    for (var m = 1; m <= 12; m++) (DateTime.utc(2026, m, 15), 4.0),
  ]);

  ForeignTaxAssessment run(
    List<PortfolioTransaction> transactions, {
    List<Asset>? withAssets,
    FxHistory? usdBrl,
  }) => assessForeignTax(
    withAssets ?? assets,
    transactions,
    usdBrl: usdBrl ?? yearlyRates(),
  );

  Matcher near(double expected) => closeTo(expected, 1e-6);

  test('o custo converte no dia da compra e a venda no dia dela', () {
    // compra US$ 1.000 a R$ 5 (custo 5.000), venda US$ 1.200 a R$ 6 (7.200)
    final result = run([
      buy(1, '2024-03-15', 10, 1000),
      sell(1, '2025-03-15', 10, 1200),
    ]);
    final year = result.years.single;

    expect(result.warnings, isEmpty);
    expect(year.year, 2025);
    expect(year.capitalGain, near(2200));
    expect(year.taxableBase, near(2200));
    expect(year.taxDue, near(330));
    expect(year.rate, 0.15);
    expect(year.sales.single.saleValue, near(7200));
    expect(year.sales.single.cost, near(5000));
    expect(year.sales.single.fxRate, 6);
  });

  test('a taxa soma ao custo e reduz o ganho, sem mexer no valor bruto', () {
    // custo (1.000 + 20) × 5 = 5.100; recebe (1.200 − 30) × 6 = 7.020
    final year = run([
      buy(1, '2024-03-15', 10, 1000, fee: 20),
      sell(1, '2025-03-15', 10, 1200, fee: 30),
    ]).years.single;

    expect(year.capitalGain, near(1920));
    expect(year.sales.single.saleValue, near(7200));
  });

  test('ganhos e perdas de ativos diferentes no ano se compensam', () {
    final year = run([
      buy(1, '2025-01-15', 1, 100),
      buy(2, '2025-01-15', 1, 500),
      sell(1, '2025-06-15', 1, 200),
      sell(2, '2025-06-15', 1, 300),
    ]).years.single;

    // ganho 100 × 6 = 600; perda −200 × 6 = −1.200
    expect(year.capitalGain, near(-600));
    expect(year.taxableBase, 0);
    expect(year.taxDue, 0);
    expect(year.lossCarriedOut, near(600));
  });

  test('o prejuízo passa para os anos seguintes e é consumido', () {
    final years = run([
      buy(1, '2024-01-15', 2, 1000),
      sell(1, '2024-06-15', 1, 400),
      sell(1, '2025-06-15', 1, 700),
    ]).years;

    // 2024: custo 500 × 5 = 2.500, venda 400 × 5 = 2.000 → −500
    // 2025: 700 × 6 = 4.200 − 2.500 = 1.700, abate 500
    expect(years.map((y) => y.year), [2024, 2025]);
    expect(years[0].lossCarriedOut, near(500));
    expect(years[0].taxDue, 0);
    expect(years[1].lossCarriedIn, near(500));
    expect(years[1].taxableBase, near(1200));
    expect(years[1].taxDue, near(180));
    expect(years[1].lossCarriedOut, 0);
  });

  test('provento soma à base e a retenção no exterior é crédito até o imposto', () {
    // US$ 100 a R$ 6 = 600; retido US$ 30 = 180; imposto 90, crédito 90
    final year = run([
      dividend(1, '2025-05-15', 100, withheld: 30),
    ]).years.single;

    expect(year.dividends, near(600));
    expect(year.withheld, near(180));
    expect(year.taxableBase, near(600));
    expect(year.taxBeforeCredit, near(90));
    expect(year.creditUsed, near(90));
    expect(year.taxDue, 0);
  });

  test('uma retenção pequena só abate o imposto', () {
    final year = run([
      dividend(1, '2025-05-15', 100, withheld: 5),
    ]).years.single;

    expect(year.creditUsed, near(30));
    expect(year.taxDue, near(60));
  });

  test('sem câmbio na data o item fica de fora e o ano avisa', () {
    final result = run([dividend(1, '2027-05-15', 100)], usdBrl: FxHistory([]));

    expect(result.years, isEmpty);
    expect(result.warnings.single.kind, ForeignTaxWarningKind.fxMissing);
    expect(result.warnings.single.year, 2027);
  });

  test('sem série de câmbio nenhuma operação em USD é apurada', () {
    final result = assessForeignTax(assets, [
      buy(1, '2025-01-15', 1, 100),
      sell(1, '2025-06-15', 1, 200),
    ]);

    expect(result.years, isEmpty);
    expect(result.warnings.single.kind, ForeignTaxWarningKind.fxMissing);
  });

  test('venda de antes de 2024 não é apurada, mas a compra forma o custo', () {
    // compra em 2023 a R$ 5 (série cobre só 2024+, o dia mais próximo vale)
    final result = run([
      buy(1, '2023-12-20', 2, 1000),
      sell(1, '2024-06-15', 1, 400),
    ]);
    final year = result.years.single;

    // custo 500 × 5 = 2.500; venda 400 × 5 = 2.000
    expect(year.capitalGain, near(-500));
    expect(result.warnings, isEmpty);

    final before = run([
      buy(1, '2023-06-15', 2, 1000),
      sell(1, '2023-12-20', 1, 400),
    ]);
    expect(before.years, isEmpty, reason: 'ano anterior a 2024 não entra');
  });

  test('vender acima do custo conhecido avisa e só conta a parte coberta', () {
    final result = run([
      buy(1, '2024-01-15', 1, 100),
      sell(1, '2025-06-15', 2, 400),
    ]);
    final sale = result.years.single.sales.single;

    expect(result.warnings.single.kind, ForeignTaxWarningKind.costUnknown);
    expect(sale.costUnknown, isTrue);
    expect(sale.quantity, near(1));
    // metade vendida com custo: valor 400 × 6 × ½ = 1.200, custo 500, ganho 700
    expect(sale.gain, near(700));
  });

  test('ativo em BRL vale 1 e outras classes e moedas não entram', () {
    final reit = stock(3, 'O', assetClass: AssetClass.reit, currency: 'BRL');
    final brStock = stock(4, 'PETR4', assetClass: AssetClass.acaoBr, currency: 'BRL');
    final eur = stock(5, 'SAP', currency: 'EUR');
    final result = run(
      [
        buy(3, '2025-01-15', 1, 100),
        sell(3, '2025-06-15', 1, 150),
        buy(4, '2025-01-15', 1, 100),
        sell(4, '2025-06-15', 1, 150),
        buy(5, '2025-01-15', 1, 100),
        sell(5, '2025-06-15', 1, 150),
      ],
      withAssets: [reit, brStock, eur],
    );

    // só o REIT em BRL (câmbio 1) é apurado; o EUR sai com aviso de câmbio
    expect(result.years.single.capitalGain, near(50));
    expect(result.years.single.sales.single.ticker, 'O');
    expect(result.warnings.single.kind, ForeignTaxWarningKind.fxMissing);
    expect(result.warnings.single.year, 2025);
  });

  test('transferência, aporte e retirada não mexem na apuração', () {
    final result = run([
      PortfolioTransaction(
        id: 900,
        assetId: 1,
        type: TransactionType.transferencia,
        quantity: 5,
        totalValue: 0,
        date: DateTime(2025, 1, 15),
        createdAt: DateTime(2025, 1, 15),
      ),
    ]);

    expect(result.years, isEmpty);
    expect(result.warnings, isEmpty);
  });
}
