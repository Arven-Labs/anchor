import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/come_cotas.dart';
import 'package:anchor_mobile/domain/fund_tax.dart';
import 'package:anchor_mobile/models/asset.dart';
import 'package:anchor_mobile/models/fund.dart';
import 'package:anchor_mobile/models/portfolio_transaction.dart';
import 'package:anchor_mobile/screens/fund_tax_screen.dart';
import 'package:anchor_mobile/services/portfolio_repository.dart';

void main() {
  DateTime d(String s) => DateTime.parse('${s}T00:00:00Z');

  final fund = Asset(
    id: 1,
    ticker: 'FUNDO',
    name: 'Fundo Teste',
    assetClass: AssetClass.fundo,
    currency: 'BRL',
    createdAt: DateTime(2026, 1, 1),
  );

  PortfolioTransaction tx(
    int id,
    TransactionType type,
    String date,
    double quantity,
    double unitPrice, {
    double? fee,
  }) => PortfolioTransaction(
    id: id,
    assetId: 1,
    type: type,
    quantity: quantity,
    unitPrice: unitPrice,
    totalValue: quantity * unitPrice,
    fee: fee,
    date: d(date),
    createdAt: d(date),
  );

  FundQuota quota(String date, double value) =>
      FundQuota(assetId: 1, date: d(date), value: value, createdAt: d(date));

  List<FundReport> reports(
    List<PortfolioTransaction> transactions, {
    FundTerm? term = FundTerm.longTerm,
    List<FundQuota> quotas = const [],
    String today = '2026-01-10',
  }) => buildFundReports(
    assets: [fund],
    transactions: transactions,
    terms: {1: ?term},
    quotas: quotas,
    today: d(today),
  );

  test('só ativos da classe fundo entram no relatório', () {
    final stock = Asset(
      id: 2,
      ticker: 'PETR4',
      name: 'Petrobras',
      assetClass: AssetClass.acaoBr,
      currency: 'BRL',
      createdAt: DateTime(2026, 1, 1),
    );
    final result = buildFundReports(
      assets: [stock, fund],
      transactions: const [],
      terms: const {},
      quotas: const [],
      today: d('2026-01-10'),
    );
    expect(result.map((r) => r.asset.ticker), ['FUNDO']);
  });

  test('o come-cotas registrado tira cotas e vira a base do ganho', () {
    // último come-cotas antes de 10/01/2026: 28/11/2025; 2,5 cotas a 12,00 = imposto 30
    final report = reports(
      [
        tx(1, TransactionType.compra, '2025-01-02', 100, 10),
        tx(2, TransactionType.comeCotas, '2025-11-28', 2.5, 12),
      ],
      quotas: [quota('2026-01-05', 13)],
    ).single;
    expect(report.quantity, closeTo(97.5, 1e-9));
    expect(report.deductions, hasLength(1));
    expect(report.estimate.basisSource, BasisSource.lastComeCotas);
    expect(report.estimate.basisPerQuota, closeTo(12, 1e-9));
    // ganho = 97,5 × (13 − 12) = 97,5; imposto 15% = 14,625
    expect(report.estimate.tax, closeTo(14.625, 1e-9));
    expect(report.estimate.lastReferenceUnrecorded, isNull);
  });

  test('o come-cotas não muda o custo médio das cotas que ficam', () {
    // desconto a 9,00 (abaixo do custo 10,00): a base continua sendo o custo médio
    final report = reports(
      [
        tx(1, TransactionType.compra, '2025-01-02', 100, 10),
        tx(2, TransactionType.comeCotas, '2025-11-28', 2.5, 9),
      ],
      quotas: [quota('2026-01-05', 13)],
    ).single;
    expect(report.estimate.basisSource, BasisSource.cost);
    expect(report.estimate.basisPerQuota, closeTo(10, 1e-9));
  });

  test('a taxa da compra entra no custo médio', () {
    final report = reports(
      [tx(1, TransactionType.compra, '2025-01-02', 100, 10, fee: 100)],
      quotas: [quota('2026-01-05', 12)],
    ).single;
    // custo 1.100 / 100 cotas = 11,00 → ganho 100 × 1,00 = 100, imposto 15
    expect(report.estimate.basisPerQuota, closeTo(11, 1e-9));
    expect(report.estimate.tax, closeTo(15, 1e-9));
  });

  test('a venda vira resgate estimado e sem tipo não há estimativa', () {
    final txs = [
      tx(1, TransactionType.compra, '2025-01-01', 100, 10),
      tx(2, TransactionType.venda, '2026-03-01', 100, 12),
    ];
    final typed = reports(txs).single;
    expect(typed.redemptions, hasLength(1));
    expect(typed.redemptions.single.ir, closeTo(35, 1e-6));
    expect(typed.quantity, 0);

    final untyped = reports(txs, term: null).single;
    expect(untyped.redemptions, isEmpty);
    expect(untyped.estimate.status, EstimateStatus.noPosition);
  });

  test('as cotas vêm da mais nova para a mais antiga e só do fundo', () {
    final other = FundQuota(
      assetId: 2,
      date: d('2026-01-09'),
      value: 99,
      createdAt: d('2026-01-09'),
    );
    final report = reports(
      [tx(1, TransactionType.compra, '2025-01-02', 100, 10)],
      quotas: [quota('2025-12-01', 11), quota('2026-01-05', 12), other],
    ).single;
    expect(report.quotas.map((q) => q.value), [12, 11]);
  });

  test('a cota vira o preço do dia na ida e na volta do banco', () {
    final original = FundQuota(
      id: 7,
      assetId: 1,
      date: d('2026-01-05'),
      value: 12.34,
      notes: 'extrato',
      createdAt: d('2026-01-06'),
    );
    final map = original.toMap();
    expect(map['quota_date'], '2026-01-05');
    expect(map['source'], 'manual');
    final back = FundQuota.fromMap(map);
    expect(back.date, d('2026-01-05'));
    expect(back.value, 12.34);
    expect(back.notes, 'extrato');
  });

  test('o come-cotas tira cotas da posição sem mexer no preço médio', () {
    final positions = PortfolioRepository.computePositions(
      [fund],
      [
        tx(1, TransactionType.compra, '2025-01-02', 100, 10),
        tx(2, TransactionType.comeCotas, '2025-05-30', 2.5, 12),
      ],
    );
    expect(positions.single.netQuantity, closeTo(97.5, 1e-9));
    expect(positions.single.averageBuyPrice, 10);
  });

  test('o come-cotas pede quantidade e valor da cota, mas não ativo de caixa', () {
    expect(TransactionType.comeCotas.value, 'come_cotas');
    expect(TransactionType.fromValue('come_cotas'), TransactionType.comeCotas);
    expect(TransactionType.comeCotas.needsAsset, isTrue);
    expect(TransactionType.comeCotas.needsQuantity, isTrue);
    expect(TransactionType.comeCotas.needsUnitPrice, isTrue);
    expect(AssetClass.fromValue('fundo'), AssetClass.fundo);
    expect(AssetClass.fundo.hasAutoQuote, isFalse);
    expect(AssetClass.fundo.defaultCurrency, 'BRL');
  });

  group('textos da tela', () {
    test('a situação diz o próximo come-cotas, o imposto e as cotas', () {
      final report = reports(
        [tx(1, TransactionType.compra, '2025-01-02', 100, 10)],
        quotas: [quota('2026-01-05', 12)],
      ).single;
      expect(
        comeCotasStatusText(report.estimate),
        'Próximo come-cotas em 29/05/2026: R\$ 30,00 (2.5000 cotas)',
      );
      expect(
        comeCotasBasisText(report.estimate),
        'Base por cota R\$ 10,00 (custo médio) · ganho R\$ 200,00 × 15.0%',
      );
    });

    test('sem tipo, sem cota ou de ações a tela explica o motivo', () {
      String statusOf({FundTerm? term, List<FundQuota> quotas = const []}) =>
          comeCotasStatusText(
            reports(
              [tx(1, TransactionType.compra, '2025-01-02', 100, 10)],
              term: term,
              quotas: quotas,
            ).single.estimate,
          );
      expect(statusOf(), contains('Escolha o tipo do fundo'));
      expect(statusOf(term: FundTerm.longTerm), contains('Lance o valor da cota'));
      expect(
        statusOf(term: FundTerm.equity, quotas: [quota('2026-01-05', 12)]),
        'Fundo de ações: sem come-cotas.',
      );
    });

    test('avisa cota velha e come-cotas sem registro', () {
      final report = reports(
        [tx(1, TransactionType.compra, '2025-01-02', 100, 10)],
        quotas: [quota('2026-08-01', 12)],
        today: '2026-10-22',
      ).single;
      final warnings = fundWarnings(report);
      expect(warnings, hasLength(2));
      expect(warnings[0], contains('mais de 35 dias'));
      expect(warnings[1], contains('29/05/2026'));
    });

    test('avisa resgate maior que as aplicações lançadas', () {
      final report = reports([
        tx(1, TransactionType.compra, '2025-01-01', 10, 10),
        tx(2, TransactionType.venda, '2026-06-01', 15, 12),
      ]).single;
      expect(
        fundWarnings(report).single,
        contains('maior que as aplicações lançadas'),
      );
    });
  });
}
