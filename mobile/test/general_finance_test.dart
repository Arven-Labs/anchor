import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/currency_catalog.dart';
import 'package:anchor_mobile/domain/general_finance.dart';
import 'package:anchor_mobile/domain/money_format.dart';
import 'package:anchor_mobile/models/general_transaction.dart';

GeneralTransaction tx(
  int account,
  GeneralTransactionType type,
  double valor,
  String date,
) => GeneralTransaction(
  bankAccountId: account,
  type: type,
  valor: valor,
  date: date,
  createdAt: DateTime(2026, 1, 1),
);

void main() {
  const receita = GeneralTransactionType.receita;
  const despesa = GeneralTransactionType.despesa;

  group('saldo', () {
    test('despesa sai e receita entra', () {
      expect(signedValor(receita, 100), 100);
      expect(signedValor(despesa, 30), -30);
    });

    test('US\$ 100 recebidos e US\$ 30 gastos dão 70, por conta', () {
      final balances = computeBalances([
        tx(1, receita, 100, '2026-01-02'),
        tx(1, despesa, 30, '2026-01-05'),
        tx(2, receita, 500, '2026-01-03'),
      ], '2026-02-01');

      expect(balances[1], 70);
      expect(balances[2], 500);
    });

    test('lançamento depois de hoje não entra no saldo, o de hoje entra', () {
      final balances = computeBalances([
        tx(1, receita, 100, '2026-03-10'),
        tx(1, despesa, 40, '2026-03-11'),
      ], '2026-03-10');

      expect(balances[1], 100);
    });

    test('conta sem lançamento não aparece no mapa', () {
      expect(computeBalances(const [], '2026-01-01'), isEmpty);
    });
  });

  group('conversão para a moeda base', () {
    const rates = {'USD': 5.0, 'EUR': 6.0};

    test('US\$ 70 a 5,00 são R\$ 350', () {
      expect(convertToBase(70, 'USD', 'BRL', rates), 350);
    });

    test('mesma moeda devolve o próprio valor, sem precisar de taxa', () {
      expect(convertToBase(42.5, 'PYG', 'PYG', const {}), 42.5);
    });

    test('base em dólar inverte: R\$ 350 são US\$ 70, e € 10 são US\$ 12', () {
      expect(convertToBase(350, 'BRL', 'USD', rates), closeTo(70, 1e-9));
      expect(convertToBase(10, 'EUR', 'USD', rates), closeTo(12, 1e-9));
    });

    test('sem taxa de um dos lados, ou taxa inútil, devolve nulo', () {
      expect(convertToBase(10, 'PYG', 'BRL', rates), isNull);
      expect(convertToBase(10, 'USD', 'PYG', rates), isNull);
      expect(convertToBase(10, 'USD', 'BRL', const {'USD': 0.0}), isNull);
      expect(convertToBase(10, 'USD', 'BRL', const {'USD': -1.0}), isNull);
      expect(convertToBase(10, 'USD', 'BRL', {'USD': double.nan}), isNull);
    });
  });

  group('patrimônio', () {
    test(
      'soma caixa e carteira na base; item sem taxa ou sem preço fica fora e conta',
      () {
        final worth = computeNetWorth(
          base: 'BRL',
          cashItems: const [
            ValuedItem('BRL', 100), // 100
            ValuedItem('USD', 70), // 350
            ValuedItem('PYG', 1000000), // sem taxa: fora
          ],
          portfolioItems: const [
            ValuedItem('USD', 10), // 50
            ValuedItem('BRL', null), // sem preço: fora
          ],
          ratesToBrl: const {'USD': 5.0},
        );

        expect(worth.cash, 450);
        expect(worth.portfolio, 50);
        expect(worth.total, 500);
        expect(worth.itemsMissingFx, 2);
      },
    );

    test('a dívida abate o patrimônio na base; dívida sem câmbio fica fora e conta', () {
      final worth = computeNetWorth(
        base: 'BRL',
        cashItems: const [ValuedItem('BRL', 1000)],
        portfolioItems: const [ValuedItem('BRL', 500)],
        debtItems: const [
          ValuedItem('BRL', 300), // 300
          ValuedItem('USD', 20), // 100
          ValuedItem('PYG', 5000000), // sem taxa: fora
        ],
        ratesToBrl: const {'USD': 5.0},
      );

      expect(worth.debt, 400);
      expect(worth.total, 1100); // 1000 + 500 - 400
      expect(worth.itemsMissingFx, 1);
    });

    test('o saldo a receber dos empréstimos soma ao patrimônio, convertido para a base', () {
      final worth = computeNetWorth(
        base: 'BRL',
        cashItems: const [ValuedItem('BRL', 1000)],
        portfolioItems: const [],
        debtItems: const [ValuedItem('BRL', 300)],
        loanItems: const [
          ValuedItem('BRL', 200), // 200
          ValuedItem('USD', 10), // 50
          ValuedItem('PYG', 1000000), // sem taxa: fora
        ],
        ratesToBrl: const {'USD': 5.0},
      );

      expect(worth.loansReceivable, 250);
      expect(worth.total, 950); // 1000 + 250 - 300
      expect(worth.itemsMissingFx, 1);
    });

    test('empréstimo recebido e concedido movem a conta com o sinal certo', () {
      expect(signedValor(GeneralTransactionType.emprestimoConcedido, 100), -100);
      expect(signedValor(GeneralTransactionType.recebimentoEmprestimo, 100), 100);
    });

    test('sem dívida o patrimônio é o mesmo de antes', () {
      final worth = computeNetWorth(
        base: 'BRL',
        cashItems: const [ValuedItem('BRL', 100)],
        portfolioItems: const [],
        ratesToBrl: const {},
      );

      expect(worth.debt, 0);
      expect(worth.total, 100);
    });

    test('parcela de dívida sai da conta como despesa', () {
      expect(signedValor(GeneralTransactionType.parcelaDivida, 112), -112);
    });

    test('muda de base sem refazer as contas', () {
      final worth = computeNetWorth(
        base: 'USD',
        cashItems: const [ValuedItem('USD', 70), ValuedItem('BRL', 50)],
        portfolioItems: const [],
        ratesToBrl: const {'USD': 5.0},
      );

      expect(worth.cash, closeTo(80, 1e-9)); // 70 + 50 / 5
      expect(worth.base, 'USD');
      expect(worth.itemsMissingFx, 0);
    });
  });

  group('catálogo e formato', () {
    test(
      'o catálogo tem as 17 moedas do desktop e aceita espaço e minúscula',
      () {
        expect(currencyCatalog, hasLength(17));
        expect(validateCurrency(' eur '), 'EUR');
        expect(validateCurrency('XYZ'), isNull);
        expect(validateCurrency(''), isNull);
      },
    );

    test('formatMoney põe o código na frente, com milhar e vírgula', () {
      expect(formatMoney(1234.5, 'USD'), 'USD 1.234,50');
      expect(formatMoney(-1234567.891, 'BRL'), 'BRL -1.234.567,89');
      expect(formatMoney(0, 'PYG'), 'PYG 0,00');
      expect(formatMoney(999.999, 'EUR'), 'EUR 1.000,00');
    });

    test('formatMoney não mostra zero negativo', () {
      expect(formatMoney(-0.0, 'BRL'), 'BRL 0,00');
      expect(formatMoney(-0.004, 'BRL'), 'BRL 0,00');
    });
  });

  group('gasto da categoria no mês', () {
    GeneralTransaction spend(
      int account,
      double valor,
      String date, {
      int? category = 1,
      GeneralTransactionType type = despesa,
    }) => GeneralTransaction(
      bankAccountId: account,
      type: type,
      categoryId: category,
      valor: valor,
      date: date,
      createdAt: DateTime(2026, 1, 1),
    );

    ({double spent, int missingFx}) spentOf(
      List<GeneralTransaction> txs, {
      Map<String, double> rates = const {},
    }) => spentThisMonth(
      txs,
      categoryId: 1,
      monthPrefix: '2026-08',
      accountCurrency: {1: 'BRL', 2: 'USD'},
      base: 'BRL',
      ratesToBrl: rates,
    );

    test('soma só despesa da categoria e do mês', () {
      final result = spentOf([
        spend(1, 100, '2026-08-05'),
        spend(1, 50, '2026-08-20'),
        spend(1, 999, '2026-07-31'), // outro mês
        spend(1, 999, '2026-08-05', category: 2), // outra categoria
        spend(1, 999, '2026-08-05', category: null), // sem categoria
        spend(1, 999, '2026-08-05', type: receita), // receita
      ]);

      expect(result.spent, 150);
      expect(result.missingFx, 0);
    });

    test('converte a moeda da conta para a base', () {
      final result = spentOf([
        spend(1, 100, '2026-08-05'),
        spend(2, 10, '2026-08-06'),
      ], rates: {'USD': 5.0});

      expect(result.spent, 150);
    });

    test('despesa sem câmbio fica fora da soma e é contada', () {
      final result = spentOf([
        spend(1, 100, '2026-08-05'),
        spend(2, 10, '2026-08-06'),
      ]);

      expect(result.spent, 100);
      expect(result.missingFx, 1);
    });

    test('sem lançamento o gasto é zero', () {
      expect(spentOf([]).spent, 0);
    });
  });
}
