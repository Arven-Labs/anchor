import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/loan.dart';
import 'package:anchor_mobile/models/loan.dart';

void main() {
  group('meses completos', () {
    test('conta só meses fechados, pelo dia do mês', () {
      final inicio = DateTime(2026, 1, 15);

      expect(fullMonthsBetween(inicio, DateTime(2026, 1, 15)), 0);
      expect(fullMonthsBetween(inicio, DateTime(2026, 2, 14)), 0);
      expect(fullMonthsBetween(inicio, DateTime(2026, 2, 15)), 1);
      expect(fullMonthsBetween(inicio, DateTime(2027, 1, 14)), 11);
      expect(fullMonthsBetween(inicio, DateTime(2027, 1, 15)), 12);
    });

    test('antes do início é zero', () {
      expect(fullMonthsBetween(DateTime(2026, 5, 1), DateTime(2026, 1, 1)), 0);
    });
  });

  group('juros simples', () {
    double interest(double? rate, DateTime hoje) => accruedInterest(
      principal: 1000,
      taxaMensalPercentual: rate,
      dataInicio: DateTime(2026, 1, 10),
      hoje: hoje,
    );

    test('1000 a 1,5% a.m. por 4 meses completos dá 60 (sem composição)', () {
      expect(interest(1.5, DateTime(2026, 5, 10)), closeTo(60, 1e-9));
      expect(interest(1.5, DateTime(2026, 5, 9)), closeTo(45, 1e-9)); // 3 meses
    });

    test('sem taxa, taxa zero ou negativa não há juros', () {
      expect(interest(null, DateTime(2027, 1, 10)), 0);
      expect(interest(0, DateTime(2027, 1, 10)), 0);
      expect(interest(-1, DateTime(2027, 1, 10)), 0);
    });
  });

  group('saldo a receber', () {
    test('principal mais juros menos o baixado', () {
      expect(loanOutstandingBalance(1000, 60, [200, 100]), 760);
    });

    test('baixar mais do que o devido quita, sem saldo negativo', () {
      expect(loanOutstandingBalance(1000, 0, [800, 500]), 0);
    });

    test('extra recebido soma recebido menos baixado, positivo ou negativo', () {
      expect(
        extraReceived([
          (recebido: 110.0, baixado: 100.0), // um pouquinho a mais
          (recebido: 0.0, baixado: 40.0), // perdão
        ]),
        -30,
      );
      expect(extraReceived([]), 0);
    });
  });

  group('empréstimo com baixas', () {
    final loan = Loan(
      id: 1,
      borrowerId: 1,
      principal: 1000,
      taxaMensalPercentual: 1,
      dataInicio: '2026-01-10',
      includeInNetWorth: true,
      currency: 'BRL',
      createdAt: DateTime(2026, 1, 10),
    );
    final borrower = Borrower(id: 1, nome: 'Irmão', createdAt: DateTime(2026, 1, 1));

    LoanRepayment repayment(double received, double writtenOff) => LoanRepayment(
      loanId: 1,
      data: '2026-03-10',
      valorRecebido: received,
      valorBaixado: writtenOff,
      createdAt: DateTime(2026, 3, 10),
    );

    test('saldo, baixado e extra saem das baixas e da data de hoje', () {
      final item = LoanWithRepayments(loan, borrower, [
        repayment(300, 280),
        repayment(0, 20),
      ]);
      final hoje = DateTime(2026, 5, 10, 15, 30); // 4 meses, com hora do dia

      expect(item.interestOn(hoje), closeTo(40, 1e-9));
      expect(item.totalWrittenOff, 300);
      expect(item.extra, closeTo(0, 1e-9)); // +20 recebido a mais, −20 perdoado
      expect(item.balanceOn(hoje), closeTo(740, 1e-9)); // 1000 + 40 − 300
    });

    test('data de início corrompida vira juros zero em vez de erro', () {
      final broken = Loan(
        id: 2,
        borrowerId: 1,
        principal: 500,
        taxaMensalPercentual: 2,
        dataInicio: 'sem data',
        includeInNetWorth: false,
        currency: 'BRL',
        createdAt: DateTime(2026, 1, 10),
      );

      expect(
        LoanWithRepayments(broken, borrower, const []).balanceOn(DateTime(2026, 5, 10)),
        500,
      );
    });
  });
}
