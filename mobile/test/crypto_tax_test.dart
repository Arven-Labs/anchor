import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/crypto_tax.dart';
import 'package:anchor_mobile/domain/fx_history.dart';
import 'package:anchor_mobile/models/asset.dart';
import 'package:anchor_mobile/models/portfolio_transaction.dart';

void main() {
  Asset coin(
    int id,
    String ticker, {
    String currency = 'BRL',
    AssetClass assetClass = AssetClass.cripto,
  }) => Asset(
    id: id,
    ticker: ticker,
    name: ticker,
    assetClass: assetClass,
    currency: currency,
    createdAt: DateTime(2026, 1, 1),
  );

  final assets = [coin(1, 'BTC'), coin(2, 'ETH')];

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
    createdAt: DateTime(2026, 1, 1),
  );

  PortfolioTransaction buy(int asset, String date, double quantity, double gross, {double? fee}) =>
      trade(asset, date, TransactionType.compra, quantity, gross, fee: fee);
  PortfolioTransaction sell(int asset, String date, double quantity, double gross, {double? fee}) =>
      trade(asset, date, TransactionType.venda, quantity, gross, fee: fee);

  CryptoTaxAssessment run(
    List<PortfolioTransaction> transactions, {
    List<Asset>? withAssets,
    FxHistory? usdBrl,
  }) => assessCryptoTax(withAssets ?? assets, transactions, usdBrl: usdBrl);

  Matcher near(double expected) => closeTo(expected, 1e-6);

  test('vendas do mês até o limite são isentas mesmo com ganho', () {
    final result = run([
      buy(1, '2026-01-02', 1, 10000),
      sell(1, '2026-02-10', 1, 35000),
    ]);
    final month = result.months.single;

    expect(month.exempt, isTrue);
    expect(month.tax, 0);
    expect(month.due, 0);
    expect(month.gain, near(25000));
    expect(month.salesTotal, near(35000), reason: 'o limite exato é isento');
    expect(result.warnings, isEmpty);
  });

  test('acima do limite o ganho do mês inteiro é tributado a 15%', () {
    final feb = run([
      buy(1, '2026-01-02', 1, 10000),
      sell(1, '2026-02-10', 1, 40000),
    ]).months.single;

    expect(feb.exempt, isFalse);
    expect(feb.gain, near(30000));
    expect(feb.tax, near(4500));
    expect(feb.due, near(4500));
    expect(feb.darfCode, 4600);
    expect(feb.dueDate, DateTime.utc(2026, 3, 31));
  });

  test('o limite soma todas as moedas vendidas no mês', () {
    final month = run([
      buy(1, '2026-01-02', 1, 10000),
      buy(2, '2026-01-02', 10, 10000),
      sell(1, '2026-02-10', 1, 20000),
      sell(2, '2026-02-12', 10, 20000),
    ]).months.single;

    expect(month.salesTotal, near(40000));
    expect(month.exempt, isFalse);
    expect(month.tax, near(0.15 * 20000));
  });

  test('mês de prejuízo não deve nada e a perda não compensa nem carrega', () {
    final months = run([
      buy(1, '2026-01-02', 2, 100000),
      sell(1, '2026-02-10', 1, 40000),
      sell(1, '2026-03-10', 1, 60000),
    ]).months;

    // Fev: vende 1 por 40 mil com custo 50 mil → perda de 10 mil. Mar: ganho de 10 mil, tributado inteiro.
    expect(months[0].gain, near(-10000));
    expect(months[0].tax, 0);
    expect(months[1].gain, near(10000));
    expect(months[1].tax, near(1500));
  });

  test('o custo é o médio por ativo', () {
    final month = run([
      buy(1, '2026-01-02', 1, 10000),
      buy(1, '2026-01-20', 1, 30000),
      sell(1, '2026-02-10', 1, 50000),
    ]).months.single;

    expect(month.sales.single.cost, near(20000));
    expect(month.gain, near(30000));
  });

  test('o custo médio corrente baixa a cada venda', () {
    // Compra 2 por 100 mil (médio 50 mil), vende 1 (custo 50 mil), compra 1 por 80 mil:
    // sobra 2 com custo 130 mil (médio 65 mil), então a 2ª venda custa 65 mil.
    final months = run([
      buy(1, '2026-01-02', 2, 100000),
      sell(1, '2026-02-10', 1, 60000),
      buy(1, '2026-02-20', 1, 80000),
      sell(1, '2026-03-10', 1, 100000),
    ]).months;

    expect(months[0].sales.single.cost, near(50000));
    expect(months[1].sales.single.cost, near(65000));
  });

  test('imposto abaixo do mínimo do DARF acumula para o mês seguinte', () {
    // Fev: ganho de 40 → imposto de 6 (abaixo de R$ 10); Mar: mais 6 → DARF de 12.
    final months = run([
      buy(1, '2026-01-02', 2, 80000),
      sell(1, '2026-02-10', 1, 40040),
      sell(1, '2026-03-10', 1, 40040),
    ]).months;

    expect(months[0].gain, near(40));
    expect(months[0].due, 0);
    expect(months[0].carriedOut, near(6));
    expect(months[1].due, near(12));
    expect(months[1].carriedOut, 0);
  });

  test('venda de ativo fora de BRL só avisa, e venda acima do custo conhecido é sinalizada', () {
    final foreign = run(
      [buy(1, '2026-01-02', 1, 10000), sell(1, '2026-02-10', 1, 50000)],
      withAssets: [coin(1, 'BTC', currency: 'EUR')],
    );
    expect(foreign.months, isEmpty);
    expect(
      foreign.warnings.single.kind,
      CryptoTaxWarningKind.salesNonBrl,
    );
    expect(foreign.warnings.single.period, '2026-02');

    final unknown = run([
      buy(1, '2026-01-02', 1, 10000),
      sell(1, '2026-02-10', 2, 80000),
    ]);
    final sale = unknown.months.single.sales.single;
    expect(sale.costUnknown, isTrue);
    expect(sale.quantity, near(1));
    expect(sale.saleValue, near(40000), reason: 'só a parte com custo conhecido');
    expect(unknown.warnings.single.kind, CryptoTaxWarningKind.costUnknown);
    expect(unknown.warnings.single.ticker, 'BTC');
  });

  test('ganho que cruza a 1ª faixa é tributado por faixa', () {
    // Ganho de 5,5 mi: 5 mi a 15% (750 mil) e 500 mil a 17,5% (87,5 mil).
    final month = run([
      buy(1, '2026-01-02', 1, 1000000),
      sell(1, '2026-02-10', 1, 6500000),
    ]).months.single;

    expect(month.tax, near(837500));
    expect(month.rate, near(837500 / 5500000));
  });

  test('as faixas seguem o ganho acumulado no ano e recomeçam a cada ano', () {
    final months = run([
      buy(1, '2026-01-02', 1, 1000000),
      sell(1, '2026-02-10', 0.5, 6000000),
      sell(1, '2026-03-10', 0.5, 6000000),
      buy(1, '2027-01-05', 1, 1000000),
      sell(1, '2027-02-10', 1, 2000000),
    ]).months;

    // Fev: ganho de 5,5 mi (5 mi a 15%, 500 mil a 17,5%). Mar: ganho de 5,5 mi já dentro da 2ª faixa
    // (até 10 mi acumulados) e 1 mi na 3ª.
    expect(months[0].tax, near(837500));
    expect(months[1].tax, near(4500000 * 0.175 + 1000000 * 0.20));
    // Em 2027 o acumulado recomeça: ganho de 1 mi, só 15%.
    expect(months[2].tax, near(150000));
    expect(months[2].rate, near(0.15));
  });

  test('acima de 30 milhões a alíquota máxima vale para o excedente', () {
    final month = run([
      buy(1, '2026-01-02', 1, 1000000),
      sell(1, '2026-02-10', 1, 36000000),
    ]).months.single;

    // Ganho de 35 mi: 5 mi a 15%, 5 mi a 17,5%, 20 mi a 20% e 5 mi a 22,5%.
    expect(month.tax, near(5e6 * 0.15 + 5e6 * 0.175 + 20e6 * 0.20 + 5e6 * 0.225));
  });

  test('outras classes, transferências e proventos não entram na apuração', () {
    final stock = coin(3, 'PETR4', assetClass: AssetClass.acaoBr);
    final result = assessCryptoTax(
      [...assets, stock],
      [
        buy(3, '2026-01-02', 100, 3000),
        sell(3, '2026-02-10', 100, 99000),
        trade(1, '2026-01-02', TransactionType.provento, 1, 5),
        trade(1, '2026-01-03', TransactionType.transferencia, 1, 5),
      ],
    );

    expect(result.months, isEmpty);
    expect(result.warnings, isEmpty);
  });

  test('a taxa da compra sobe o custo e a da venda baixa o ganho, sem mexer no limite de isenção', () {
    final month = run([
      buy(1, '2026-01-02', 1, 10000, fee: 100),
      sell(1, '2026-02-10', 1, 40000, fee: 400),
    ]).months.single;
    final sale = month.sales.single;

    expect(sale.cost, near(10100));
    expect(sale.saleValue, near(40000), reason: 'o limite conta o valor bruto');
    expect(sale.proceeds, near(39600));
    expect(sale.gain, near(29500));
    expect(month.salesTotal, near(40000));
    expect(month.tax, near(0.15 * 29500));
  });

  test('a taxa pode transformar a venda em prejuízo e a taxa sozinha não acende o limite', () {
    final month = run([
      buy(1, '2026-01-02', 1, 10000),
      sell(1, '2026-02-10', 1, 10100, fee: 200),
    ]).months.single;

    expect(month.gain, near(-100));
    expect(month.tax, 0);
    // Vendas de 34.800 + 200 de taxa não passam o limite por causa da taxa: conta o bruto.
    final edge = run([
      buy(1, '2026-01-02', 1, 10000),
      sell(1, '2026-02-10', 1, 35000, fee: 500),
    ]).months.single;
    expect(edge.exempt, isTrue);
  });

  test('venda acima do custo conhecido leva só a parte proporcional da taxa', () {
    // Compra 1, vende 2 por 80 mil com taxa de 800: cobre metade → valor 40 mil, líquido (80 mil − 800) / 2.
    final sale = run([
      buy(1, '2026-01-02', 1, 10000),
      sell(1, '2026-02-10', 2, 80000, fee: 800),
    ]).months.single.sales.single;

    expect(sale.saleValue, near(40000));
    expect(sale.proceeds, near(39600));
    expect(sale.gain, near(29600));
  });

  group('cripto em USD', () {
    final usdBtc = [coin(1, 'BTC', currency: 'USD')];
    // Dólar a R$ 5 em janeiro e a R$ 6 em fevereiro.
    FxHistory dollar() => FxHistory([
      (DateTime.utc(2026, 1, 2), 5.0),
      (DateTime.utc(2026, 2, 10), 6.0),
    ]);

    test('é convertida pelo dólar da data de cada operação (caso da Sessão 171 do desktop)', () {
      // Compra de US$ 10 mil a R$ 5 (custo R$ 50 mil) e venda de US$ 12 mil a R$ 6 (R$ 72 mil):
      // ganho de R$ 22 mil, acima do limite, 15% = R$ 3.300.
      final result = run(
        [buy(1, '2026-01-02', 1, 10000), sell(1, '2026-02-10', 1, 12000)],
        withAssets: usdBtc,
        usdBrl: dollar(),
      );
      final month = result.months.single;

      expect(result.usdConverted, isTrue);
      expect(month.salesTotal, near(72000));
      expect(month.gain, near(22000));
      expect(month.tax, near(3300));
      expect(result.warnings, isEmpty);
    });

    test('a taxa também é convertida', () {
      final month = run(
        [
          buy(1, '2026-01-02', 1, 10000, fee: 20),
          sell(1, '2026-02-10', 1, 12000, fee: 50),
        ],
        withAssets: usdBtc,
        usdBrl: dollar(),
      ).months.single;

      // Custo (10.000 + 20) × 5 = 50.100; líquido (12.000 − 50) × 6 = 71.700.
      expect(month.sales.single.cost, near(50100));
      expect(month.sales.single.proceeds, near(71700));
      expect(month.gain, near(21600));
    });

    test('sem câmbio, a operação em USD fica de fora e a venda avisa', () {
      for (final fx in [null, FxHistory(const [])]) {
        final result = run(
          [buy(1, '2026-01-02', 1, 10000), sell(1, '2026-02-10', 1, 12000)],
          withAssets: usdBtc,
          usdBrl: fx,
        );

        expect(result.months, isEmpty);
        expect(result.usdConverted, isFalse);
        expect(result.warnings.single.kind, CryptoTaxWarningKind.fxMissing);
        expect(result.warnings.single.period, '2026-02');
      }
    });

    test('a venda em USD soma no limite do mês com a venda em reais', () {
      // BTC em USD vende R$ 24 mil (US$ 4 mil a R$ 6) e ETH em reais vende R$ 20 mil: R$ 44 mil, acima do limite.
      final month = run(
        [
          buy(1, '2026-01-02', 1, 1000),
          buy(2, '2026-01-02', 1, 10000),
          sell(1, '2026-02-10', 1, 4000),
          sell(2, '2026-02-10', 1, 20000),
        ],
        withAssets: [coin(1, 'BTC', currency: 'USD'), coin(2, 'ETH')],
        usdBrl: dollar(),
      ).months.single;

      expect(month.salesTotal, near(44000));
      expect(month.exempt, isFalse);
      // BTC: 24.000 − 5.000 = 19.000; ETH: 20.000 − 10.000 = 10.000.
      expect(month.gain, near(29000));
    });

    test('usdConverted é falso quando não há operação em USD', () {
      final result = run([
        buy(1, '2026-01-02', 1, 10000),
        sell(1, '2026-02-10', 1, 40000),
      ], usdBrl: dollar());

      expect(result.usdConverted, isFalse);
    });
  });

  test('no mesmo dia a compra vem antes da venda, qualquer que seja a ordem da lista', () {
    final months = run([
      sell(1, '2026-02-10', 1, 40000),
      buy(1, '2026-02-10', 1, 10000),
    ]).months;

    expect(months.single.sales.single.costUnknown, isFalse);
    expect(months.single.gain, near(30000));
  });
}
