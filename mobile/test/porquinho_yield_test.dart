import 'dart:math' as math;

import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/finance_error.dart';
import 'package:anchor_mobile/domain/porquinho_yield.dart';

// Estimativa do rendimento de cada resgate do Porquinho. Porta de
// `desktop/src-tauri/src/domain/porquinho_yield.rs` (os mesmos 11 casos) mais o que o
// `estimate_in` de `commands/porquinho_estimate.rs` decide: dia já coberto e rendimento zero.

DateTime d(int y, int m, int day) => DateTime.utc(y, m, day);

/// Meta de 10,10% (CDI de 10,00% ao ano) desde antes de tudo.
List<SelicPoint> flatSelic() => [SelicPoint(d(2024, 1, 1), 10.10)];

Leg application(DateTime date, double amount) => Leg(date: date, kind: LegKind.application, amount: amount);
Leg redemption(DateTime date, double amount) => Leg(date: date, kind: LegKind.redemption, amount: amount);

double round2(double value) => (value * 100).roundToDouble() / 100;

List<RedemptionEstimate> estimate(List<Leg> legs, {double pct = 1.0, List<SelicPoint>? selic}) =>
    estimateRedemptions(legs: legs, selic: selic ?? flatSelic(), pctOfCdi: pct);

void main() {
  group('Selic meta em vigor', () {
    test('vale a última meta até o dia; antes da primeira, nenhuma', () {
      final selic = [SelicPoint(d(2025, 1, 1), 12.25), SelicPoint(d(2025, 3, 20), 14.25)];

      expect(metaOn(selic, d(2024, 12, 31)), isNull);
      expect(metaOn(selic, d(2025, 3, 19)), 12.25);
      expect(metaOn(selic, d(2025, 3, 20)), 14.25);
    });
  });

  group('fator de rendimento', () {
    test('um dia útil rende o CDI diário e a % do CDI escala o ganho', () {
      // Quarta 11/6/2025 → quinta 12/6: um dia útil. CDI de 10% ao ano.
      final one = accrualFactor(flatSelic(), d(2025, 6, 11), d(2025, 6, 12), 1.0)!;
      final half = accrualFactor(flatSelic(), d(2025, 6, 11), d(2025, 6, 12), 0.5)!;

      final daily = _pow(1.10, 1 / 252) - 1;
      expect(one, closeTo(1 + daily, 1e-12));
      expect(half, closeTo(1 + daily * 0.5, 1e-12));
    });

    test('fim de semana e feriado não rendem', () {
      // Sexta 13/6 → segunda 16/6: só o dia 16 rende.
      final weekend = accrualFactor(flatSelic(), d(2025, 6, 13), d(2025, 6, 16), 1.0)!;
      final oneDay = accrualFactor(flatSelic(), d(2025, 6, 15), d(2025, 6, 16), 1.0)!;
      // Corpus Christi (quinta 19/6/2025): quarta 18 → sexta 20 é um dia só.
      final holiday = accrualFactor(flatSelic(), d(2025, 6, 18), d(2025, 6, 20), 1.0)!;

      expect(weekend, closeTo(oneDay, 1e-12));
      expect(holiday, closeTo(oneDay, 1e-12));
    });

    test('31/12 rende (não é feriado nacional) e 20/11 só deixa de render a partir de 2024', () {
      final newYearsEve = accrualFactor(flatSelic(), d(2025, 12, 30), d(2025, 12, 31), 1.0)!;
      final oneDay = accrualFactor(flatSelic(), d(2025, 6, 11), d(2025, 6, 12), 1.0)!;
      // 20/11/2023 foi uma segunda-feira útil; 20/11/2024 (Consciência Negra) não rende.
      final before2024 = accrualFactor(flatSelic(), d(2023, 11, 19), d(2023, 11, 20), 1.0)!;
      final from2024 = accrualFactor(flatSelic(), d(2024, 11, 19), d(2024, 11, 20), 1.0)!;

      expect(newYearsEve, closeTo(oneDay, 1e-12));
      expect(before2024, closeTo(oneDay, 1e-12));
      expect(from2024, 1.0);
    });

    test('a mudança de meta no meio do trecho vale dia a dia', () {
      final selic = [SelicPoint(d(2024, 1, 1), 10.10), SelicPoint(d(2025, 6, 17), 20.10)];

      final f = accrualFactor(selic, d(2025, 6, 13), d(2025, 6, 18), 1.0)!;

      // 16/6 a 10% ao ano, 17/6 e 18/6 a 20%.
      final low = _pow(1.10, 1 / 252);
      final high = _pow(1.20, 1 / 252);
      expect(f, closeTo(low * high * high, 1e-12));
    });

    test('sem meta antes da série, não há fator', () {
      expect(accrualFactor([SelicPoint(d(2025, 6, 17), 10.10)], d(2025, 6, 11), d(2025, 6, 12), 1.0), isNull);
    });
  });

  group('estimativa de cada resgate', () {
    test('o valor pago fecha no centavo: principal + rendimento líquido; sem IOF acima de 30 dias, IR de 22,5%', () {
      final e = estimate([application(d(2025, 3, 3), 1000), redemption(d(2025, 6, 3), 250.37)]).single;

      expect(e.fullyCovered, isTrue);
      expect(e.principal + e.netYield, closeTo(250.37, 0.0051));
      expect(e.grossYield, greaterThan(0));
      expect(e.netYield, allOf(greaterThan(0), lessThan(e.grossYield)));
      expect(e.iof, 0);
      expect((e.ir - round2(e.grossYield * 0.225)).abs(), lessThanOrEqualTo(0.011));
    });

    test('resgate em até 30 dias paga IOF antes do IR', () {
      final e = estimate([application(d(2025, 6, 2), 10000), redemption(d(2025, 6, 9), 10000)]).single;

      // 7 dias aplicados: IOF de 76% do rendimento; o IR incide sobre os 24% que sobram.
      expect(e.holdingDays, 7);
      expect((e.iof - round2(e.grossYield * 0.76)).abs(), lessThanOrEqualTo(0.011));
      expect((e.ir - round2((e.grossYield - e.iof) * 0.225)).abs(), lessThanOrEqualTo(0.011));
    });

    test('o resgate consome a aplicação mais antiga primeiro (PEPS)', () {
      // Dois lotes; os resgates pequenos saem todos do mais antigo, que rende mais por real.
      final est = estimate([
        application(d(2025, 1, 6), 500),
        application(d(2025, 5, 5), 500),
        redemption(d(2025, 6, 2), 100),
        redemption(d(2025, 6, 2), 100),
      ]);

      expect(est[0].holdingDays, est[1].holdingDays);
      expect(est[0].holdingDays, d(2025, 6, 2).difference(d(2025, 1, 6)).inDays);
      expect(est[0].grossYield, est[1].grossYield);
    });

    test('resgate maior que as aplicações conhecidas é sinalizado e estima só a parte conhecida', () {
      final e = estimate([application(d(2025, 3, 3), 100), redemption(d(2025, 6, 3), 400)]).single;

      expect(e.fullyCovered, isFalse);
      // O rendimento é só o dos 100 aplicados (cerca de 2,4% em três meses a 10% ao ano, menos IR).
      expect(e.grossYield, allOf(greaterThan(1.5), lessThan(3.5)));
      expect(e.principal + e.netYield, closeTo(400, 0.0051));
    });

    test('resgate no mesmo dia da aplicação não rende', () {
      final e = estimate([application(d(2025, 6, 2), 50), redemption(d(2025, 6, 2), 50)]).single;

      expect((e.grossYield, e.netYield), (0.0, 0.0));
      expect(e.fullyCovered, isTrue);
    });

    test('a série da Selic que começa depois da aplicação é um erro, não uma estimativa com buraco', () {
      final legs = [application(d(2025, 3, 3), 100), redemption(d(2025, 6, 3), 101)];
      final late = [SelicPoint(d(2025, 5, 1), 10.10)];

      expect(
        () => estimate(legs, selic: late),
        throwsA(isA<FinanceArgumentError>().having((e) => e.code, 'code', FinanceErrorCode.selicNotCovered)),
      );
    });

    test('guarda a posição do resgate na lista de fluxos', () {
      final est = estimate([application(d(2025, 3, 3), 100), redemption(d(2025, 6, 3), 101), redemption(d(2025, 6, 4), 1)]);

      expect(est.map((e) => e.legIndex), [1, 2]);
    });
  });

  group('o que propor (dia coberto, rendimento zero, % do CDI)', () {
    final legs = [
      application(d(2025, 1, 2), 1000),
      redemption(d(2025, 2, 3), 500),
      redemption(d(2025, 3, 3), 500),
    ];

    YieldProposals propose({Set<String> covered = const {}, double pct = 1.0, List<Leg>? withLegs}) => proposeYields(
      legs: withLegs ?? legs,
      selic: flatSelic(),
      pctOfCdi: pct,
      coveredDays: covered,
    );

    test('sem nada coberto, cada resgate com rendimento vira uma proposta', () {
      final result = propose();

      expect(result.redemptions, 2);
      expect(result.proposals, hasLength(2));
      expect(result.skippedCoveredDay, 0);
      expect(result.skippedNoYield, 0);
    });

    test('resgate num dia que já tem rendimento lançado é pulado e contado', () {
      final result = propose(covered: {'2025-02-03'});

      expect(result.proposals.map((e) => e.date), [d(2025, 3, 3)]);
      expect(result.skippedCoveredDay, 1);
    });

    test('rendimento abaixo de R\$ 0,01 é pulado e contado', () {
      final same = [application(d(2025, 6, 2), 50), redemption(d(2025, 6, 2), 50)];

      final result = propose(withLegs: same);

      expect(result.proposals, isEmpty);
      expect(result.skippedNoYield, 1);
    });

    test('sem resgates não há nada a estimar e a Selic nem é consultada', () {
      final result = proposeYields(
        legs: [application(d(2025, 1, 2), 1000)],
        selic: const [],
        pctOfCdi: 1.0,
        coveredDays: const {},
      );

      expect(result.redemptions, 0);
      expect(result.proposals, isEmpty);
    });

    test('a % do CDI tem de estar entre 0 (exclusive) e 200%', () {
      for (final bad in [0.0, -0.5, 2.01, double.nan, double.infinity]) {
        expect(
          () => propose(pct: bad),
          throwsA(isA<FinanceArgumentError>().having((e) => e.code, 'code', FinanceErrorCode.pctOfCdiOutOfRange)),
        );
      }
      expect(propose(pct: 2.0).proposals, hasLength(2));
    });
  });
}

double _pow(double base, double exponent) => math.pow(base, exponent).toDouble();
