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

  // Aviso de come-cotas registrado feito com saldo de prejuízo no grupo: porte dos
  // testes de `CreditNote` do desktop.
  CompFund withComeCotas(CompFund fund, List<String> dates) => CompFund(
    assetId: fund.assetId,
    ticker: fund.ticker,
    term: fund.term,
    administrator: fund.administrator,
    redemptions: fund.redemptions,
    comeCotas: [for (final date in dates) ComeCotasMark(d(date))],
  );

  /// O mesmo fundo com come-cotas em que o usuário informou o crédito usado:
  /// `(data, rendimento coberto)`.
  CompFund withCreditUsed(CompFund fund, List<(String, double)> marks) =>
      CompFund(
        assetId: fund.assetId,
        ticker: fund.ticker,
        term: fund.term,
        administrator: fund.administrator,
        redemptions: fund.redemptions,
        comeCotas: [for (final (date, used) in marks) ComeCotasMark(d(date), used)],
      );

  group('aviso de come-cotas registrado com saldo no grupo', () {
    test('um come-cotas depois da perda avisa com o saldo do grupo', () {
      // X perde 200 em 01/06/2025; o come-cotas registrado em 30/11/2025 foi feito
      // com 200 de saldo no grupo
      final result = compensate([
        withComeCotas(
          loser(1, 'XPTO11', FundTerm.longTerm, 'XP', 8),
          ['2025-11-30'],
        ),
      ]);

      expect(result.notes, hasLength(1));
      expect(result.notes.single.assetId, 1);
      expect(result.notes.single.date, d('2025-11-30'));
      expect(result.notes.single.balance, closeTo(200, 1e-6));
      // o aviso não muda nada: o saldo segue inteiro
      expect(sum(result.credits), closeTo(200, 1e-6));
    });

    test('antes da perda e no mesmo dia não avisa; no dia seguinte avisa', () {
      final none = compensate([
        withComeCotas(
          loser(1, 'XPTO11', FundTerm.longTerm, 'XP', 8),
          ['2025-05-01', '2025-06-01'],
        ),
      ]);
      expect(none.notes, isEmpty);

      final nextDay = compensate([
        withComeCotas(
          loser(1, 'XPTO11', FundTerm.longTerm, 'XP', 8),
          ['2025-06-02'],
        ),
      ]);
      expect(nextDay.notes.single.date, d('2025-06-02'));
    });

    test('depois de um resgate que gastou todo o crédito não avisa', () {
      // X perde 200 (01/06/2025) e Y gasta os 200 num ganho de 01/09/2025: o
      // come-cotas de X em 01/07/2025 ainda via 200 de saldo (aviso); o de Y em
      // 30/11/2025 já não vê nada
      final result = compensate([
        withComeCotas(
          loser(1, 'XPTO11', FundTerm.longTerm, 'XP', 8),
          ['2025-07-01'],
        ),
        withComeCotas(
          winner(2, 'YPTO11', FundTerm.longTerm, 'XP', '2025-09-01'),
          ['2025-11-30'],
        ),
      ]);

      expect(result.credits, isEmpty);
      expect(result.notes.single.assetId, 1);
      expect(result.notes.single.balance, closeTo(200, 1e-6));
    });

    test('mostra o saldo que sobrou depois de um uso parcial', () {
      // X perde 200; Y ganha só 100 (11,00 contra 10,00 em 243 dias: IR 20, usa 100
      // de crédito); sobram 100 para o come-cotas de 30/11/2025
      final result = compensate([
        withComeCotas(
          loser(1, 'XPTO11', FundTerm.longTerm, 'XP', 8),
          ['2025-11-30'],
        ),
        fixture(2, 'YPTO11', FundTerm.longTerm, 'XP', [
          buy(1, '2025-01-01', 100, 1000),
          redeem(2, '2025-09-01', 100, 11),
        ]),
      ]);

      expect(result.redemptions[1].single.lossCreditUsed, closeTo(100, 1e-6));
      expect(sum(result.credits), closeTo(100, 1e-6));
      expect(result.notes.single.balance, closeTo(100, 1e-6));
    });

    test('outros grupos não avisam; sem administradora o grupo é o próprio fundo', () {
      final other = compensate([
        loser(1, 'XPTO11', FundTerm.longTerm, 'XP', 8),
        withComeCotas(
          winner(2, 'BBB11', FundTerm.longTerm, 'BTG', '2025-09-01'),
          ['2025-11-30'],
        ),
        withComeCotas(
          winner(3, 'CCC11', FundTerm.shortTerm, 'XP', '2025-09-01'),
          ['2025-11-30'],
        ),
        withComeCotas(
          winner(4, 'DDD11', FundTerm.longTerm, null, '2025-09-01'),
          ['2025-11-30'],
        ),
      ]);
      expect(other.notes, isEmpty);

      final alone = compensate([
        withComeCotas(
          loser(5, 'EEE11', FundTerm.longTerm, null, 8),
          ['2025-11-30'],
        ),
      ]);
      expect(alone.notes.single.assetId, 5);
    });

    test('cada come-cotas tem o seu aviso, em ordem de data', () {
      final result = compensate([
        withComeCotas(
          loser(1, 'XPTO11', FundTerm.longTerm, 'XP', 8),
          ['2025-12-01', '2025-11-30'],
        ),
      ]);

      expect(result.notes.map((n) => n.date), [d('2025-11-30'), d('2025-12-01')]);
    });
  });

  // Crédito de prejuízo informado no come-cotas: porte dos testes de `ComeCotasMark`
  // do desktop.
  group('crédito de prejuízo informado no come-cotas', () {
    test('sai do saldo do grupo e o resgate posterior recebe menos crédito', () {
      // X perde 200 (01/06/2025). No come-cotas de 01/07/2025 o usuário informou 150
      // de rendimento coberto: o saldo cai para 50, sem aviso. O ganho de Y de
      // 01/09/2025 (200 a 20% = IR 40) só recebe os 50 restantes:
      // (200 − 50) × 20% = 30
      final result = compensate([
        withCreditUsed(
          loser(1, 'XPTO11', FundTerm.longTerm, 'XP', 8),
          [('2025-07-01', 150)],
        ),
        winner(2, 'YPTO11', FundTerm.longTerm, 'XP', '2025-09-01'),
      ]);

      expect(result.notes, isEmpty);
      final y = result.redemptions[1].single;
      expect(y.lossCreditUsed, closeTo(50, 1e-6));
      expect(y.ir, closeTo(30, 1e-6));
      expect(result.credits, isEmpty);
    });

    test('zero informado é uma resposta: sem aviso e o saldo fica inteiro', () {
      final result = compensate([
        withCreditUsed(
          loser(1, 'XPTO11', FundTerm.longTerm, 'XP', 8),
          [('2025-07-01', 0)],
        ),
        winner(2, 'YPTO11', FundTerm.longTerm, 'XP', '2025-09-01'),
      ]);

      expect(result.notes, isEmpty);
      expect(result.redemptions[1].single.ir, closeTo(0, 1e-6));
      expect(result.redemptions[1].single.lossCreditUsed, closeTo(200, 1e-6));
    });

    test('acima do saldo leva só o saldo e avisa com o valor informado', () {
      // saldo de 200 e o usuário informou 300: o saldo zera (não fica negativo), Y não
      // recebe crédito (IR 40) e o aviso traz o saldo de 200 e o valor informado
      final result = compensate([
        withCreditUsed(
          loser(1, 'XPTO11', FundTerm.longTerm, 'XP', 8),
          [('2025-07-01', 300)],
        ),
        winner(2, 'YPTO11', FundTerm.longTerm, 'XP', '2025-09-01'),
      ]);

      expect(result.notes, hasLength(1));
      expect(result.notes.single.balance, closeTo(200, 1e-6));
      expect(result.notes.single.informed, 300);
      expect(result.redemptions[1].single.ir, closeTo(40, 1e-6));
      expect(result.redemptions[1].single.lossCreditUsed, 0);
      expect(result.credits, isEmpty);

      // sem nenhum saldo (o come-cotas é de antes da perda), qualquer valor acima de
      // zero também avisa
      final early = compensate([
        withCreditUsed(
          loser(1, 'XPTO11', FundTerm.longTerm, 'XP', 8),
          [('2025-05-01', 100)],
        ),
      ]);
      expect(early.notes.single.balance, closeTo(0, 1e-9));
      expect(early.notes.single.informed, 100);
    });

    test('sem valor informado o aviso original continua, com informed nulo', () {
      final result = compensate([
        withComeCotas(
          loser(1, 'XPTO11', FundTerm.longTerm, 'XP', 8),
          ['2025-11-30'],
        ),
      ]);

      expect(result.notes.single.informed, isNull);
      expect(result.notes.single.balance, closeTo(200, 1e-6));
    });
  });
}
