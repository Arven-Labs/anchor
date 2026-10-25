import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/liability.dart';

void main() {
  const price = AmortizationSystem.price;
  const sac = AmortizationSystem.sac;

  List<Installment> schedule(
    double principal,
    double rate,
    int months,
    AmortizationSystem system,
  ) => computeInstallments(
    principal: principal,
    taxaMensal: rate,
    prazoMeses: months,
    sistema: system,
  );

  group('cronograma', () {
    test('recusa principal, prazo e taxa inválidos', () {
      expect(() => schedule(0, 0.01, 12, price), throwsArgumentError);
      expect(() => schedule(-100, 0.01, 12, price), throwsArgumentError);
      expect(() => schedule(double.nan, 0.01, 12, price), throwsArgumentError);
      expect(() => schedule(1000, -0.01, 12, price), throwsArgumentError);
      expect(() => schedule(1000, 0.01, 0, price), throwsArgumentError);
    });

    test('Price: parcela fixa e saldo final zero (conta à mão: 1000 a 10% em 2 meses)', () {
      final result = schedule(1000, 0.10, 2, price);

      // pmt = 1000 * 0,1 * 1,21 / 0,21 = 576,19; juros 100 e depois 52,38.
      expect(result[0].valorParcela, closeTo(576.19, 0.005));
      expect(result[0].valorJuros, closeTo(100, 1e-9));
      expect(result[0].valorAmortizacao, closeTo(476.19, 0.005));
      expect(result[1].valorParcela, closeTo(576.19, 0.005));
      expect(result[1].valorJuros, closeTo(52.38, 0.005));
      expect(result[1].saldoDevedorApos, 0);
    });

    test('SAC: amortização fixa e parcela decrescente (1200 a 1% em 12 meses)', () {
      final result = schedule(1200, 0.01, 12, sac);

      expect(result.first.valorAmortizacao, closeTo(100, 1e-9));
      expect(result.first.valorParcela, closeTo(112, 1e-9)); // 100 + 1% de 1200
      expect(result.last.valorParcela, closeTo(101, 1e-9)); // 100 + 1% de 100
      for (var k = 1; k < result.length; k++) {
        expect(result[k].valorParcela, lessThan(result[k - 1].valorParcela));
      }
      expect(result.last.saldoDevedorApos, 0);
    });

    test('a amortização soma o principal nos dois sistemas', () {
      for (final system in AmortizationSystem.values) {
        final result = schedule(10000, 0.015, 24, system);
        final total = result.fold<double>(0, (sum, i) => sum + i.valorAmortizacao);
        expect(total, closeTo(10000, 0.01));
      }
    });

    test('sem juros, Price divide o principal por igual', () {
      final result = schedule(1200, 0, 12, price);

      for (final installment in result) {
        expect(installment.valorJuros, 0);
        expect(installment.valorParcela, closeTo(100, 1e-9));
      }
    });
  });

  group('saldo devedor', () {
    test('só a amortização das parcelas já vencidas abate o principal', () {
      const installments = [
        (dataVencimento: '2026-02-01', valorAmortizacao: 100.0),
        (dataVencimento: '2026-03-01', valorAmortizacao: 100.0),
      ];

      expect(outstandingBalance(1200, installments, '2026-01-31'), 1200);
      expect(outstandingBalance(1200, installments, '2026-02-01'), 1100);
      expect(outstandingBalance(1200, installments, '2026-02-15'), 1100);
      expect(outstandingBalance(1200, installments, '2026-03-01'), 1000);
    });
  });

  group('addMonths', () {
    test('mantém o dia dentro do mês e vira o ano', () {
      expect(addMonths(DateTime(2026, 1, 15), 1), DateTime(2026, 2, 15));
      expect(addMonths(DateTime(2026, 1, 15), 12), DateTime(2027, 1, 15));
      expect(addMonths(DateTime(2026, 11, 15), 3), DateTime(2027, 2, 15));
    });

    test('recua para o fim do mês quando o dia não existe', () {
      expect(addMonths(DateTime(2026, 1, 31), 1), DateTime(2026, 2, 28));
      expect(addMonths(DateTime(2026, 11, 30), 3), DateTime(2027, 2, 28));
      expect(addMonths(DateTime(2028, 1, 31), 1), DateTime(2028, 2, 29)); // bissexto
    });
  });
}
