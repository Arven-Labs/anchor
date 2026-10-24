import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/fund_redemption.dart';
import 'package:anchor_mobile/models/fund.dart';

// Porta dos testes de `domain/tax/fund_redemption.rs` do desktop; as contas à mão
// estão nos comentários.
void main() {
  DateTime d(String s) => DateTime.parse('${s}T00:00:00Z');

  FundEvent buy(int id, String date, double quantity, double cost) =>
      FundEvent.buy(date: d(date), id: id, quantity: quantity, cost: cost);

  FundEvent comeCotas(int id, String date, double quantity, double quotaValue) =>
      FundEvent.comeCotas(
        date: d(date),
        id: id,
        quantity: quantity,
        quotaValue: quotaValue,
      );

  FundEvent redeem(int id, String date, double quantity, double quotaValue) =>
      FundEvent.redeem(
        date: d(date),
        id: id,
        quantity: quantity,
        quotaValue: quotaValue,
      );

  List<RedemptionEstimate> run(List<FundEvent> events, FundTerm term) =>
      estimateRedemptions(events, term);

  /// Resgate total de 100 cotas compradas a 10,00 e resgatadas a 12,00 depois de
  /// [days] dias.
  RedemptionEstimate after(int days, FundTerm term) {
    final redeemed = d('2025-01-01').add(Duration(days: days));
    final date = redeemed.toIso8601String().substring(0, 10);
    return run([
      buy(1, '2025-01-01', 100, 1000),
      redeem(2, date, 100, 12),
    ], term).first;
  }

  test('longo prazo segue a tabela regressiva', () {
    // rendimento de 100 × (12 − 10) = 200; IOF zero acima de 30 dias
    expect(after(100, FundTerm.longTerm).ir, closeTo(45, 1e-6), reason: '22,5% até 180');
    expect(after(200, FundTerm.longTerm).ir, closeTo(40, 1e-6), reason: '20% de 181 a 360');
    expect(after(500, FundTerm.longTerm).ir, closeTo(35, 1e-6), reason: '17,5% de 361 a 720');
    expect(after(800, FundTerm.longTerm).ir, closeTo(30, 1e-6), reason: '15% acima de 720');
    expect(after(180, FundTerm.longTerm).lots.first.rate, closeTo(0.225, 1e-9));
    expect(after(181, FundTerm.longTerm).lots.first.rate, closeTo(0.20, 1e-9));
  });

  test('curto prazo tem duas faixas', () {
    expect(after(100, FundTerm.shortTerm).ir, closeTo(45, 1e-6));
    expect(after(500, FundTerm.shortTerm).ir, closeTo(40, 1e-6));
    expect(after(800, FundTerm.shortTerm).ir, closeTo(40, 1e-6));
  });

  test('o líquido é o bruto menos IOF e IR', () {
    final e = after(800, FundTerm.longTerm);
    expect(e.gross, closeTo(1200, 1e-6));
    expect(e.cost, closeTo(1000, 1e-6));
    expect(e.net, closeTo(1170, 1e-6));
  });

  test('resgate parcial consome o lote mais antigo primeiro', () {
    // lote A (01/01/2025, 100 a 10,00) e lote B (01/06/2025, 100 a 11,00); resgate
    // de 150 a 12,00 em 01/03/2026
    // A: 424 dias, 17,5% sobre 200 = 35; B: 50 cotas, 273 dias, 20% sobre 50 = 10
    final result = run([
      buy(1, '2025-01-01', 100, 1000),
      buy(2, '2025-06-01', 100, 1100),
      redeem(3, '2026-03-01', 150, 12),
      redeem(4, '2026-03-02', 50, 12),
    ], FundTerm.longTerm);
    final first = result[0];
    expect(first.lots, hasLength(2));
    expect(first.lots[0].quotas, closeTo(100, 1e-6));
    expect(first.lots[0].ir, closeTo(35, 1e-6));
    expect(first.lots[1].quotas, closeTo(50, 1e-6));
    expect(first.lots[1].ir, closeTo(10, 1e-6));
    expect(first.ir, closeTo(45, 1e-6));
    expect(first.cost, closeTo(1550, 1e-6));
    // o que sobrou é o resto do lote B (50 cotas a 11,00): 274 dias, 20% sobre 50
    final second = result[1];
    expect(second.lots, hasLength(1));
    expect(second.lots[0].lotDate, d('2025-06-01'));
    expect(second.lots[0].quotas, closeTo(50, 1e-6));
    expect(second.ir, closeTo(10, 1e-6));
  });

  test('fundo de ações paga 15% e não tem IOF', () {
    // 5 dias de aplicação: sem IOF, 15% sobre 200
    final e = run([
      buy(1, '2026-03-01', 100, 1000),
      redeem(2, '2026-03-06', 100, 12),
    ], FundTerm.equity).first;
    expect(e.iof, 0);
    expect(e.ir, closeTo(30, 1e-6));
  });

  test('o IOF sai do rendimento antes do IR', () {
    // 10 dias: IOF 66% de 200 = 132; IR 22,5% sobre 68 = 15,30; líquido 1.052,70
    final e = run([
      buy(1, '2026-03-01', 100, 1000),
      redeem(2, '2026-03-11', 100, 12),
    ], FundTerm.longTerm).first;
    expect(e.iof, closeTo(132, 1e-6));
    expect(e.ir, closeTo(15.3, 1e-6));
    expect(e.net, closeTo(1052.7, 1e-6));
  });

  test('resgate no dia da compra paga o IOF do 1º dia', () {
    final e = run([
      buy(1, '2026-03-01', 10, 100),
      redeem(2, '2026-03-01', 10, 12),
    ], FundTerm.shortTerm).first;
    expect(e.iof, closeTo(0.96 * 20, 1e-6));
  });

  test('o come-cotas já pago abate o IR do resgate', () {
    // 100 cotas a 10,00; come-cotas em 31/05/2024 de 2,5 cotas a 12,00 (imposto 30,
    // ganho já tributado 5); resgate de 97,5 cotas a 15,00 em 01/03/2026 (789 dias,
    // 15%): rendimento 487,50 + 5 = 492,50; IR 73,875 − 30 = 43,875;
    // líquido 1.462,50 − 43,875 = 1.418,625
    final e = run([
      buy(1, '2024-01-02', 100, 1000),
      comeCotas(2, '2024-05-31', 2.5, 12),
      redeem(3, '2026-03-01', 97.5, 15),
    ], FundTerm.longTerm).first;
    expect(e.gross, closeTo(1462.5, 1e-6));
    expect(e.cost, closeTo(975, 1e-6));
    expect(e.lots.first.gain, closeTo(492.5, 1e-6));
    expect(e.ir, closeTo(43.875, 1e-6));
    expect(e.comeCotasCredit, closeTo(30, 1e-6));
    expect(e.excessComeCotas, closeTo(0, 1e-6));
    expect(e.net, closeTo(1418.625, 1e-6));
  });

  test('come-cotas maior que o IR devido é excesso, não restituição', () {
    // mesmo come-cotas (30 pagos), mas o resgate sai a 9,00: rendimento
    // −97,50 + 5 = −92,50, IR zero, 30 de excesso
    final e = run([
      buy(1, '2024-01-02', 100, 1000),
      comeCotas(2, '2024-05-31', 2.5, 12),
      redeem(3, '2026-03-01', 97.5, 9),
    ], FundTerm.longTerm).first;
    expect(e.ir, 0);
    expect(e.comeCotasCredit, closeTo(0, 1e-6));
    expect(e.excessComeCotas, closeTo(30, 1e-6));
  });

  test('prejuízo não paga IR nem IOF', () {
    final e = run([
      buy(1, '2026-03-01', 100, 1000),
      redeem(2, '2026-03-05', 100, 9),
    ], FundTerm.longTerm).first;
    expect(e.ir, 0);
    expect(e.iof, 0);
    expect(e.net, closeTo(900, 1e-6));
  });

  test('resgate acima dos lotes conhecidos é sinalizado e tributa só o coberto', () {
    final e = run([
      buy(1, '2025-01-01', 10, 100),
      redeem(2, '2026-06-01', 15, 12),
    ], FundTerm.longTerm).first;
    expect(e.uncoveredQuotas, closeTo(5, 1e-6));
    // 10 cotas × (12 − 10) = 20 a 17,5% (516 dias)
    expect(e.ir, closeTo(3.5, 1e-6));
    expect(e.gross, closeTo(180, 1e-6));
  });

  test('os eventos podem vir em qualquer ordem', () {
    final events = [
      redeem(2, '2026-03-01', 100, 12),
      buy(1, '2025-01-01', 100, 1000),
    ];
    expect(run(events, FundTerm.longTerm).first.ir, closeTo(35, 1e-6));
  });

  // Fundos fechados e exclusivos (Lei 14.754): porta dos testes de amortização, entidade
  // de investimento e estoque do desktop.
  FundEvent amortize(int id, String date, double amount, double quotaValue) =>
      FundEvent.amortize(
        date: d(date),
        id: id,
        amount: amount,
        quotaValue: quotaValue,
      );

  FundEvent estoque(String date, double quotaValue, double rate) =>
      FundEvent.estoque(date: d(date), quotaValue: quotaValue, rate: rate);

  FundEstimate fund(List<FundEvent> events, FundTerm term) =>
      estimateFund(events, term);

  group('amortização', () {
    test('devolve capital, tributa a fração do ganho e mantém as cotas', () {
      // 100 cotas a 10,00 (custo 1.000); amortização de 600 a cota 12,00 em
      // 01/03/2026: posição 1.200, fração 0,5 (50 cotas equivalentes); 424 dias
      // (17,5%): rendimento 50 × (12 − 10) = 100, IR 17,50; custo baixado 500
      final events = [
        buy(1, '2025-01-01', 100, 1000),
        amortize(2, '2026-03-01', 600, 12),
      ];
      final a = fund(events, FundTerm.longTerm).redemptions[0];
      expect(a.isAmortization, isTrue);
      expect(a.quotas, closeTo(50, 1e-6));
      expect(a.gross, closeTo(600, 1e-6));
      expect(a.cost, closeTo(500, 1e-6));
      expect(a.iof, closeTo(0, 1e-6));
      expect(a.ir, closeTo(17.5, 1e-6));
      expect(a.net, closeTo(582.5, 1e-6));
      expect(a.lots[0].rate, closeTo(0.175, 1e-9));
      expect(a.lots[0].days, 424);

      // as 100 cotas ficam, com o custo pela metade (5,00 por cota): resgate de tudo
      // a 6,00 em 455 dias rende 100 × 1 = 100
      final more = [...events, redeem(3, '2026-04-01', 100, 6)];
      final r = fund(more, FundTerm.longTerm).redemptions[1];
      expect(r.isAmortization, isFalse);
      expect(r.quotas, closeTo(100, 1e-6));
      expect(r.cost, closeTo(500, 1e-6));
      expect(r.ir, closeTo(17.5, 1e-6));
      expect(r.uncoveredQuotas, closeTo(0, 1e-9));
    });

    test('também reduz o come-cotas já pago na mesma fração', () {
      // 100 cotas a 10,00; come-cotas de 2,5 cotas a 12,00 (ganho já tributado 5,
      // imposto pago 30), sobram 97,5. Amortização de 731,25 a 15,00 em
      // 01/03/2026: posição 97,5 × 15 = 1.462,50, fração 0,5 (48,75 cotas); 789 dias
      // (15%): rendimento 48,75 × 5 = 243,75 + 2,5 já tributado = 246,25; IR devido
      // 36,9375 − 15 já pago = 21,9375
      final events = [
        buy(1, '2024-01-02', 100, 1000),
        comeCotas(2, '2024-05-31', 2.5, 12),
        amortize(3, '2026-03-01', 731.25, 15),
      ];
      final a = fund(events, FundTerm.longTerm).redemptions[0];
      expect(a.quotas, closeTo(48.75, 1e-6));
      expect(a.cost, closeTo(487.5, 1e-6));
      expect(a.lots[0].gain, closeTo(246.25, 1e-6));
      expect(a.ir, closeTo(21.9375, 1e-6));
      expect(a.comeCotasCredit, closeTo(15, 1e-6));
      expect(a.excessComeCotas, closeTo(0, 1e-6));
      expect(a.net, closeTo(731.25 - 21.9375, 1e-6));
    });

    test('acima da posição é limitada e sem cotas é ignorada', () {
      // posição de 100 × 12 = 1.200; amortização de 5.000 vira a fração 1
      final capped = fund([
        buy(1, '2025-01-01', 100, 1000),
        amortize(2, '2026-03-01', 5000, 12),
      ], FundTerm.longTerm);
      expect(capped.redemptions[0].quotas, closeTo(100, 1e-6));
      expect(capped.redemptions[0].cost, closeTo(1000, 1e-6));
      // sem lote aberto não há o que amortizar
      expect(
        fund([
          amortize(1, '2026-03-01', 100, 12),
        ], FundTerm.longTerm).redemptions,
        isEmpty,
      );
    });
  });

  test(
    'entidade de investimento paga 15% fixo, sem IOF, em qualquer prazo',
    () {
      // 5 dias de aplicação: sem IOF e sem tabela por prazo, 15% sobre o rendimento de 200
      final e = run([
        buy(1, '2026-03-01', 100, 1000),
        redeem(2, '2026-03-06', 100, 12),
      ], FundTerm.investmentEntity).first;
      expect(e.iof, closeTo(0, 1e-6));
      expect(e.ir, closeTo(30, 1e-6));
      // a mesma coisa numa amortização (50 cotas equivalentes, rendimento 100)
      final a = fund([
        buy(1, '2026-03-01', 100, 1000),
        amortize(2, '2026-03-06', 600, 12),
      ], FundTerm.investmentEntity).redemptions[0];
      expect(a.iof, closeTo(0, 1e-6));
      expect(a.ir, closeTo(15, 1e-6));
    },
  );

  group('estoque de 2023', () {
    test('tributa o ganho em dinheiro e conta como imposto já pago', () {
      // lote 1: 100 cotas a 10,00 (01/06/2022); lote 2: 50 cotas a 20,00 (acima da
      // cota de 14,00: sem ganho); lote 3: 100 cotas a 10,00 em 01/02/2024 (depois da
      // data: fora). Cota de 31/12/2023 a 14,00: base = 100 × 4 = 400
      final events = [
        buy(1, '2022-06-01', 100, 1000),
        buy(2, '2023-01-01', 50, 1000),
        buy(3, '2024-02-01', 100, 1000),
        estoque('2023-12-31', 14, 0.15),
      ];
      final s = fund(events, FundTerm.longTerm).estoque!;
      expect(s.base, closeTo(400, 1e-6));
      expect(s.tax, closeTo(60, 1e-6));
      expect(s.rate, closeTo(0.15, 1e-9));
      expect(s.quotaValue, closeTo(14, 1e-9));

      // resgate das 100 cotas do lote 1 a 15,00 em 01/03/2026 (1.369 dias, 15%):
      // rendimento 500 + 400 já tributados = 900, IR devido 135, menos os 60 pagos
      // do estoque = 75
      final more = [...events, redeem(4, '2026-03-01', 100, 15)];
      final r = fund(more, FundTerm.longTerm).redemptions[0];
      expect(r.lots[0].gain, closeTo(900, 1e-6));
      expect(r.ir, closeTo(75, 1e-6));
      expect(r.comeCotasCredit, closeTo(60, 1e-6));
      expect(r.cost, closeTo(1000, 1e-6));
      expect(r.excessComeCotas, closeTo(0, 1e-6));
    });

    test('a opção de 8% paga menos agora e credita menos depois', () {
      final result = fund([
        buy(1, '2022-06-01', 100, 1000),
        estoque('2023-12-31', 14, 0.08),
        redeem(2, '2026-03-01', 100, 15),
      ], FundTerm.longTerm);
      // 8% de 400 = 32 pagos; IR devido 135 − 32 = 103
      expect(result.estoque!.tax, closeTo(32, 1e-6));
      expect(result.redemptions[0].ir, closeTo(103, 1e-6));
    });

    test(
      'sem lote aberto ou sem ganho a base é zero; sem o evento não há estoque',
      () {
        // cota de 31/12/2023 abaixo do custo: base zero, nada de crédito
        final low = fund([
          buy(1, '2022-06-01', 100, 1000),
          estoque('2023-12-31', 8, 0.15),
        ], FundTerm.longTerm);
        expect(low.estoque!.base, closeTo(0, 1e-9));
        // sem o evento os resgates seguem como antes
        final none = fund([
          buy(1, '2022-06-01', 100, 1000),
          redeem(2, '2026-03-01', 100, 15),
        ], FundTerm.longTerm);
        expect(none.estoque, isNull);
        expect(none.redemptions[0].ir, closeTo(75, 1e-6));
      },
    );
  });

  group('base com o estoque já tributado', () {
    test('soma o estoque ao custo e só cai com amortização', () {
      // 100 cotas a 10,00 e estoque a 14,00: o ganho tributado em 2023 é 4,00 por
      // cota, então o custo vira 14,00
      final events = [
        buy(1, '2022-06-01', 100, 1000),
        estoque('2023-12-31', 14, 0.15),
      ];
      expect(
        fund(events, FundTerm.longTerm).taxedBasisPerQuota,
        closeTo(14, 1e-9),
      );

      // come-cotas de 5 cotas e resgate de 45: as cotas saem com a sua parte, o valor
      // por cota não muda
      events
        ..add(comeCotas(2, '2024-05-31', 5, 15))
        ..add(redeem(3, '2024-08-01', 45, 15));
      expect(
        fund(events, FundTerm.longTerm).taxedBasisPerQuota,
        closeTo(14, 1e-9),
      );

      // amortização de 300 sobre 50 cotas a 15,00 (posição de 750): fração 0,4, e o
      // custo e o estoque caem 40%
      events.add(amortize(4, '2025-01-02', 300, 15));
      expect(
        fund(events, FundTerm.longTerm).taxedBasisPerQuota,
        closeTo(14 * 0.6, 1e-9),
      );
    });

    test('só existe com o evento do estoque e com cotas abertas', () {
      final buyOnly = [buy(1, '2022-06-01', 100, 1000)];
      expect(fund(buyOnly, FundTerm.longTerm).taxedBasisPerQuota, isNull);
      // cota de 2023 abaixo do custo: nada foi tributado, a base é o custo
      final low = [...buyOnly, estoque('2023-12-31', 8, 0.15)];
      expect(
        fund(low, FundTerm.longTerm).taxedBasisPerQuota,
        closeTo(10, 1e-9),
      );
      // sem cotas abertas depois do resgate total
      final sold = [
        ...buyOnly,
        estoque('2023-12-31', 14, 0.15),
        redeem(2, '2024-03-01', 100, 15),
      ];
      expect(fund(sold, FundTerm.longTerm).taxedBasisPerQuota, isNull);
    });
  });
}
