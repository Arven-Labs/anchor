import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/home_summary.dart';
import 'package:anchor_mobile/models/asset.dart';
import 'package:anchor_mobile/models/position.dart';
import 'package:anchor_mobile/services/live_positions.dart';

void main() {
  LivePositionRow row(
    String ticker, {
    AssetClass assetClass = AssetClass.acaoBr,
    String currency = 'BRL',
    required double quantity,
    double? avg,
    double? price,
    double? manualTotal,
  }) => LivePositionRow(
    Position(
      asset: Asset(
        id: 1,
        ticker: ticker,
        name: ticker,
        assetClass: assetClass,
        currency: currency,
        createdAt: DateTime(2026, 1, 1),
      ),
      netQuantity: quantity,
      averageBuyPrice: avg,
    ),
    price,
    manualTotalValue: manualTotal,
  );

  HomeSummary summary(
    List<LivePositionRow> rows, {
    String base = 'BRL',
    Map<String, double> rates = const {},
  }) => computeHomeSummary(rows, base, rates);

  test('uma ação: valor, custo, lucro e percentual', () {
    final s = summary([row('PETR4', quantity: 10, avg: 20, price: 25)]);

    expect(s.totalValue, 250);
    expect(s.costBasis, 200);
    expect(s.profit, 50);
    expect(s.profitPercent, 25);
    expect(s.itemsMissing, 0);
    expect(s.profitExcluded, 0);
  });

  test('sem posição nenhuma: tudo zerado e lucro nulo (não zero)', () {
    final s = summary([]);

    expect(s.totalValue, 0);
    expect(s.profit, isNull);
    expect(s.profitPercent, isNull);
    expect(s.allocation, isEmpty);
  });

  test('BRL e USD são convertidos para a moeda base antes de somar', () {
    final s = summary(
      [
        row('PETR4', quantity: 10, avg: 20, price: 25),
        // 2 × US$ 12 = US$ 24 = R$ 120; custo 2 × 10 = US$ 20 = R$ 100
        row('AAPL', currency: 'USD', quantity: 2, avg: 10, price: 12),
      ],
      rates: {'USD': 5.0},
    );

    expect(s.totalValue, 370);
    expect(s.costBasis, 300);
    expect(s.profit, 70);
  });

  test('base em dólar divide o real pela taxa', () {
    final s = summary(
      [row('PETR4', quantity: 10, avg: 20, price: 25)],
      base: 'USD',
      rates: {'USD': 5.0},
    );

    expect(s.totalValue, 50);
    expect(s.profit, 10);
  });

  test('posição zerada (já vendida) não conta', () {
    final s = summary([
      row('VALE3', quantity: 0, avg: 50, price: 60),
      row('PETR4', quantity: 10, avg: 20, price: 25),
    ]);

    expect(s.totalValue, 250);
    expect(s.itemsMissing, 0);
    expect(s.allocation, hasLength(1));
  });

  test('posição sem cotação fica fora do valor e do lucro, sem virar prejuízo', () {
    final s = summary([
      row('PETR4', quantity: 10, avg: 20, price: 25),
      // sem cotação agora: somar o custo dela sem o valor mostraria um prejuízo falso
      row('VALE3', quantity: 100, avg: 50, price: null),
    ]);

    expect(s.totalValue, 250);
    expect(s.costBasis, 200);
    expect(s.profit, 50);
    expect(s.itemsMissing, 1);
    expect(s.profitExcluded, 1);
  });

  test('moeda sem câmbio: o item fica de fora dos dois lados e é contado', () {
    final s = summary(
      [
        row('PETR4', quantity: 10, avg: 20, price: 25),
        row('AAPL', currency: 'USD', quantity: 2, avg: 10, price: 12),
      ],
      rates: const {},
    );

    expect(s.totalValue, 250);
    expect(s.profit, 50);
    expect(s.itemsMissing, 1);
    expect(s.profitExcluded, 1);
  });

  test('sem preço médio (nunca houve compra): entra no valor, não no lucro', () {
    final s = summary([
      row('PETR4', quantity: 10, avg: null, price: 25),
      row('VALE3', quantity: 10, avg: 20, price: 25),
    ]);

    expect(s.totalValue, 500);
    expect(s.costBasis, 200);
    expect(s.profit, 50);
    expect(s.profitExcluded, 1);
  });

  test('ativo manual (empresa não listada) entra no valor e na alocação, não no lucro', () {
    final s = summary([
      row('PETR4', quantity: 10, avg: 20, price: 25),
      row(
        'MINHAEMPRESA',
        assetClass: AssetClass.empresaNaoListada,
        quantity: 1,
        manualTotal: 1000,
      ),
    ]);

    expect(s.totalValue, 1250);
    expect(s.profit, 50, reason: 'ativo manual não tem custo comparável');
    expect(s.profitExcluded, 0, reason: 'excluído por desenho, não por falta de dado');
    expect(s.allocation.map((a) => a.assetClass), [
      AssetClass.empresaNaoListada,
      AssetClass.acaoBr,
    ]);
  });

  test('custo zero: lucro existe, percentual não (sem divisão por zero)', () {
    final s = summary([row('BONUS', quantity: 10, avg: 0, price: 5)]);

    expect(s.profit, 50);
    expect(s.profitPercent, isNull);
  });

  test('alocação junta a mesma classe, ordena do maior para o menor e soma 100%', () {
    final s = summary([
      row('PETR4', quantity: 10, avg: 20, price: 25), // 250
      row('VALE3', quantity: 10, avg: 20, price: 15), // 150
      row('HGLG11', assetClass: AssetClass.fii, quantity: 10, avg: 100, price: 60), // 600
    ]);

    expect(s.allocation.map((a) => a.assetClass), [
      AssetClass.fii,
      AssetClass.acaoBr,
    ]);
    expect(s.allocation[0].value, 600);
    expect(s.allocation[1].value, 400);
    expect(s.allocation[0].share, closeTo(0.6, 1e-9));
    expect(
      s.allocation.fold<double>(0, (sum, a) => sum + a.share),
      closeTo(1.0, 1e-9),
    );
  });

  test('a alocação usa o valor convertido para a base', () {
    final s = summary(
      [
        row('PETR4', quantity: 10, avg: 20, price: 25), // R$ 250
        row(
          'AAPL',
          assetClass: AssetClass.acaoInternacional,
          currency: 'USD',
          quantity: 10,
          avg: 10,
          price: 10, // US$ 100 = R$ 500
        ),
      ],
      rates: {'USD': 5.0},
    );

    expect(s.allocation.first.assetClass, AssetClass.acaoInternacional);
    expect(s.allocation.first.value, 500);
  });
}
