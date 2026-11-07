import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/b3_tax.dart';
import 'package:anchor_mobile/models/asset.dart';
import 'package:anchor_mobile/models/portfolio_transaction.dart';

// Casos portados de `desktop/src-tauri/src/domain/tax/assessment.rs` (mesmos
// números), mais os de borda do limite de isenção. Dados fictícios.
void main() {
  Asset asset(int id, String ticker, AssetClass assetClass) => Asset(
    id: id,
    ticker: ticker,
    name: ticker,
    assetClass: assetClass,
    currency: 'BRL',
    createdAt: DateTime(2024, 1, 1),
  );

  const petr = 1;
  const bova = 2;
  const taee = 3;
  const hglg = 4;
  const btc = 5;
  final assets = [
    asset(petr, 'PETR4', AssetClass.acaoBr),
    asset(bova, 'BOVA11', AssetClass.etfBr),
    asset(taee, 'TAEE11', AssetClass.acaoBr),
    asset(hglg, 'HGLG11', AssetClass.fii),
    asset(btc, 'BTC', AssetClass.cripto),
  ];

  var nextId = 0;
  PortfolioTransaction trade(
    int assetId,
    String date,
    TransactionType type,
    double quantity,
    double gross, {
    double? fee,
    int? custodia,
  }) => PortfolioTransaction(
    id: ++nextId,
    assetId: assetId,
    type: type,
    quantity: quantity,
    unitPrice: gross / quantity,
    totalValue: gross,
    fee: fee,
    custodiaId: custodia,
    date: DateTime.parse(date),
    createdAt: DateTime(2024, 1, 1),
  );

  PortfolioTransaction buy(int a, String d, double q, double g, {double? fee, int? custodia}) =>
      trade(a, d, TransactionType.compra, q, g, fee: fee, custodia: custodia);
  PortfolioTransaction sell(int a, String d, double q, double g, {double? fee, int? custodia}) =>
      trade(a, d, TransactionType.venda, q, g, fee: fee, custodia: custodia);

  B3TaxAssessment run(List<PortfolioTransaction> transactions) =>
      assessB3Tax(assets, transactions);

  B3Month month(B3TaxAssessment result, String period) =>
      result.months.firstWhere((m) => m.period == period);

  B3BucketResult bucket(B3Month m, B3Bucket which) =>
      m.buckets.firstWhere((b) => b.bucket == which);

  Matcher near(double expected) => closeTo(expected, 1e-6);

  test('exemplo oficial da Receita: ganho, imposto, IRRF, DARF e vencimento', () {
    final result = run([
      buy(petr, '2024-01-10', 1000, 50000, fee: 15),
      sell(petr, '2024-02-15', 1000, 60000, fee: 18),
    ]);

    expect(result.months.map((m) => m.period), ['2024-02']);
    final feb = month(result, '2024-02');
    expect(feb.stockSalesTotal, near(60000));
    expect(feb.stockExempt, isFalse);
    expect(feb.sales.single.gain, near(9967));
    expect(feb.grossTax, near(1495.05));
    expect(feb.irrfWithheld, near(3.0));
    expect(feb.due, near(1492.05));
    expect(feb.darfCode, 6015);
    expect(feb.dueDate, DateTime.utc(2024, 3, 28));
  });

  test('ação vendida até R\$ 20 mil no mês: ganho isento, imposto zero', () {
    final result = run([
      buy(petr, '2024-01-10', 100, 10000),
      sell(petr, '2024-02-15', 100, 18000),
    ]);
    final feb = month(result, '2024-02');
    expect(feb.stockExempt, isTrue);
    expect(feb.exemptGain, near(8000));
    expect(feb.grossTax, near(0));
    expect(feb.due, near(0));
  });

  test('limite de isenção: exatamente R\$ 20 mil isenta, um centavo a mais tributa', () {
    final atLimit = run([
      buy(petr, '2024-01-10', 100, 10000),
      sell(petr, '2024-02-15', 100, 20000),
    ]);
    expect(month(atLimit, '2024-02').stockExempt, isTrue);
    expect(month(atLimit, '2024-02').grossTax, near(0));

    final overLimit = run([
      buy(petr, '2024-01-10', 100, 10000),
      sell(petr, '2024-02-15', 100, 20000.01),
    ]);
    expect(month(overLimit, '2024-02').stockExempt, isFalse);
    expect(month(overLimit, '2024-02').grossTax, near(10000.01 * 0.15));
  });

  test('ETF nunca é isento: 2.000 de ganho paga 300', () {
    final result = run([
      buy(bova, '2024-01-10', 100, 10000),
      sell(bova, '2024-02-15', 100, 12000),
    ]);
    final feb = month(result, '2024-02');
    expect(feb.stockSalesTotal, near(0));
    expect(feb.grossTax, near(300));
    expect(feb.irrfWithheld, near(0.6));
    expect(feb.due, near(299.4));
  });

  test('unit (ação terminada em 11) paga imposto e não entra no limite de ações', () {
    final result = run([
      buy(taee, '2024-01-10', 100, 4000),
      sell(taee, '2024-02-15', 100, 5000),
      buy(petr, '2024-01-10', 100, 10000),
      sell(petr, '2024-02-15', 100, 18000),
    ]);
    final feb = month(result, '2024-02');
    expect(feb.stockSalesTotal, near(18000));
    expect(feb.stockExempt, isTrue);
    expect(feb.exemptGain, near(8000));
    expect(feb.grossTax, near(150));
    expect(feb.sales.firstWhere((s) => s.ticker == 'TAEE11').isUnit, isTrue);
  });

  test('day trade: comprou e vendeu no mesmo dia na mesma custódia paga 20%', () {
    final result = run([
      buy(petr, '2024-02-15', 100, 3000, custodia: 1),
      sell(petr, '2024-02-15', 100, 3500, custodia: 1),
    ]);
    final feb = month(result, '2024-02');
    expect(bucket(feb, B3Bucket.dayTrade).tax, near(100));
    expect(feb.irrfWithheld, near(5));
    expect(feb.due, near(95));
    expect(feb.stockSalesTotal, near(0)); // day trade não conta para a isenção
  });

  test('day trade parcial: o resto da compra vai ao custo médio e a venda sobrante é swing', () {
    final result = run([
      buy(petr, '2024-03-01', 100, 1000),
      buy(petr, '2024-04-20', 50, 600),
      sell(petr, '2024-04-20', 80, 1200),
    ]);
    final apr = month(result, '2024-04');
    final dayTrade = apr.sales.singleWhere((s) => s.bucket == B3Bucket.dayTrade);
    final swing = apr.sales.singleWhere((s) => s.bucket == B3Bucket.common);
    expect(dayTrade.quantity, near(50));
    expect(dayTrade.gain, near(150));
    expect(swing.quantity, near(30));
    expect(swing.gain, near(150));
  });

  test('custódias diferentes no mesmo dia não são day trade', () {
    final result = run([
      buy(petr, '2024-02-15', 100, 3000, custodia: 1),
      sell(petr, '2024-02-15', 100, 3500, custodia: 2),
    ]);
    final feb = month(result, '2024-02');
    expect(feb.sales.single.bucket, B3Bucket.common);
    expect(feb.sales.single.gain, near(500));
  });

  test('FII tem gaveta e alíquota próprias: 100 de ganho paga 20', () {
    final result = run([
      buy(hglg, '2024-01-10', 10, 1500),
      sell(hglg, '2024-02-15', 10, 1600),
    ]);
    final feb = month(result, '2024-02');
    expect(bucket(feb, B3Bucket.fii).tax, near(20));
    expect(feb.sales.single.bucket, B3Bucket.fii);
  });

  test('prejuízo só compensa dentro da própria gaveta', () {
    final result = run([
      buy(bova, '2023-12-10', 100, 5000),
      sell(bova, '2024-01-15', 100, 4000), // -1.000 no ETF
      buy(bova, '2024-02-01', 100, 1000),
      sell(bova, '2024-02-15', 100, 2500), // +1.500 no ETF
      buy(hglg, '2024-02-01', 10, 1000),
      sell(hglg, '2024-02-15', 10, 2000), // +1.000 no FII
    ]);
    final feb = month(result, '2024-02');
    final common = bucket(feb, B3Bucket.common);
    expect(common.lossUsed, near(1000));
    expect(common.taxable, near(500));
    final fii = bucket(feb, B3Bucket.fii);
    expect(fii.lossUsed, near(0));
    expect(fii.taxable, near(1000));
    expect(feb.grossTax, near(500 * 0.15 + 1000 * 0.20));
  });

  test('ganho isento de ação não consome o prejuízo acumulado', () {
    final result = run([
      buy(bova, '2023-12-10', 100, 1000),
      sell(bova, '2024-01-15', 100, 500), // -500 na gaveta comum
      buy(petr, '2024-02-01', 100, 1000),
      sell(petr, '2024-02-15', 100, 3000), // +2.000, mês isento
    ]);
    final feb = month(result, '2024-02');
    expect(feb.exemptGain, near(2000));
    final common = bucket(feb, B3Bucket.common);
    expect(common.lossUsed, near(0));
    expect(common.lossCarried, near(500));
  });

  test('imposto abaixo de R\$ 10 não gera DARF e soma ao mês seguinte', () {
    final result = run([
      buy(bova, '2024-02-01', 100, 1000),
      sell(bova, '2024-02-10', 100, 1040), // imposto 6,00; IRRF 0,052
      buy(bova, '2024-03-01', 100, 1000),
      sell(bova, '2024-03-10', 100, 1040),
    ]);
    final feb = month(result, '2024-02');
    expect(feb.due, near(0));
    expect(feb.carriedOut, near(5.948));
    final mar = month(result, '2024-03');
    expect(mar.carriedIn, near(5.948));
    expect(mar.due, near(11.896));
    expect(mar.carriedOut, near(0));
  });

  test('posição zerada recomeça o custo médio do zero', () {
    final result = run([
      buy(bova, '2024-01-02', 10, 1000),
      sell(bova, '2024-01-10', 10, 1000),
      buy(bova, '2024-02-02', 10, 500),
      sell(bova, '2024-02-10', 10, 600),
    ]);
    final feb = month(result, '2024-02');
    expect(feb.sales.single.cost, near(500));
    expect(feb.sales.single.gain, near(100));
  });

  test('vender mais do que o histórico mostra marca custo desconhecido e avisa', () {
    final result = run([
      buy(petr, '2024-01-10', 10, 300),
      sell(petr, '2024-02-15', 20, 800),
    ]);
    final sale = month(result, '2024-02').sales.single;
    expect(sale.costUnknown, isTrue);
    expect(sale.quantity, near(10));
    expect(sale.gain, near(100));
    expect(result.warnings.single.kind, B3TaxWarningKind.costUnknown);
    expect(result.warnings.single.ticker, 'PETR4');
  });

  test('ignora cripto e transações que não são compra nem venda', () {
    final result = run([
      buy(btc, '2024-01-10', 1, 1000),
      sell(btc, '2024-02-15', 1, 2000),
      trade(petr, '2024-02-15', TransactionType.provento, 1, 50),
    ]);
    expect(result.months, isEmpty);
    expect(result.warnings, isEmpty);
  });

  test('a ordem em que as transações chegam não muda o resultado', () {
    final ordered = [
      buy(petr, '2024-01-10', 1000, 50000, fee: 15),
      sell(petr, '2024-02-15', 1000, 60000, fee: 18),
    ];
    final result = run(ordered.reversed.toList());
    expect(month(result, '2024-02').due, near(1492.05));
  });
}
