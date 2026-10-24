import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/come_cotas.dart';
import 'package:anchor_mobile/domain/fund_compensation.dart';
import 'package:anchor_mobile/domain/fund_redemption.dart';
import 'package:anchor_mobile/models/fund.dart';

// Porta dos testes de `domain/tax/fund_compensation.rs` do desktop; as contas à mão
// estão nos comentários.
void main() {
  DateTime d(String s) => DateTime.parse('${s}T00:00:00Z');

  FundEvent buy(int id, String date, double quantity, double cost) =>
      FundEvent.buy(date: d(date), id: id, quantity: quantity, cost: cost);

  FundEvent redeem(int id, String date, double quantity, double quotaValue) =>
      FundEvent.redeem(
        date: d(date),
        id: id,
        quantity: quantity,
        quotaValue: quotaValue,
      );

  CompFund fixture(
    int assetId,
    String ticker,
    FundTerm term,
    String? administrator,
    List<FundEvent> events,
  ) => CompFund(
    assetId: assetId,
    ticker: ticker,
    term: term,
    administrator: administrator,
    redemptions: estimateRedemptions(events, term),
  );

  /// Fundo que perde: 100 cotas a 10,00 resgatadas a [quota] em 01/06/2025 (151 dias,
  /// sem IOF).
  CompFund loser(
    int id,
    String ticker,
    FundTerm term,
    String? admin,
    double quota,
  ) => fixture(id, ticker, term, admin, [
    buy(1, '2025-01-01', 100, 1000),
    redeem(2, '2025-06-01', 100, quota),
  ]);

  /// Fundo que ganha: 100 cotas a 10,00 resgatadas a 12,00 em [date] (243 dias em
  /// 01/09/2025: faixa de 20%).
  CompFund winner(
    int id,
    String ticker,
    FundTerm term,
    String? admin,
    String date,
  ) => fixture(id, ticker, term, admin, [
    buy(1, '2025-01-01', 100, 1000),
    redeem(2, date, 100, 12),
  ]);

  double sum(List<LossCredit> credits) =>
      credits.fold(0, (total, credit) => total + credit.amount);

  test('a perda de um fundo zera o IR posterior de outro da mesma administradora', () {
    // X perde 100 × (8 − 10) = 200; Y ganha 200 a 20% = 40 de IR, que a perda zera
    // (40 ÷ 0,20 = 200 de crédito)
    final result = compensate([
      loser(1, 'XPTO11', FundTerm.longTerm, 'XP', 8),
      winner(2, 'YPTO11', FundTerm.longTerm, 'xp ', '2025-09-01'),
    ]);
    final x = result.redemptions[0].single;
    final y = result.redemptions[1].single;

    expect(x.lossCreated, closeTo(200, 1e-6));
    expect(x.ir, closeTo(0, 1e-6));
    expect(y.irBeforeCompensation, closeTo(40, 1e-6));
    expect(y.ir, closeTo(0, 1e-6));
    expect(y.lossCreditUsed, closeTo(200, 1e-6));
    expect(y.net, closeTo(1200, 1e-6));
    expect(result.credits, isEmpty, reason: 'o crédito foi todo usado');
  });

  test('um crédito parcial reduz o IR na proporção', () {
    // perda de 50 (100 × (9,5 − 10)); o ganho de 200 paga (200 − 50) × 20% = 30
    final result = compensate([
      loser(1, 'XPTO11', FundTerm.longTerm, 'XP', 9.5),
      winner(2, 'YPTO11', FundTerm.longTerm, 'XP', '2025-09-01'),
    ]);
    final y = result.redemptions[1].single;

    expect(y.ir, closeTo(30, 1e-6));
    expect(y.lossCreditUsed, closeTo(50, 1e-6));
    expect(y.net, closeTo(1200 - 30, 1e-6));
  });

  test('administradoras ou tipos diferentes não se compensam', () {
    final otherAdmin = compensate([
      loser(1, 'XPTO11', FundTerm.longTerm, 'XP', 8),
      winner(2, 'YPTO11', FundTerm.longTerm, 'BTG', '2025-09-01'),
    ]);
    expect(otherAdmin.redemptions[1].single.ir, closeTo(40, 1e-6));
    expect(sum(otherAdmin.credits), closeTo(200, 1e-6),
        reason: 'o crédito fica no grupo da XP');

    // curto prazo: 22,5% até 180 dias; 243 dias = 20%, mesmo IR de 40
    final otherTerm = compensate([
      loser(1, 'XPTO11', FundTerm.longTerm, 'XP', 8),
      winner(2, 'YPTO11', FundTerm.shortTerm, 'XP', '2025-09-01'),
    ]);
    expect(otherTerm.redemptions[1].single.ir, closeTo(40, 1e-6));
    expect(otherTerm.redemptions[1].single.lossCreditUsed, closeTo(0, 1e-6));
  });

  test('ganho no mesmo dia ou antes da perda não recebe crédito', () {
    // o ganho de Y é no mesmo dia da perda de X (01/06/2025)
    final sameDay = compensate([
      loser(1, 'XPTO11', FundTerm.longTerm, 'XP', 8),
      winner(2, 'YPTO11', FundTerm.longTerm, 'XP', '2025-06-01'),
    ]);
    // 151 dias: 22,5% sobre 200 = 45, sem abatimento
    expect(sameDay.redemptions[1].single.ir, closeTo(45, 1e-6));
    expect(sameDay.redemptions[1].single.lossCreditUsed, closeTo(0, 1e-6));

    // o ganho vem ANTES da perda: não é retroativo
    final before = compensate([
      loser(1, 'XPTO11', FundTerm.longTerm, 'XP', 8),
      winner(2, 'YPTO11', FundTerm.longTerm, 'XP', '2025-05-01'),
    ]);
    expect(before.redemptions[1].single.lossCreditUsed, closeTo(0, 1e-6));
    expect(before.credits.single.amount, closeTo(200, 1e-6));
  });

  test('o crédito é usado lote a lote, do mais antigo, e acaba', () {
    // Y: lotes de 50 cotas (01/01 e 01/03/2025); resgate de 100 a 12,00 em 01/09/2025:
    // os dois na faixa de 20% (243 e 184 dias), rendimento 100 cada, IR 20 cada.
    // Crédito 120: lote 1 usa 100 (IR 0), lote 2 usa 20 (IR 16)
    final result = compensate([
      loser(1, 'XPTO11', FundTerm.longTerm, 'XP', 8.8), // perde 100 × 1,2 = 120
      fixture(2, 'YPTO11', FundTerm.longTerm, 'XP', [
        buy(1, '2025-01-01', 50, 500),
        buy(2, '2025-03-01', 50, 500),
        redeem(3, '2025-09-01', 100, 12),
      ]),
    ]);
    final y = result.redemptions[1].single;

    expect(result.redemptions[0].single.lossCreated, closeTo(120, 1e-6));
    expect(y.lots[0].ir, closeTo(0, 1e-6));
    expect(y.lots[1].ir, closeTo(16, 1e-6));
    expect(y.ir, closeTo(16, 1e-6));
    expect(y.lossCreditUsed, closeTo(120, 1e-6));
    expect(y.irBeforeCompensation, closeTo(40, 1e-6));
    expect(result.credits, isEmpty);
  });

  test('o crédito é gasto uma vez entre os resgates seguintes', () {
    // perda de 100 (100 × (9 − 10)); duas vendas de 50 cotas a 14,00 (rendimento 200,
    // IR 40 cada): a 1ª usa 100 de crédito (IR 20), a 2ª nada
    final result = compensate([
      loser(1, 'XPTO11', FundTerm.longTerm, 'XP', 9),
      fixture(2, 'YPTO11', FundTerm.longTerm, 'XP', [
        buy(1, '2025-01-01', 100, 1000),
        redeem(2, '2025-09-01', 50, 14),
        redeem(3, '2025-09-02', 50, 14),
      ]),
    ]);
    final first = result.redemptions[1][0];
    final second = result.redemptions[1][1];

    expect(first.lossCreditUsed, closeTo(100, 1e-6));
    expect(first.ir, closeTo(20, 1e-6));
    expect(second.lossCreditUsed, closeTo(0, 1e-6));
    expect(second.ir, closeTo(40, 1e-6));
    expect(result.credits, isEmpty);
  });

  test('fundo sem administradora compensa só consigo mesmo', () {
    // Z perde num resgate e ganha noutro, sem administradora: compensa consigo; W, sem
    // administradora, não recebe nada de Z
    final result = compensate([
      fixture(1, 'ZPTO11', FundTerm.longTerm, null, [
        buy(1, '2025-01-01', 100, 1000),
        buy(2, '2025-02-01', 100, 1000),
        redeem(3, '2025-05-01', 100, 8),
        redeem(4, '2025-09-01', 100, 12),
      ]),
      winner(2, 'WPTO11', FundTerm.longTerm, null, '2025-09-01'),
    ]);
    final z = result.redemptions[0];

    expect(z[0].lossCreated, closeTo(200, 1e-6));
    // 2º lote: 01/02 → 01/09 = 212 dias, 20% sobre 200 = 40, zerado pelo crédito de 200
    expect(z[1].irBeforeCompensation, closeTo(40, 1e-6));
    expect(z[1].ir, closeTo(0, 1e-6));
    expect(result.redemptions[1].single.ir, closeTo(40, 1e-6),
        reason: 'W não recebe crédito de Z');
  });

  test('o come-cotas já pago não gasta crédito à toa', () {
    // lote de 100 cotas com rendimento 200 (IR 40) e come-cotas pago de 36: o IR a
    // pagar é 4 e só ele gasta crédito (4 ÷ 0,20 = 20), não os 200 inteiros
    final result = compensate([
      loser(1, 'XPTO11', FundTerm.longTerm, 'XP', 8),
      fixture(2, 'YPTO11', FundTerm.longTerm, 'XP', [
        buy(1, '2025-01-01', 100, 1000),
        FundEvent.comeCotas(
          date: d('2025-05-30'),
          id: 2,
          quantity: 3,
          quotaValue: 12,
        ),
        redeem(3, '2025-09-01', 97, 12),
      ]),
    ]);
    final y = result.redemptions[1].single;

    // rendimento nominal = 97 × 2 + 3 × 2 = 200, IR devido 40; come-cotas pago 3 × 12 = 36
    expect(y.irBeforeCompensation, closeTo(4, 1e-6));
    expect(y.ir, closeTo(0, 1e-6));
    expect(y.lossCreditUsed, closeTo(20, 1e-6));
    expect(result.credits.single.amount, closeTo(180, 1e-6));
  });

  test('sem prejuízo nada muda', () {
    final result = compensate([
      winner(1, 'XPTO11', FundTerm.longTerm, 'XP', '2025-09-01'),
      winner(2, 'YPTO11', FundTerm.longTerm, 'XP', '2025-10-01'),
    ]);

    for (final redemptions in result.redemptions) {
      final r = redemptions.single;
      expect(r.ir, closeTo(r.irBeforeCompensation, 1e-6));
      expect(r.lossCreditUsed, 0);
      expect(r.lossCreated, 0);
    }
    expect(result.credits, isEmpty);
  });

  test('a administradora é comparada sem caixa nem espaços sobrando', () {
    expect(normalizeAdministrator('  XP   Investimentos '), 'xp investimentos');
    expect(normalizeAdministrator('   '), isNull);
    expect(normalizeAdministrator(null), isNull);
    expect(collapseAdministrator('  XP   Investimentos '), 'XP Investimentos');
    expect(collapseAdministrator(' '), isNull);
  });

  FundHolding holding(int assetId, String ticker, double quantity, double quota) =>
      FundHolding(
        assetId: assetId,
        ticker: ticker,
        name: ticker,
        term: FundTerm.longTerm,
        quantity: quantity,
        averageCost: 10,
        firstBuy: d('2026-07-01'),
        quotas: [QuotaValue(d('2026-10-20'), quota)],
      );

  test('o saldo do grupo abate o próximo come-cotas sem uso duplo', () {
    // dois fundos do mesmo grupo, ganho de 200 cada (15% = 30 de imposto cada); saldo 250
    final estimates = estimateComeCotas([
      holding(1, 'AAA11', 100, 12),
      holding(2, 'BBB11', 100, 12),
    ], d('2026-10-22'));
    applyLossCredit(estimates, [
      const LossCredit(
        administrator: 'XP',
        term: FundTerm.longTerm,
        tickers: ['AAA11', 'BBB11'],
        assetIds: [1, 2],
        amount: 250,
      ),
    ]);
    final a = estimates[0];
    final b = estimates[1];

    // AAA11 (1º por ticker) usa 200 e zera; BBB11 usa os 50 que sobraram: ganho 150,
    // imposto 22,50, 1,875 cotas
    expect(a.lossCreditUsed, closeTo(200, 1e-6));
    expect(a.tax, closeTo(0, 1e-6));
    expect(a.taxBeforeCompensation, closeTo(30, 1e-6));
    expect(b.lossCreditUsed, closeTo(50, 1e-6));
    expect(b.gain, closeTo(150, 1e-6));
    expect(b.tax, closeTo(22.5, 1e-6));
    expect(b.quotasDeducted, closeTo(1.875, 1e-6));
    expect(b.taxBeforeCompensation, closeTo(30, 1e-6));
  });

  test('a estimativa ignora outros grupos e fundos de ações', () {
    final equity = FundHolding(
      assetId: 3,
      ticker: 'CCC11',
      name: 'CCC11',
      term: FundTerm.equity,
      quantity: 100,
      averageCost: 10,
      firstBuy: d('2026-07-01'),
      quotas: [QuotaValue(d('2026-10-20'), 12)],
    );
    final estimates = estimateComeCotas([
      holding(1, 'AAA11', 100, 12),
      equity,
    ], d('2026-10-22'));
    applyLossCredit(estimates, [
      const LossCredit(
        administrator: 'BTG',
        term: FundTerm.longTerm,
        tickers: ['ZZZ11'],
        assetIds: [9],
        amount: 500,
      ),
    ]);

    expect(estimates[0].lossCreditUsed, 0);
    expect(estimates[0].tax, closeTo(30, 1e-6));
    expect(estimates[1].status, EstimateStatus.exempt);
  });
}
