import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/come_cotas.dart';
import 'package:anchor_mobile/domain/darf_calendar.dart';
import 'package:anchor_mobile/models/fund.dart';

// Porta dos testes de `domain/tax/come_cotas.rs` do desktop; as contas à mão estão
// nos comentários.
void main() {
  DateTime d(String s) => DateTime.parse('${s}T00:00:00Z');

  QuotaValue quota(String date, double value) => QuotaValue(d(date), value);

  FundHolding fund(
    FundTerm? term,
    double quantity,
    double averageCost,
    List<QuotaValue> quotas, {
    String? firstBuy = '2025-01-10',
    List<QuotaValue> deductions = const [],
  }) => FundHolding(
    assetId: 1,
    ticker: 'FUNDO',
    name: 'Fundo Teste',
    term: term,
    quantity: quantity,
    averageCost: averageCost,
    firstBuy: firstBuy == null ? null : d(firstBuy),
    quotas: quotas,
    deductions: deductions,
  );

  ComeCotasEstimate run(FundHolding holding, String today) =>
      estimateComeCotas([holding], d(today)).first;

  test('as datas são o último dia útil de maio e de novembro', () {
    expect(lastBusinessDay(2026, 5), d('2026-05-29'), reason: '31/05/2026 é domingo');
    expect(lastBusinessDay(2026, 11), d('2026-11-30'));
    expect(lastBusinessDay(2025, 5), d('2025-05-30'), reason: '31/05/2025 é sábado');
    expect(lastBusinessDay(2025, 11), d('2025-11-28'), reason: '30/11/2025 é domingo');
  });

  test('a próxima data inclui hoje e passa para o ano seguinte', () {
    expect(nextReferenceDate(d('2026-01-10')), d('2026-05-29'));
    expect(nextReferenceDate(d('2026-05-29')), d('2026-05-29'));
    expect(nextReferenceDate(d('2026-05-30')), d('2026-11-30'));
    expect(nextReferenceDate(d('2026-12-01')), d('2027-05-31'));
  });

  test('a última data é estritamente antes de hoje', () {
    expect(lastReferenceDate(d('2026-10-22')), d('2026-05-29'));
    expect(lastReferenceDate(d('2026-01-10')), d('2025-11-28'));
    expect(lastReferenceDate(d('2026-05-29')), d('2025-11-28'));
    expect(lastReferenceDate(d('2026-05-30')), d('2026-05-29'));
  });

  test('longo prazo paga 15% do ganho sobre o custo médio', () {
    // 100 cotas, custo 10,00, cota a 12,00: ganho 200, imposto 30, 2,5 cotas
    final e = run(
      fund(FundTerm.longTerm, 100, 10, [quota('2026-01-05', 12)]),
      '2026-01-10',
    );
    expect(e.status, EstimateStatus.estimated);
    expect(e.gain, closeTo(200, 1e-6));
    expect(e.rate, closeTo(0.15, 1e-9));
    expect(e.tax, closeTo(30, 1e-6));
    expect(e.quotasDeducted, closeTo(2.5, 1e-6));
    expect(e.basisSource, BasisSource.cost);
    expect(e.referenceDate, d('2026-05-29'));
  });

  test('curto prazo paga 20%', () {
    final e = run(
      fund(FundTerm.shortTerm, 100, 10, [quota('2026-01-05', 12)]),
      '2026-01-10',
    );
    expect(e.rate, closeTo(0.20, 1e-9));
    expect(e.tax, closeTo(40, 1e-6));
  });

  test('a base sobe para o valor da cota do último come-cotas', () {
    // última data: 29/05/2026; cota de 28/05 a 11,00 (> custo 10,00); hoje 12,00
    final e = run(
      fund(FundTerm.longTerm, 100, 10, [
        quota('2026-05-28', 11),
        quota('2026-10-20', 12),
      ]),
      '2026-10-22',
    );
    expect(e.basisSource, BasisSource.lastComeCotas);
    expect(e.basisPerQuota, closeTo(11, 1e-9));
    expect(e.gain, closeTo(100, 1e-6));
    expect(e.tax, closeTo(15, 1e-6));
    expect(e.referenceDate, d('2026-11-30'));
  });

  test('posição comprada depois do último come-cotas mantém o custo como base', () {
    final e = run(
      fund(
        FundTerm.longTerm,
        100,
        10,
        [quota('2026-05-28', 11), quota('2026-10-20', 12)],
        firstBuy: '2026-07-01',
      ),
      '2026-10-22',
    );
    expect(e.basisSource, BasisSource.cost);
    expect(e.gain, closeTo(200, 1e-6));
  });

  test('uma cota velha demais não serve de base', () {
    // o único valor antes do último come-cotas é de mais de 45 dias antes dele
    final e = run(
      fund(FundTerm.longTerm, 100, 10, [
        quota('2026-03-01', 11),
        quota('2026-10-20', 12),
      ]),
      '2026-10-22',
    );
    expect(e.basisSource, BasisSource.cost);
  });

  test('cota abaixo da base não paga nada', () {
    final e = run(
      fund(FundTerm.longTerm, 100, 10, [quota('2026-01-05', 9)]),
      '2026-01-10',
    );
    expect(e.status, EstimateStatus.estimated);
    expect(e.gain, 0);
    expect(e.tax, 0);
    expect(e.quotasDeducted, 0);
  });

  test('fundo de ações é isento', () {
    final e = run(
      fund(FundTerm.equity, 100, 10, [quota('2026-01-05', 12)]),
      '2026-01-10',
    );
    expect(e.status, EstimateStatus.exempt);
    expect(e.tax, 0);
  });

  test('sem tipo, sem cota ou sem posição não há estimativa', () {
    expect(
      run(fund(null, 100, 10, [quota('2026-01-05', 12)]), '2026-01-10').status,
      EstimateStatus.noTerm,
    );
    expect(
      run(fund(FundTerm.longTerm, 100, 10, []), '2026-01-10').status,
      EstimateStatus.noQuota,
    );
    // valor de cota com data futura não conta
    expect(
      run(
        fund(FundTerm.longTerm, 100, 10, [quota('2026-02-01', 12)]),
        '2026-01-10',
      ).status,
      EstimateStatus.noQuota,
    );
    expect(
      run(
        fund(FundTerm.longTerm, 0, 10, [quota('2026-01-05', 12)]),
        '2026-01-10',
      ).status,
      EstimateStatus.noPosition,
    );
  });

  test('valor de cota com mais de 35 dias é sinalizado', () {
    expect(
      run(
        fund(FundTerm.longTerm, 100, 10, [quota('2025-12-01', 12)]),
        '2026-01-10',
      ).staleQuota,
      isTrue,
    );
    expect(
      run(
        fund(FundTerm.longTerm, 100, 10, [quota('2026-01-05', 12)]),
        '2026-01-10',
      ).staleQuota,
      isFalse,
    );
  });

  test('o desconto registrado dá o valor exato da cota do último come-cotas', () {
    // sem valor lançado perto de 29/05, mas com o desconto registrado a 11,00
    final e = run(
      fund(
        FundTerm.longTerm,
        100,
        10,
        [quota('2026-10-20', 12)],
        deductions: [quota('2026-05-29', 11)],
      ),
      '2026-10-22',
    );
    expect(e.basisSource, BasisSource.lastComeCotas);
    expect(e.basisPerQuota, closeTo(11, 1e-9));
    expect(e.gain, closeTo(100, 1e-6));
    expect(e.lastReferenceUnrecorded, isNull, reason: 'o desconto está registrado');
  });

  test('o desconto registrado vale mais que a tabela de cotas', () {
    // a tabela diz 10,50 perto da data, o desconto registrado diz 11,00
    final e = run(
      fund(
        FundTerm.longTerm,
        100,
        10,
        [quota('2026-05-28', 10.5), quota('2026-10-20', 12)],
        deductions: [quota('2026-05-29', 11)],
      ),
      '2026-10-22',
    );
    expect(e.basisPerQuota, closeTo(11, 1e-9));
  });

  test('come-cotas que passou sem registro é sinalizado', () {
    final e = run(
      fund(FundTerm.longTerm, 100, 10, [quota('2026-10-20', 12)]),
      '2026-10-22',
    );
    expect(e.lastReferenceUnrecorded, d('2026-05-29'));
    // um desconto registrado 30 dias depois da data não é o dela
    final late = run(
      fund(
        FundTerm.longTerm,
        100,
        10,
        [quota('2026-10-20', 12)],
        deductions: [quota('2026-06-28', 11)],
      ),
      '2026-10-22',
    );
    expect(late.lastReferenceUnrecorded, d('2026-05-29'));
  });

  test('sem aviso para fundo de ações, compra posterior ou sem posição', () {
    final equity = run(
      fund(FundTerm.equity, 100, 10, [quota('2026-10-20', 12)]),
      '2026-10-22',
    );
    expect(equity.lastReferenceUnrecorded, isNull);

    final later = run(
      fund(
        FundTerm.longTerm,
        100,
        10,
        [quota('2026-10-20', 12)],
        firstBuy: '2026-07-01',
      ),
      '2026-10-22',
    );
    expect(later.lastReferenceUnrecorded, isNull);

    final empty = run(
      fund(FundTerm.longTerm, 0, 10, [quota('2026-10-20', 12)]),
      '2026-10-22',
    );
    expect(empty.lastReferenceUnrecorded, isNull);
  });

  test('o tipo é lido do texto gravado', () {
    expect(FundTerm.parse('longo_prazo'), FundTerm.longTerm);
    expect(FundTerm.parse('curto_prazo'), FundTerm.shortTerm);
    expect(FundTerm.parse('acoes'), FundTerm.equity);
    expect(FundTerm.parse('outro'), isNull);
    expect(FundTerm.parse(null), isNull);
  });
}
