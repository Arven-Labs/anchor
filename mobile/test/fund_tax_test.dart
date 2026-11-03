import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/come_cotas.dart';
import 'package:anchor_mobile/domain/cost_basis.dart';
import 'package:anchor_mobile/domain/fund_compensation.dart' show LossCredit, RecordNote;
import 'package:anchor_mobile/domain/fund_redemption.dart';
import 'package:anchor_mobile/domain/fund_tax.dart';
import 'package:anchor_mobile/l10n/app_localizations.dart';
import 'package:anchor_mobile/l10n/fund_tax_labels.dart' as labels;
import 'package:anchor_mobile/models/asset.dart';
import 'package:anchor_mobile/models/fund.dart';
import 'package:anchor_mobile/models/portfolio_transaction.dart';
import 'package:anchor_mobile/services/fund_quota_sync.dart';
import 'package:anchor_mobile/services/portfolio_repository.dart';

void main() {
  DateTime d(String s) => DateTime.parse('${s}T00:00:00Z');

  // Os textos da tela moram em `l10n/fund_tax_labels.dart` (ADR 0001) e recebem o idioma.
  // Os testes em pt chamam por estes embrulhos; o inglês tem o grupo próprio no fim.
  late AppLocalizations pt;
  late AppLocalizations en;
  setUpAll(() async {
    pt = await AppLocalizations.delegate.load(const Locale('pt'));
    en = await AppLocalizations.delegate.load(const Locale('en'));
  });
  String comeCotasStatusText(ComeCotasEstimate e) => labels.comeCotasStatusText(pt, e);
  String comeCotasBasisText(ComeCotasEstimate e) => labels.comeCotasBasisText(pt, e);
  String comeCotasCreditText(ComeCotasEstimate e) => labels.comeCotasCreditText(pt, e);
  String estoqueText(FundReport r) => labels.estoqueText(pt, r);
  String refundableText(FundReport r) => labels.refundableText(pt, r);
  List<String> fundWarnings(FundReport r) => labels.fundWarnings(pt, r);
  List<String> redemptionCompensationLines(RedemptionEstimate r) =>
      labels.redemptionCompensationLines(pt, r);
  String lossCreditText(LossCredit c) => labels.lossCreditText(pt, c);
  String recordNoteText(RecordNote n) => labels.recordNoteText(pt, n);
  String cvmStatusText(CvmStatus? s, DateTime? last) => labels.cvmStatusText(pt, s, last);

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
      expect(
        labels.lossCreditText(en, result.lossCredits.single),
        'R\$ 200.00 — AAA11 (no administrator, Long term)',
      );
      expect(
        lossCreditText(result.lossCredits.single),
        'R\$ 200,00 — AAA11 (sem administradora, Longo prazo)',
      );
    });

    test('sair de uma administradora com saldo de perda avisa até quando vai a guarda', () {
      // A (XP) perde 200 e zera a posição em 01/06/2025; B não tem administradora. O
      // saldo de 200 fica no grupo XP e a guarda do registro vai até 31/12/2026.
      final result = twoFunds({1: 'XP'});
      final note = result.recordNotes.single;
      expect(note.administrator, 'XP');
      expect(note.redeemedOn, d('2025-06-01'));
      expect(note.guaranteedUntil, DateTime.utc(2026, 12, 31));
      expect(note.expired, isFalse);
      expect(note.balance, closeTo(200, 1e-6));
      final text = recordNoteText(note);
      expect(text, contains('XP'));
      expect(text, contains('31/12/2026'));
      expect(text, contains('A lei não limita'));
      expect(
        labels.recordNoteText(en, note),
        'All the funds you entered from administrator XP were redeemed by 06/01/2025. '
        "The administrator is only required to keep the loss record (R\$ 200.00) until 12/31/2026 "
        '(IN RFB 1.585, art. 15 §2). The law does not limit your right to offset, but check '
        'with the administrator.',
      );

      // depois de 31/12/2026 a garantia terminou
      final late = buildFundReports(
        assets: [fundAsset(1, 'AAA11'), fundAsset(2, 'BBB11')],
        transactions: transactions,
        terms: {1: FundTerm.longTerm, 2: FundTerm.longTerm},
        quotas: const [],
        today: d('2027-01-05'),
        administrators: {1: 'XP'},
      );
      expect(late.recordNotes.single.expired, isTrue);
      expect(recordNoteText(late.recordNotes.single), contains('terminou em 31/12/2026'));
      expect(
        labels.recordNoteText(en, late.recordNotes.single),
        contains("obligation to keep the loss record (R\$ 200.00) ended on 12/31/2026"),
      );
    });

    test('sem saldo de perda, ou sem administradora, não há aviso de guarda', () {
      // a perda de A foi toda usada pelo ganho de B (mesma administradora): saldo zero
      expect(twoFunds({1: 'XP', 2: '  xp '}).recordNotes, isEmpty);
      expect(twoFunds(const {}).recordNotes, isEmpty);
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
      expect(
        labels
            .fundWarnings(en, grouped.reports[1])
            .singleWhere((w) => w.contains('loss credit in the group')),
        startsWith('The semiannual quota tax of 11/30/2025 was recorded with R\$ '),
      );
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
      expect(
        labels
            .fundWarnings(en, over.reports[1])
            .singleWhere((w) => w.contains('loss credit')),
        'The semiannual quota tax of 11/30/2025 says R\$ 300.00 of loss credit was used, '
        'but the group only had R\$ 200.00: only that balance was deducted. Check the amount.',
      );
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
      expect(
        labels.comeCotasCreditText(en, estimate),
        'Loss credit of R\$ 200.00 has already reduced the gain: the tax would be R\$ 30.00 '
        'without it.',
      );
    });

    test('os textos do resgate dizem o prejuízo e o crédito usado', () {
      final result = twoFunds({1: 'XP', 2: 'XP'});

      final loss = redemptionCompensationLines(result.reports[0].redemptions.single);
      expect(loss.single, contains('perdeu R\$ 200,00'));
      final used = redemptionCompensationLines(result.reports[1].redemptions.single);
      expect(used.single, allOf(contains('R\$ 200,00'), contains('R\$ 0,00 em vez de R\$ 40,00')));
      expect(comeCotasCreditText(result.reports[0].estimate), isEmpty);

      expect(
        labels.redemptionCompensationLines(en, result.reports[0].redemptions.single).single,
        'This redemption lost R\$ 200.00: it offsets the gain of later redemptions and '
        'semiannual quota tax in the same group (same administrator and same type).',
      );
      expect(
        labels.redemptionCompensationLines(en, result.reports[1].redemptions.single).single,
        'Losses from earlier redemptions in the group offset R\$ 200.00 of the gain: tax '
        'R\$ 0.00 instead of R\$ 40.00.',
      );
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
          )?.code,
          AmortizationProblemCode.noSharesOnDate,
        );
      });

      test('o valor não passa da posição, com a folga do desktop', () {
        // em 01/09/2025 há 60 cotas; a 15,00 a posição vale 900 (compra de 2026 fora)
        AmortizationProblem? at(double amount) => amortizationProblem(
          history,
          date: d('2025-09-01'),
          quotaValue: 15,
          amount: amount,
        );
        expect(at(900), isNull);
        expect(at(900.5), isNull, reason: 'folga de 0,1% (0,90) sobre 900');
        expect(at(901)?.code, AmortizationProblemCode.exceedsPosition);
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
        'Próximo come-cotas em 29/05/2026: R\$ 30,00 (2,5000 cotas)',
      );
      expect(
        comeCotasBasisText(report.estimate),
        'Base por cota R\$ 10,00 (custo médio) · ganho R\$ 200,00 × 15,0%',
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
      expect(pt.fundCvmDownloadWarning, contains('MB'));
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
        expect(text, contains('não prevê restituição'));
        expect(text, contains('perda a compensar'));
        expect(text, contains('IN RFB 1.585/2015, art. 15'));
        expect(text, isNot(contains('PER/DCOMP')));
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

      test('en: a frase do excesso e o aviso de cada resgate', () {
        final report = reports(
          history([
            tx(3, TransactionType.venda, '2026-03-01', 50, 9),
            tx(4, TransactionType.venda, '2026-03-02', 47.5, 9),
          ]),
        ).single;

        expect(
          labels.refundableText(en, report),
          'Semiannual quota tax paid above the tax due: R\$ 30.00. The law does not provide a '
          'refund of this excess (IN RFB 1.585/2015, art. 15; Law 14.754/2023, art. 17, §6). It '
          'is tax paid on income that later disappeared, and it comes back as a loss to offset '
          'against later gains. The app already adds this loss to the loss credit; the excess '
          'itself is not included in the net amounts.',
        );
        final warnings = labels
            .fundWarnings(en, report)
            .where((w) => w.contains('exceeded the tax due'))
            .toList();
        expect(warnings, hasLength(2));
        expect(
          warnings.first,
          'In the redemption of 03/01/2026 the semiannual quota tax paid exceeded the tax due by '
          'R\$ 15.38: it stays out of the net amount (see the note about the excess).',
        );
      });
    });
  });

  group('textos da tela em inglês', () {
    String statusOf({FundTerm? term, List<FundQuota> quotas = const []}) =>
        labels.comeCotasStatusText(
          en,
          reports(
            [tx(1, TransactionType.compra, '2025-01-02', 100, 10)],
            term: term,
            quotas: quotas,
          ).single.estimate,
        );

    test('a situação do come-cotas, a base e os motivos de não estimar', () {
      final report = reports(
        [tx(1, TransactionType.compra, '2025-01-02', 100, 10)],
        quotas: [quota('2026-01-05', 12)],
      ).single;
      expect(
        labels.comeCotasStatusText(en, report.estimate),
        'Next semiannual quota tax on 05/29/2026: R\$ 30.00 (2.5000 quotas)',
      );
      expect(
        labels.comeCotasBasisText(en, report.estimate),
        'Basis per quota R\$ 10.00 (average cost) · gain R\$ 200.00 × 15.0%',
      );
      expect(
        statusOf(),
        'Choose the fund type to estimate the semiannual quota tax and the redemption tax.',
      );
      expect(
        statusOf(term: FundTerm.longTerm),
        'Enter the quota value to estimate the semiannual quota tax.',
      );
      expect(
        statusOf(term: FundTerm.equity, quotas: [quota('2026-01-05', 12)]),
        'Equity fund or investment entity: no semiannual quota tax.',
      );
      final empty = reports([
        tx(1, TransactionType.compra, '2025-01-02', 100, 10),
        tx(2, TransactionType.venda, '2025-06-01', 100, 11),
      ]).single;
      expect(labels.comeCotasStatusText(en, empty.estimate), 'No quotas in the portfolio.');
    });

    test('o estoque de 2023 e a origem da base', () {
      FundReport closed({EstoqueOption? option, List<FundQuota> quotas = const []}) =>
          buildFundReports(
            assets: [fund],
            transactions: [tx(1, TransactionType.compra, '2022-06-01', 100, 10)],
            terms: {1: FundTerm.longTerm},
            quotas: quotas,
            today: d('2026-10-22'),
            structures: {1: FundStructure.closed},
            estoqueOptions: {1: ?option},
          ).reports.single;

      expect(
        labels.estoqueText(en, closed()),
        'Say how the 2023 stock was taxed to estimate the tax and the cost of the semiannual '
        'quota tax.',
      );
      expect(
        labels.estoqueText(en, closed(option: EstoqueOption.general)),
        'Enter the quota of 12/31/2023 (or up to 10 days before) to estimate the stock.',
      );
      final estimated = closed(
        option: EstoqueOption.option8,
        quotas: [quota('2023-12-29', 14), quota('2026-10-20', 16)],
      );
      final text = labels.estoqueText(en, estimated);
      expect(text, startsWith('2023 stock: gain of R\$ '));
      expect(text, allOf(contains('up to 12/31/2023'), contains('at 8% = R\$ ')));
      expect(
        labels.comeCotasBasisText(en, estimated.estimate),
        contains('(average cost plus the 2023 stock already taxed)'),
      );
      expect(
        labels.estoqueText(
          en,
          reports([tx(1, TransactionType.compra, '2022-06-01', 100, 10)]).single,
        ),
        isEmpty,
      );
    });

    test('avisos de cota velha, come-cotas sem registro e resgate maior que as aplicações', () {
      final stale = reports(
        [tx(1, TransactionType.compra, '2025-01-02', 100, 10)],
        quotas: [quota('2026-08-01', 12)],
        today: '2026-10-22',
      ).single;
      final warnings = labels.fundWarnings(en, stale);
      expect(
        warnings[0],
        'The latest quota value is more than 35 days old: the estimate may be out of date.',
      );
      expect(
        warnings[1],
        'The semiannual quota tax of 05/29/2026 has already passed with no deduction recorded: '
        'enter the Semiannual quota tax transaction so the basis is right.',
      );

      final uncovered = reports([
        tx(1, TransactionType.compra, '2025-01-01', 10, 10),
        tx(2, TransactionType.venda, '2026-06-01', 15, 12),
      ]).single;
      expect(
        labels.fundWarnings(en, uncovered).single,
        'Redemption of 06/01/2026 is larger than the investments entered: the cost counts only '
        'the known part.',
      );
    });

    test('o estado da CVM em cada caso', () {
      String t(CvmStatus? s, [DateTime? last]) => labels.cvmStatusText(en, s, last);
      expect(t(null), isEmpty);
      expect(t(null, d('2026-10-02')), 'Quotas read from the CVM. Last CVM quota: 10/02/2026.');
      expect(
        t(const CvmStatus(CvmStatusKind.noCnpj)),
        "Enter the fund's CNPJ to read the quota from the CVM.",
      );
      expect(t(const CvmStatus(CvmStatusKind.pending)), 'Not read yet.');
      expect(
        t(const CvmStatus(CvmStatusKind.notFound)),
        'The CVM returned no quota for this CNPJ (or subclass) in the months read. Check the '
        'CNPJ.',
      );
      expect(
        t(const CvmStatus(CvmStatusKind.needsSubclass, options: ['A'])),
        'This CNPJ only has subclasses in the report: choose one.',
      );
      expect(
        t(const CvmStatus(CvmStatusKind.error, message: 'HTTP 503'), d('2026-10-02')),
        'Could not read the CVM right now (HTTP 503). Try again in a moment. Last CVM quota: '
        '10/02/2026.',
      );
      expect(
        t(const CvmStatus(CvmStatusKind.error)),
        'Could not read the CVM right now (error). Try again in a moment.',
      );
    });

    test('tipo, estrutura e estoque têm nome nos dois idiomas', () {
      for (final l in [en, pt]) {
        for (final term in FundTerm.values) {
          expect(labels.fundTermName(l, term), isNotEmpty);
        }
        for (final s in FundStructure.values) {
          expect(labels.fundStructureName(l, s), isNotEmpty);
        }
        for (final o in EstoqueOption.values) {
          expect(labels.estoqueOptionName(l, o), isNotEmpty);
        }
      }
      expect(labels.fundTermName(en, FundTerm.investmentEntity), 'Investment entity');
      expect(labels.fundStructureName(en, FundStructure.exclusive), 'Exclusive');
      expect(labels.estoqueOptionName(en, EstoqueOption.option8), 'Option (8%)');
      // pt: o texto que os `label` dos enums guardavam, por extenso (os `label` saíram).
      expect({for (final t in FundTerm.values) t: labels.fundTermName(pt, t)}, {
        FundTerm.longTerm: 'Longo prazo',
        FundTerm.shortTerm: 'Curto prazo',
        FundTerm.equity: 'Ações',
        FundTerm.investmentEntity: 'Entidade de investimento',
      });
      expect({for (final s in FundStructure.values) s: labels.fundStructureName(pt, s)}, {
        FundStructure.open: 'Aberto',
        FundStructure.closed: 'Fechado',
        FundStructure.exclusive: 'Exclusivo',
      });
      expect({for (final o in EstoqueOption.values) o: labels.estoqueOptionName(pt, o)}, {
        EstoqueOption.general: 'Regra geral (15%)',
        EstoqueOption.option8: 'Opção (8%)',
      });
    });

    test('erros de gravação do fundo por código, com texto nos dois idiomas', () {
      for (final code in FundSettingErrorCode.values) {
        final error = FundSettingException(code);
        for (final l in [en, pt]) {
          expect(labels.fundSettingErrorText(l, error), isNotEmpty, reason: '$code');
        }
        expect(error, isA<FormatException>(), reason: 'on FormatException continua valendo');
      }
      expect(
        labels.fundSettingErrorText(
          en,
          FundSettingException(FundSettingErrorCode.cnpjMustHave14Digits),
        ),
        'The CNPJ must have 14 digits',
      );
      expect(
        labels.fundSettingErrorText(
          pt,
          FundSettingException(FundSettingErrorCode.needTermBeforeStructure),
        ),
        'Escolha o tipo do fundo antes da estrutura',
      );
      expect(
        labels.fundSettingErrorText(
          en,
          FundSettingException(FundSettingErrorCode.needTermBeforeStructure),
        ),
        'Choose the fund type before the structure',
      );
    });
  });
}
