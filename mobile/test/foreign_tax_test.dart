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

  // PTAX do dólar a R$ 5 em 2024, R$ 6 em 2025 e R$ 4 em 2026, com um ponto
  // por mês e dia 15 (um fato mais de 10 dias depois do último ponto, ou antes
  // do primeiro, fica sem PTAX; compra e venda iguais, salvo em `spread`).
  PtaxSeries yearlyPtax({double spread = 0}) => PtaxSeries([
    // dezembro de 2023, para as compras de antes da lei formarem custo
    (DateTime.utc(2023, 12, 15), PtaxQuote(buy: 5.0 - spread, sell: 5.0)),
    for (final (year, rate) in [(2024, 5.0), (2025, 6.0), (2026, 4.0)])
      for (var m = 1; m <= 12; m++)
        (
          DateTime.utc(year, m, 15),
          PtaxQuote(buy: rate - spread, sell: rate),
        ),
  ]);

  // Câmbio de mercado com os mesmos valores, para provar a reserva.
  FxHistory marketRates() => FxHistory([
    for (final (year, rate) in [(2024, 5.0), (2025, 6.0), (2026, 4.0)])
      for (var m = 1; m <= 12; m++) (DateTime.utc(year, m, 15), rate),
  ]);

  ForeignTaxAssessment run(
    List<PortfolioTransaction> transactions, {
    List<Asset>? withAssets,
    Map<String, PtaxSeries>? ptax,
    Map<String, FxHistory>? fx,
  }) => assessForeignTax(
    withAssets ?? assets,
    transactions,
    ptax: ptax ?? {'USD': yearlyPtax()},
    fx: fx ?? const {},
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
    final result = run(
      [dividend(1, '2027-05-15', 100)],
      ptax: {'USD': PtaxSeries([])},
    );

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

  test('ativo em BRL vale 1 e outras classes não entram', () {
    final reit = stock(3, 'O', assetClass: AssetClass.reit, currency: 'BRL');
    final brStock = stock(4, 'PETR4', assetClass: AssetClass.acaoBr, currency: 'BRL');
    final result = run(
      [
        buy(3, '2025-01-15', 1, 100),
        sell(3, '2025-06-15', 1, 150),
        buy(4, '2025-01-15', 1, 100),
        sell(4, '2025-06-15', 1, 150),
      ],
      withAssets: [reit, brStock],
    );

    expect(result.years.single.capitalGain, near(50));
    expect(result.years.single.sales.single.ticker, 'O');
    expect(result.warnings, isEmpty);
  });

  group('outras moedas', () {
    final sap = stock(5, 'SAP', currency: 'EUR');

    // PTAX do euro a R$ 7 em 2024 e R$ 8 em 2025.
    PtaxSeries euroPtax() => PtaxSeries([
      for (final (year, rate) in [(2024, 7.0), (2025, 8.0)])
        for (var m = 1; m <= 12; m++)
          (
            DateTime.utc(year, m, 15),
            PtaxQuote(buy: rate, sell: rate),
          ),
    ]);

    test('cada moeda converte pela própria série', () {
      // AAPL em USD (5 → 6) e SAP em EUR (7 → 8), cada uma com seu câmbio
      final result = run(
        [
          buy(1, '2024-03-15', 1, 100),
          sell(1, '2025-03-15', 1, 120),
          buy(5, '2024-03-15', 1, 100),
          sell(5, '2025-03-15', 1, 120),
        ],
        withAssets: [...assets, sap],
        ptax: {'USD': yearlyPtax(), 'EUR': euroPtax()},
      );
      final year = result.years.single;

      // USD: 120 × 6 − 100 × 5 = 220; EUR: 120 × 8 − 100 × 7 = 260
      expect(result.warnings, isEmpty);
      expect(year.capitalGain, near(480));
      expect(year.sales.map((s) => (s.ticker, s.currency, s.fxRate)), [
        ('AAPL', 'USD', 6.0),
        ('SAP', 'EUR', 8.0),
      ]);
    });

    test('moeda sem série fica de fora com aviso e as outras seguem', () {
      final result = run(
        [
          buy(1, '2024-03-15', 1, 100),
          sell(1, '2025-03-15', 1, 120),
          buy(5, '2025-01-15', 1, 100),
          sell(5, '2025-06-15', 1, 150),
        ],
        withAssets: [...assets, sap],
      );

      expect(result.years.single.sales.single.ticker, 'AAPL');
      expect(result.warnings.single.kind, ForeignTaxWarningKind.fxMissing);
      expect(result.warnings.single.year, 2025);
    });

    test('dividendo em euro soma à base e a retenção vira crédito', () {
      // € 100 a R$ 8 = 800, retido € 10 = 80; imposto 120, crédito 80
      final result = run(
        [
          tx(5, '2025-05-15', TransactionType.provento, 1, 100, withheld: 10),
        ],
        withAssets: [...assets, sap],
        ptax: {'EUR': euroPtax()},
      );
      final year = result.years.single;

      expect(year.dividends, near(800));
      expect(year.withheld, near(80));
      expect(year.taxBeforeCredit, near(120));
      expect(year.creditUsed, near(80));
      expect(year.taxDue, near(40));
    });
  });

  group('PTAX e câmbio de mercado', () {
    test('o imposto retido usa a compra e o resto usa a venda', () {
      // venda 6,00 e compra 5,90 em 2025: provento US$ 100 → 600; retido US$ 10 → 59
      final result = run(
        [dividend(1, '2025-05-15', 100, withheld: 10)],
        ptax: {'USD': yearlyPtax(spread: 0.1)},
      );
      final year = result.years.single;

      expect(year.dividends, near(600));
      expect(year.withheld, near(59));
      expect(year.dividendItems.single.fxRate, 6.0);
      expect(year.dividendItems.single.fxSource, RateSource.ptax);
      expect(result.warnings, isEmpty);
    });

    test('dia sem cotação vale a última PTAX antes dele, até 10 dias', () {
      // a série só tem o dia 15 de cada mês: 25/03 usa o dia 15, a 10 dias
      final year = run([
        buy(1, '2025-03-15', 1, 100),
        sell(1, '2025-03-25', 1, 120),
      ]).years.single;

      expect(year.sales.single.fxRate, 6.0);
      expect(year.sales.single.fxSource, RateSource.ptax);
      expect(year.capitalGain, near(120 * 6.0 - 100 * 6.0));
    });

    test('sem PTAX cai no câmbio de mercado e o ano avisa que é aproximado', () {
      final result = run(
        [
          buy(1, '2024-03-15', 10, 1000),
          sell(1, '2025-03-15', 10, 1200),
        ],
        ptax: {'USD': PtaxSeries([])},
        fx: {'USD': marketRates()},
      );
      final year = result.years.single;

      // o resultado é o mesmo da PTAX, mas marcado como de mercado
      expect(year.capitalGain, near(2200));
      expect(year.sales.single.fxSource, RateSource.market);
      final approximate = result.warnings
          .where((w) => w.kind == ForeignTaxWarningKind.fxApproximate)
          .map((w) => w.year)
          .toList();
      expect(approximate, [2024, 2025]);
    });

    test('um buraco longo na PTAX também cai no mercado só naquele fato', () {
      // PTAX só em março de 2025; a venda de 20/06 está a mais de 10 dias dela
      final onlyMarch = PtaxSeries([
        (DateTime.utc(2025, 3, 15), const PtaxQuote(buy: 6.0, sell: 6.0)),
      ]);
      final result = run(
        [
          buy(1, '2025-03-15', 1, 100),
          sell(1, '2025-06-20', 1, 120),
        ],
        ptax: {'USD': onlyMarch},
        fx: {'USD': marketRates()},
      );

      final sale = result.years.single.sales.single;
      expect(sale.fxSource, RateSource.market);
      expect(
        result.warnings.single.kind,
        ForeignTaxWarningKind.fxApproximate,
      );
    });

    test('a PTAX vence o mercado quando as duas existem', () {
      final differentMarket = FxHistory([
        for (var m = 1; m <= 12; m++) (DateTime.utc(2025, m, 15), 9.0),
      ]);
      final result = run(
        [dividend(1, '2025-05-15', 100)],
        fx: {'USD': differentMarket},
      );

      expect(result.years.single.dividends, near(600));
      expect(result.warnings, isEmpty);
    });

    test('ativo em BRL não passa por câmbio nem marca aproximação', () {
      final reit = stock(3, 'O', assetClass: AssetClass.reit, currency: 'BRL');
      final result = run(
        [buy(3, '2025-01-15', 1, 100), sell(3, '2025-06-15', 1, 150)],
        withAssets: [reit],
        ptax: const {},
      );

      expect(result.years.single.capitalGain, near(50));
      expect(result.warnings, isEmpty);
    });
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

  test('o informe de Bens e Direitos leva a ficha do ativo a cada posição', () {
    // AAPL com grupo, código e país informados; MSFT sem. Compra de 10 por US$ 1.000
    // em 15/03/2024 a R$ 5 (custo de R$ 5.000) nos dois.
    final withFicha = [
      Asset(
        id: 1,
        ticker: 'AAPL',
        name: 'AAPL',
        assetClass: AssetClass.acaoInternacional,
        currency: 'USD',
        createdAt: DateTime(2024, 1, 1),
        dirpfGroup: '03',
        dirpfCode: '01',
        dirpfCountry: '249',
      ),
      stock(2, 'MSFT'),
    ];
    final holdings = foreignHoldings(
      withFicha,
      [buy(1, '2024-03-15', 10, 1000), buy(2, '2024-03-15', 10, 1000)],
      ptax: {'USD': yearlyPtax()},
      throughYear: 2024,
    );

    final aapl = holdings.singleWhere((h) => h.ticker == 'AAPL');
    final msft = holdings.singleWhere((h) => h.ticker == 'MSFT');
    expect(aapl.costBrl, near(5000));
    expect((aapl.dirpfGroup, aapl.dirpfCode, aapl.dirpfCountry), ('03', '01', '249'));
    expect((msft.dirpfGroup, msft.dirpfCode, msft.dirpfCountry), (null, null, null));
  });
}
