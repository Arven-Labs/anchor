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
}
