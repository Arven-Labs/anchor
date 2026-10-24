import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/come_cotas.dart';
import 'package:anchor_mobile/domain/cost_basis.dart';
import 'package:anchor_mobile/domain/fund_tax.dart';
import 'package:anchor_mobile/models/asset.dart';
import 'package:anchor_mobile/models/fund.dart';
import 'package:anchor_mobile/models/portfolio_transaction.dart';
import 'package:anchor_mobile/screens/fund_tax_screen.dart';
import 'package:anchor_mobile/services/fund_quota_sync.dart';
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
  ).reports;

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
    expect(result.reports.map((r) => r.asset.ticker), ['FUNDO']);
  });

  group('compensação entre fundos', () {
    Asset fundAsset(int id, String ticker) => Asset(
      id: id,
      ticker: ticker,
      name: 'Fundo $ticker',
      assetClass: AssetClass.fundo,
      currency: 'BRL',
      createdAt: DateTime(2026, 1, 1),
    );

    PortfolioTransaction txOf(
      int assetId,
      int id,
      TransactionType type,
      String date,
      double quantity,
      double unitPrice,
    ) => PortfolioTransaction(
      id: id,
      assetId: assetId,
      type: type,
      quantity: quantity,
      unitPrice: unitPrice,
      totalValue: quantity * unitPrice,
      date: d(date),
      createdAt: d(date),
    );

    // A perde 100 × (8 − 10) = 200 em 01/06/2025; B ganha 200 em 01/09/2025 (243 dias,
    // 20%: IR de 40).
    final transactions = [
      txOf(1, 1, TransactionType.compra, '2025-01-01', 100, 10),
      txOf(1, 2, TransactionType.venda, '2025-06-01', 100, 8),
      txOf(2, 3, TransactionType.compra, '2025-01-01', 100, 10),
      txOf(2, 4, TransactionType.venda, '2025-09-01', 100, 12),
    ];

    FundTaxResult twoFunds(Map<int, String> administrators) => buildFundReports(
      assets: [fundAsset(1, 'AAA11'), fundAsset(2, 'BBB11')],
      transactions: transactions,
      terms: {1: FundTerm.longTerm, 2: FundTerm.longTerm},
      quotas: const [],
      today: d('2026-01-10'),
      administrators: administrators,
    );

    test('a mesma administradora zera o IR do resgate que ganhou', () {
      final result = twoFunds({1: 'XP', 2: '  xp '});
      final a = result.reports[0].redemptions.single;
      final b = result.reports[1].redemptions.single;

      expect(a.lossCreated, closeTo(200, 1e-6));
      expect(b.irBeforeCompensation, closeTo(40, 1e-6));
      expect(b.ir, closeTo(0, 1e-6));
      expect(b.lossCreditUsed, closeTo(200, 1e-6));
      expect(b.net, closeTo(1200, 1e-6));
      expect(result.lossCredits, isEmpty, reason: 'o crédito foi todo usado');
      expect(result.reports[0].administrator, 'XP');
    });

    test('sem administradora cada fundo é um grupo e o saldo fica no que perdeu', () {
      final result = twoFunds(const {});

      expect(result.reports[1].redemptions.single.ir, closeTo(40, 1e-6));
      expect(result.reports[1].redemptions.single.lossCreditUsed, 0);
      expect(result.lossCredits, hasLength(1));
      expect(result.lossCredits.single.amount, closeTo(200, 1e-6));
      expect(result.lossCredits.single.tickers, ['AAA11']);
      expect(lossCreditText(result.lossCredits.single), contains('sem administradora'));
    });

    test('um come-cotas registrado depois da perda do grupo gera o aviso na tela', () {
      // A perde 200 em 01/06/2025. C tem um come-cotas registrado de 5 cotas a 12,00
      // em 30/11/2025 (depois da perda) e outro de 1 cota a 11,00 em 01/05/2025
      // (antes dela)
      FundTaxResult result(Map<int, String> administrators) => buildFundReports(
        assets: [fundAsset(1, 'AAA11'), fundAsset(3, 'CCC11')],
        transactions: [
          txOf(1, 1, TransactionType.compra, '2025-01-01', 100, 10),
          txOf(1, 2, TransactionType.venda, '2025-06-01', 100, 8),
          txOf(3, 3, TransactionType.compra, '2025-01-01', 100, 10),
          txOf(3, 4, TransactionType.comeCotas, '2025-05-01', 1, 11),
          txOf(3, 5, TransactionType.comeCotas, '2025-11-30', 5, 12),
        ],
        terms: {1: FundTerm.longTerm, 3: FundTerm.longTerm},
        quotas: const [],
        today: d('2026-01-10'),
        administrators: administrators,
      );

      // grupos separados: o saldo é do fundo A, então C não avisa
      expect(result(const {}).reports[1].creditNotes, isEmpty);

      final grouped = result({1: 'XP', 3: 'XP'});
      final notes = grouped.reports[1].creditNotes;
      expect(notes, hasLength(1));
      expect(notes.single.date, d('2025-11-30'));
      expect(notes.single.balance, closeTo(200, 1e-6));
      expect(grouped.reports[0].creditNotes, isEmpty);
      // o aviso não gasta crédito: o saldo do grupo segue inteiro
      expect(grouped.lossCredits.single.amount, closeTo(200, 1e-6));

      final warnings = fundWarnings(grouped.reports[1])
          .where((w) => w.contains('crédito de prejuízo no grupo'))
          .toList();
      expect(warnings, hasLength(1));
      expect(warnings.single, contains('30/11/2025'));
      expect(warnings.single, contains('pode ter usado parte dele'));
    });

    test('o crédito informado no come-cotas abate o saldo e muda o aviso', () {
      // A perde 200 em 01/06/2025. C tem um come-cotas de 5 cotas a 12,00 em
      // 30/11/2025 com o crédito de prejuízo usado informado
      FundTaxResult result(double used) => buildFundReports(
        assets: [fundAsset(1, 'AAA11'), fundAsset(3, 'CCC11')],
        transactions: [
          txOf(1, 1, TransactionType.compra, '2025-01-01', 100, 10),
          txOf(1, 2, TransactionType.venda, '2025-06-01', 100, 8),
          txOf(3, 3, TransactionType.compra, '2025-01-01', 100, 10),
          PortfolioTransaction(
            id: 4,
            assetId: 3,
            type: TransactionType.comeCotas,
            quantity: 5,
            unitPrice: 12,
            totalValue: 60,
            lossCreditUsed: used,
            date: d('2025-11-30'),
            createdAt: d('2025-11-30'),
          ),
        ],
        terms: {1: FundTerm.longTerm, 3: FundTerm.longTerm},
        quotas: const [],
        today: d('2026-01-10'),
        administrators: {1: 'XP', 3: 'XP'},
      );

      // 150 dos 200 do saldo: sem aviso e sobram 50
      final within = result(150);
      expect(within.reports[1].creditNotes, isEmpty);
      expect(within.lossCredits.single.amount, closeTo(50, 1e-6));
      expect(
        fundWarnings(within.reports[1]).where((w) => w.contains('crédito de prejuízo')),
        isEmpty,
      );

      // 300 passa do saldo de 200: só 200 saem (o saldo zera) e o aviso traz o valor
      final over = result(300);
      expect(over.lossCredits, isEmpty);
      final note = over.reports[1].creditNotes.single;
      expect(note.informed, 300);
      expect(note.balance, closeTo(200, 1e-6));
      final warning = fundWarnings(over.reports[1])
          .singleWhere((w) => w.contains('crédito de prejuízo'));
      expect(warning, contains('30/11/2025'));
      expect(warning, contains('só esse saldo foi abatido'));
    });

    test('o saldo do grupo abate o come-cotas estimado do outro fundo', () {
      // C: 100 cotas a 10,00 compradas em 01/07/2026, cota a 12,00: ganho 200, 15% = 30
      final result = buildFundReports(
        assets: [fundAsset(1, 'AAA11'), fundAsset(3, 'CCC11')],
        transactions: [
          transactions[0],
          transactions[1],
          txOf(3, 5, TransactionType.compra, '2026-07-01', 100, 10),
        ],
        terms: {1: FundTerm.longTerm, 3: FundTerm.longTerm},
        quotas: [
          FundQuota(assetId: 3, date: d('2026-10-20'), value: 12, createdAt: d('2026-10-20')),
        ],
        today: d('2026-10-22'),
        administrators: {1: 'XP', 3: 'XP'},
      );
      final estimate = result.reports[1].estimate;

      expect(estimate.lossCreditUsed, closeTo(200, 1e-6));
      expect(estimate.tax, closeTo(0, 1e-6));
      expect(estimate.taxBeforeCompensation, closeTo(30, 1e-6));
      expect(result.lossCredits.single.amount, closeTo(200, 1e-6));
      expect(comeCotasCreditText(estimate), contains('R\$ 30,00'));
    });

    test('os textos do resgate dizem o prejuízo e o crédito usado', () {
      final result = twoFunds({1: 'XP', 2: 'XP'});

      final loss = redemptionCompensationLines(result.reports[0].redemptions.single);
      expect(loss.single, contains('perdeu R\$ 200,00'));
      final used = redemptionCompensationLines(result.reports[1].redemptions.single);
      expect(used.single, allOf(contains('R\$ 200,00'), contains('R\$ 0,00 em vez de R\$ 40,00')));
      expect(comeCotasCreditText(result.reports[0].estimate), isEmpty);
    });
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

  group('fundos fechados e exclusivos (Lei 14.754)', () {
    Asset fundAsset(int id, String ticker) => Asset(
      id: id,
      ticker: ticker,
      name: 'Fundo $ticker',
      assetClass: AssetClass.fundo,
      currency: 'BRL',
      createdAt: DateTime(2026, 1, 1),
    );

    PortfolioTransaction fundTx(
      int assetId,
      int id,
      TransactionType type,
      String date,
      double quantity,
      double unitPrice,
    ) => PortfolioTransaction(
      id: id,
      assetId: assetId,
      type: type,
      quantity: quantity,
      unitPrice: unitPrice,
      totalValue: quantity * unitPrice,
      date: d(date),
      createdAt: d(date),
    );

    /// Amortização: valor recebido e cota do dia, sem quantidade.
    PortfolioTransaction amortization(
      int assetId,
      int id,
      String date,
      double amount,
      double quotaValue,
    ) => PortfolioTransaction(
      id: id,
      assetId: assetId,
      type: TransactionType.amortizacao,
      unitPrice: quotaValue,
      totalValue: amount,
      date: d(date),
      createdAt: d(date),
    );

    test('o tipo da amortização não pede quantidade nem preço de caixa', () {
      expect(TransactionType.amortizacao.value, 'amortizacao');
      expect(
        TransactionType.fromValue('amortizacao'),
        TransactionType.amortizacao,
      );
      expect(
        TransactionType.tryParse('amortizacao'),
        TransactionType.amortizacao,
      );
      expect(TransactionType.tryParse('so_no_desktop'), isNull);
      expect(TransactionType.tryParse(null), isNull);
      expect(TransactionType.amortizacao.needsAsset, isTrue);
      expect(TransactionType.amortizacao.needsQuantity, isFalse);
      expect(TransactionType.amortizacao.needsUnitPrice, isTrue);
      expect(TransactionType.amortizacao.isFundOnly, isTrue);
      expect(TransactionType.comeCotas.isFundOnly, isTrue);
      expect(TransactionType.compra.isFundOnly, isFalse);
    });

    test('a amortização não mexe na quantidade nem no preço médio', () {
      final positions = PortfolioRepository.computePositions(
        [fund],
        [
          tx(1, TransactionType.compra, '2025-01-02', 100, 10),
          amortization(1, 2, '2026-01-02', 600, 15),
        ],
      );
      expect(positions.single.netQuantity, closeTo(100, 1e-9));
      expect(positions.single.averageBuyPrice, 10);
    });

    test('baixar o custo médio na amortização deixa as cotas', () {
      // 100 cotas, custo 1.000; amortização de 600 com cota a 15,00: posição 1.500,
      // fração 0,4, custo baixado 400
      final position = CostPosition()..add(100, 1000);
      expect(position.amortize(600, 15), closeTo(400, 1e-9));
      expect(position.quantity, closeTo(100, 1e-9));
      expect(position.cost, closeTo(600, 1e-9));
      // além da posição só devolve o custo que tem; posição vazia não muda
      expect(position.amortize(99999, 15), closeTo(600, 1e-9));
      expect(position.cost, closeTo(0, 1e-9));
      expect(CostPosition().amortize(100, 15), 0);
    });

    test(
      'a amortização vira um evento do relatório, com o custo médio menor',
      () {
        final result = buildFundReports(
          assets: [fund],
          transactions: [
            tx(1, TransactionType.compra, '2022-06-01', 100, 10),
            amortization(1, 2, '2025-01-02', 600, 15),
          ],
          terms: {1: FundTerm.longTerm},
          quotas: const [],
          today: d('2026-10-22'),
          structures: {1: FundStructure.closed},
        );
        final report = result.reports.single;
        expect(report.quantity, closeTo(100, 1e-9));
        final a = report.redemptions.single;
        expect(a.isAmortization, isTrue);
        expect(a.quotas, closeTo(40, 1e-6));
        expect(a.gross, closeTo(600, 1e-6));
        expect(a.cost, closeTo(400, 1e-6));
        expect(a.iof, 0);
        // 40 × (15 − 10) = 200 a 15% (946 dias) = 30
        expect(a.ir, closeTo(30, 1e-6));
      },
    );

    test('o estoque entra na base do próximo come-cotas', () {
      // 100 cotas a 10,00, amortização de 600 a 15,00 (fração 0,4): o custo cai para
      // 6,00 por cota e o estoque tributado (4,00 por cota a 14,00) para 2,40, então a
      // base é 8,40. Cota de hoje 16,00: ganho 100 × 7,60 = 760, imposto 15% = 114.
      // Sem o estoque a base era 6,00.
      List<PortfolioTransaction> txs() => [
        tx(1, TransactionType.compra, '2022-06-01', 100, 10),
        amortization(1, 2, '2025-01-02', 600, 15),
      ];
      final quotas = [quota('2023-12-29', 14), quota('2026-10-20', 16)];
      final withEstoque = buildFundReports(
        assets: [fund],
        transactions: txs(),
        terms: {1: FundTerm.longTerm},
        quotas: quotas,
        today: d('2026-10-22'),
        structures: {1: FundStructure.closed},
        estoqueOptions: {1: EstoqueOption.general},
      ).reports.single;
      final e = withEstoque.estimate;
      expect(e.basisSource, BasisSource.estoque);
      expect(e.basisPerQuota, closeTo(8.4, 1e-9));
      expect(e.gain, closeTo(760, 1e-6));
      expect(e.tax, closeTo(114, 1e-6));
      expect(withEstoque.estoqueStatus, EstoqueStatus.estimated);
      expect(withEstoque.estoque!.base, closeTo(400, 1e-6));
      expect(withEstoque.estoque!.tax, closeTo(60, 1e-6));

      final without = buildFundReports(
        assets: [fund],
        transactions: txs(),
        terms: {1: FundTerm.longTerm},
        quotas: quotas,
        today: d('2026-10-22'),
        structures: {1: FundStructure.closed},
      ).reports.single;
      expect(without.estimate.basisSource, BasisSource.cost);
      expect(without.estimate.basisPerQuota, closeTo(6, 1e-9));
      expect(without.estoqueStatus, EstoqueStatus.notTaxed);
    });

    test(
      'o estado do estoque depende da estrutura, do tipo e da cota de 2023',
      () {
        EstoqueStatus status({
          FundTerm term = FundTerm.longTerm,
          FundStructure? structure,
          EstoqueOption? option,
          List<FundQuota> quotas = const [],
        }) => buildFundReports(
          assets: [fund],
          transactions: [tx(1, TransactionType.compra, '2022-06-01', 100, 10)],
          terms: {1: term},
          quotas: quotas,
          today: d('2026-10-22'),
          structures: {1: ?structure},
          estoqueOptions: {1: ?option},
        ).reports.single.estoqueStatus;

        // fundo aberto e fundo de ações não têm estoque
        expect(status(), EstoqueStatus.notApplicable);
        expect(
          status(term: FundTerm.equity, structure: FundStructure.closed),
          EstoqueStatus.notApplicable,
        );
        expect(
          status(
            term: FundTerm.investmentEntity,
            structure: FundStructure.exclusive,
          ),
          EstoqueStatus.notApplicable,
        );
        // fechado sem a opção, com a opção mas sem cota, e com tudo
        expect(status(structure: FundStructure.closed), EstoqueStatus.notTaxed);
        expect(
          status(
            structure: FundStructure.closed,
            option: EstoqueOption.option8,
          ),
          EstoqueStatus.needsQuota,
        );
        // cota a mais de 10 dias de 31/12/2023 não serve; 10 dias antes serve
        expect(
          status(
            structure: FundStructure.exclusive,
            option: EstoqueOption.general,
            quotas: [quota('2023-12-20', 14)],
          ),
          EstoqueStatus.needsQuota,
        );
        expect(
          status(
            structure: FundStructure.exclusive,
            option: EstoqueOption.general,
            quotas: [quota('2023-12-21', 14)],
          ),
          EstoqueStatus.estimated,
        );
      },
    );

    test('perda numa amortização gera crédito para outro fundo do grupo', () {
      // A: compra 100 a 10,00; amortização de 500 com cota a 8,00 (posição 800, fração
      // 0,625, 62,5 cotas): rendimento 62,5 × (8 − 10) = −125, perda de 125.
      // B: ganho de 200 a 20% (243 dias) = IR 40; o crédito de 125 abate o ganho para
      // 75 e o IR fica 15
      final result = buildFundReports(
        assets: [fundAsset(1, 'AAA11'), fundAsset(2, 'BBB11')],
        transactions: [
          fundTx(1, 1, TransactionType.compra, '2025-01-01', 100, 10),
          amortization(1, 2, '2025-06-01', 500, 8),
          fundTx(2, 3, TransactionType.compra, '2025-01-01', 100, 10),
          fundTx(2, 4, TransactionType.venda, '2025-09-01', 100, 12),
        ],
        terms: {1: FundTerm.longTerm, 2: FundTerm.longTerm},
        quotas: const [],
        today: d('2026-01-10'),
        administrators: {1: 'XP', 2: 'XP'},
      );
      final a = result.reports[0].redemptions.single;
      final b = result.reports[1].redemptions.single;
      expect(a.isAmortization, isTrue);
      expect(a.lossCreated, closeTo(125, 1e-6));
      expect(b.irBeforeCompensation, closeTo(40, 1e-6));
      expect(b.lossCreditUsed, closeTo(125, 1e-6));
      expect(b.ir, closeTo(15, 1e-6));
      expect(result.lossCredits, isEmpty);
    });

    test('a entidade de investimento é uma classe de compensação própria', () {
      // A (entidade) perde 200 em 01/06/2025; B (longo prazo) ganha 200 e paga IR 40.
      // Mesma administradora, mas o tipo é outro: B não usa o crédito de A.
      final result = buildFundReports(
        assets: [fundAsset(1, 'AAA11'), fundAsset(2, 'BBB11')],
        transactions: [
          fundTx(1, 1, TransactionType.compra, '2025-01-01', 100, 10),
          fundTx(1, 2, TransactionType.venda, '2025-06-01', 100, 8),
          fundTx(2, 3, TransactionType.compra, '2025-01-01', 100, 10),
          fundTx(2, 4, TransactionType.venda, '2025-09-01', 100, 12),
        ],
        terms: {1: FundTerm.investmentEntity, 2: FundTerm.longTerm},
        quotas: const [],
        today: d('2026-01-10'),
        administrators: {1: 'XP', 2: 'XP'},
      );
      expect(
        result.reports[0].redemptions.single.lossCreated,
        closeTo(200, 1e-6),
      );
      expect(result.reports[1].redemptions.single.ir, closeTo(40, 1e-6));
      expect(result.reports[1].redemptions.single.lossCreditUsed, 0);
      expect(result.lossCredits.single.amount, closeTo(200, 1e-6));
      expect(result.lossCredits.single.term, FundTerm.investmentEntity);
    });

    group('a amortização cabe no histórico?', () {
      final history = [
        tx(1, TransactionType.compra, '2025-01-02', 100, 10),
        tx(2, TransactionType.venda, '2025-06-02', 40, 12),
        tx(3, TransactionType.compra, '2026-01-02', 50, 11),
      ];

      test('precisa de cotas na data', () {
        // antes da compra e depois de vender tudo não há posição
        expect(
          amortizationProblem(
            history,
            date: d('2024-12-31'),
            quotaValue: 10,
            amount: 100,
          ),
          contains('cotas em carteira'),
        );
      });

      test('o valor não passa da posição, com a folga do desktop', () {
        // em 01/09/2025 há 60 cotas; a 15,00 a posição vale 900 (compra de 2026 fora)
        String? at(double amount) => amortizationProblem(
          history,
          date: d('2025-09-01'),
          quotaValue: 15,
          amount: amount,
        );
        expect(at(900), isNull);
        expect(at(900.5), isNull, reason: 'folga de 0,1% (0,90) sobre 900');
        expect(at(901), contains('passa do valor da posição'));
      });

      test(
        'ao editar, a própria transação não conta e a folga mínima é 0,011',
        () {
          final withIt = [
            ...history,
            PortfolioTransaction(
              id: 9,
              assetId: 1,
              type: TransactionType.venda,
              quantity: 60,
              unitPrice: 15,
              totalValue: 900,
              date: d('2025-09-01'),
              createdAt: d('2025-09-01'),
            ),
          ];
          // com a venda de 60 cotas no mesmo dia não sobra posição...
          expect(
            amortizationProblem(
              withIt,
              date: d('2025-09-01'),
              quotaValue: 15,
              amount: 100,
            ),
            isNotNull,
          );
          // ...a menos que seja ela a transação editada
          expect(
            amortizationProblem(
              withIt,
              date: d('2025-09-01'),
              quotaValue: 15,
              amount: 100,
              ignoreId: 9,
            ),
            isNull,
          );
          // posição pequena: 1 cota a 1,00 aceita 1,01 (folga de 0,011) e não 1,02
          final tiny = [tx(1, TransactionType.compra, '2025-01-02', 1, 1)];
          expect(
            amortizationProblem(
              tiny,
              date: d('2025-02-01'),
              quotaValue: 1,
              amount: 1.01,
            ),
            isNull,
          );
          expect(
            amortizationProblem(
              tiny,
              date: d('2025-02-01'),
              quotaValue: 1,
              amount: 1.02,
            ),
            isNotNull,
          );
        },
      );
    });
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

    test('o estoque de 2023 aparece na frase e na origem da base', () {
      FundReport closed({
        EstoqueOption? option,
        List<FundQuota> quotas = const [],
      }) => buildFundReports(
        assets: [fund],
        transactions: [tx(1, TransactionType.compra, '2022-06-01', 100, 10)],
        terms: {1: FundTerm.longTerm},
        quotas: quotas,
        today: d('2026-10-22'),
        structures: {1: FundStructure.closed},
        estoqueOptions: {1: ?option},
      ).reports.single;

      expect(
        estoqueText(closed()),
        contains('como o estoque de 2023 foi tributado'),
      );
      expect(
        estoqueText(closed(option: EstoqueOption.general)),
        contains('Lance a cota de 31/12/2023'),
      );
      final estimated = closed(
        option: EstoqueOption.option8,
        quotas: [quota('2023-12-29', 14), quota('2026-10-20', 16)],
      );
      final text = estoqueText(estimated);
      expect(text, contains('até 31/12/2023'));
      expect(text, contains('a 8%'));
      expect(
        comeCotasBasisText(estimated.estimate),
        contains('custo médio mais o estoque de 2023'),
      );
      // fundo aberto não tem frase de estoque
      expect(
        estoqueText(
          reports([tx(1, TransactionType.compra, '2022-06-01', 100, 10)])
              .single,
        ),
        isEmpty,
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
        'Fundo de ações ou entidade de investimento: sem come-cotas.',
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

    test('o estado da CVM diz o que fazer em cada caso', () {
      expect(cvmStatusText(null, null), isEmpty);
      expect(
        cvmStatusText(null, d('2026-10-02')),
        'Cotas lidas da CVM. Última cota da CVM: 02/10/2026.',
      );
      expect(
        cvmStatusText(const CvmStatus(CvmStatusKind.noCnpj), null),
        contains('Informe o CNPJ'),
      );
      expect(
        cvmStatusText(const CvmStatus(CvmStatusKind.notFound), null),
        contains('Confira o CNPJ'),
      );
      expect(
        cvmStatusText(
          const CvmStatus(CvmStatusKind.needsSubclass, options: ['A', 'B']),
          null,
        ),
        contains('escolha uma'),
      );
      expect(
        cvmStatusText(
          const CvmStatus(CvmStatusKind.error, message: 'HTTP 503'),
          d('2026-10-02'),
        ),
        allOf(contains('HTTP 503'), contains('02/10/2026')),
      );
      expect(cvmDownloadWarning, contains('MB'));
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

    group('come-cotas pago acima do IR devido', () {
      // 100 cotas a 10,00; come-cotas de 2,5 cotas a 12,00 (30 pagos); os resgates saem
      // a 9,00 e o rendimento fica negativo, então o IR é zero e o come-cotas pago
      // inteiro (30) é excesso, repartido entre os resgates na proporção das cotas
      // (50 de 97,5 = 15,3846 e 47,5 de 97,5 = 14,6154).
      List<PortfolioTransaction> history(List<PortfolioTransaction> sells) => [
        tx(1, TransactionType.compra, '2024-01-02', 100, 10),
        tx(2, TransactionType.comeCotas, '2024-05-31', 2.5, 12),
        ...sells,
      ];

      test('soma o excesso dos resgates e avisa em cada um', () {
        final report = reports(
          history([
            tx(3, TransactionType.venda, '2026-03-01', 50, 9),
            tx(4, TransactionType.venda, '2026-03-02', 47.5, 9),
          ]),
        ).single;

        expect(report.refundableComeCotas, closeTo(30, 1e-6));
        // o excesso não entra no líquido nem no IR
        expect(report.redemptions.every((r) => r.ir == 0), isTrue);
        expect(
          report.redemptions.first.net,
          closeTo(report.redemptions.first.gross, 1e-6),
        );
        final warnings = fundWarnings(report)
            .where((w) => w.contains('passou do IR devido'))
            .toList();
        expect(warnings, hasLength(2));
        expect(warnings.first, contains('fora do líquido'));
        expect(warnings.first, isNot(contains('não é modelado')));

        final text = refundableText(report);
        expect(text, contains('possivelmente restituível'));
        expect(text, contains('PER/DCOMP'));
        expect(text, contains('em até 5 anos'));
        expect(text, contains('não foi confirmada'));
      });

      test('sem excesso não há frase nem soma', () {
        final report = reports([
          tx(1, TransactionType.compra, '2025-01-01', 100, 10),
          tx(2, TransactionType.venda, '2026-06-01', 100, 12),
        ]).single;

        expect(report.refundableComeCotas, 0);
        expect(refundableText(report), isEmpty);
        expect(
          fundWarnings(report).where((w) => w.contains('passou do IR devido')),
          isEmpty,
        );
      });
    });
  });
}
