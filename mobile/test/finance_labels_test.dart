import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/currency_catalog.dart';
import 'package:anchor_mobile/domain/finance_error.dart';
import 'package:anchor_mobile/domain/generated_tx_info.dart';
import 'package:anchor_mobile/domain/liability.dart';
import 'package:anchor_mobile/l10n/app_localizations.dart';
import 'package:anchor_mobile/l10n/finance_labels.dart';
import 'package:anchor_mobile/models/general_transaction.dart';

// Textos de Finanças (ADR 0001): erro por código, tipo de lançamento e nome de moeda. O
// português é o texto que o app já mostrava.
Future<AppLocalizations> _load(String code) => AppLocalizations.delegate.load(Locale(code));

void main() {
  late AppLocalizations en;
  late AppLocalizations pt;

  setUpAll(() async {
    en = await _load('en');
    pt = await _load('pt');
  });

  group('erros por código', () {
    test('todo código tem texto nos dois idiomas, com os argumentos no lugar', () {
      for (final code in FinanceErrorCode.values) {
        for (final l in [en, pt]) {
          final text = financeErrorText(
            l,
            FinanceArgumentError(code, const {
              'currency': 'XYZ',
              'date': '2026-13-45',
              'loanCurrency': 'BRL',
              'accountCurrency': 'USD',
            }),
          );
          expect(text, isNotEmpty, reason: '$code ${l.localeName}');
          expect(text, isNot(contains('{')), reason: '$code ${l.localeName} placeholder sobrou');
        }
      }
    });

    test('pt: o texto de antes', () {
      String t(FinanceErrorCode c, [Map<String, String> a = const {}]) =>
          financeErrorText(pt, FinanceArgumentError(c, a));

      expect(t(FinanceErrorCode.unknownCurrency, {'currency': 'XYZ'}), 'Moeda fora do catálogo: XYZ');
      expect(t(FinanceErrorCode.amountMustBePositive), 'O valor deve ser maior que zero');
      expect(t(FinanceErrorCode.invalidDate, {'date': 'x'}), 'Data inválida: x');
      expect(t(FinanceErrorCode.accountNotFound), 'Conta não encontrada');
      expect(
        t(FinanceErrorCode.accountCurrencyLocked),
        'A moeda não muda numa conta que já tem lançamentos',
      );
      expect(
        t(FinanceErrorCode.borrowerHasLoans),
        'Este devedor ainda tem empréstimos: apague-os antes',
      );
      expect(
        t(FinanceErrorCode.loanCurrencyMismatch, {'loanCurrency': 'BRL', 'accountCurrency': 'USD'}),
        'O empréstimo é em BRL e a conta em USD',
      );
      expect(t(FinanceErrorCode.transferSameAccount), 'Escolha duas contas diferentes');
      expect(
        t(FinanceErrorCode.transferSameCurrencyAmountsDiffer),
        'Contas na mesma moeda recebem exatamente o que saiu',
      );
    });

    test('en: tradução fixada', () {
      String t(FinanceErrorCode c, [Map<String, String> a = const {}]) =>
          financeErrorText(en, FinanceStateError(c, a));

      expect(t(FinanceErrorCode.unknownCurrency, {'currency': 'XYZ'}), 'Currency not in the catalog: XYZ');
      expect(t(FinanceErrorCode.amountMustBePositive), 'The amount must be greater than zero');
      expect(t(FinanceErrorCode.invalidDate, {'date': 'x'}), 'Invalid date: x');
      expect(t(FinanceErrorCode.accountNotFound), 'Account not found');
      expect(
        t(FinanceErrorCode.accountCurrencyLocked),
        "The currency can't change on an account that already has transactions",
      );
      expect(
        t(FinanceErrorCode.borrowerHasLoans),
        'This borrower still has loans: delete them first',
      );
      expect(
        t(FinanceErrorCode.loanCurrencyMismatch, {'loanCurrency': 'BRL', 'accountCurrency': 'USD'}),
        'The loan is in BRL and the account in USD',
      );
      expect(
        t(FinanceErrorCode.linkedTransactionLocked),
        'A debt installment and a loan transaction are only deleted together with the debt or the loan',
      );
      expect(
        t(FinanceErrorCode.transferSameCurrencyAmountsDiffer),
        'Accounts in the same currency receive exactly what left',
      );
    });

    test('um erro que não é de Finanças mostra a própria mensagem, sem quebrar', () {
      for (final l in [en, pt]) {
        expect(financeErrorText(l, StateError('falha do banco')), 'falha do banco');
        expect(financeErrorText(l, ArgumentError('x')), 'Invalid argument(s): x');
        expect(financeErrorText(l, 'texto solto'), 'texto solto');
      }
    });
  });

  group('tipos de lançamento', () {
    test('pt: igual ao GeneralTransactionType.label de hoje, para todo tipo', () {
      for (final t in GeneralTransactionType.values) {
        expect(transactionTypeName(pt, t), t.label, reason: t.value);
      }
    });

    test('en: tradução fixada', () {
      expect({for (final t in GeneralTransactionType.values) t.value: transactionTypeName(en, t)}, {
        'receita': 'Income',
        'despesa': 'Expense',
        'transferencia_saida': 'Transfer (out)',
        'transferencia_entrada': 'Transfer (in)',
        'parcela_divida': 'Debt installment',
        'emprestimo_concedido': 'Loan granted',
        'recebimento_emprestimo': 'Loan repayment',
      });
    });
  });

  group('nomes de moeda', () {
    test('as 17 moedas do catálogo têm nome nos dois idiomas', () {
      expect(currencyCatalog, hasLength(17));
      for (final (code, _) in currencyCatalog) {
        expect(currencyName(en, code), isNotEmpty, reason: code);
        expect(currencyName(pt, code), isNotEmpty, reason: code);
        expect(currencyName(en, code), isNot(equals(code)), reason: code);
      }
    });

    test('pt: o nome que o catálogo já tinha', () {
      for (final (code, name) in currencyCatalog) {
        expect(currencyName(pt, code), name, reason: code);
      }
    });

    test('en: nomes fixados', () {
      expect({for (final (code, _) in currencyCatalog) code: currencyName(en, code)}, {
        'USD': 'US dollar',
        'EUR': 'Euro',
        'GBP': 'Pound sterling',
        'JPY': 'Yen',
        'CHF': 'Swiss franc',
        'CAD': 'Canadian dollar',
        'AUD': 'Australian dollar',
        'NZD': 'New Zealand dollar',
        'CNY': 'Yuan',
        'BRL': 'Brazilian real',
        'ARS': 'Argentine peso',
        'UYU': 'Uruguayan peso',
        'PYG': 'Guarani',
        'CLP': 'Chilean peso',
        'COP': 'Colombian peso',
        'PEN': 'Peruvian sol',
        'MXN': 'Mexican peso',
      });
    });

    test('moeda fora do catálogo mostra o código, em vez de quebrar', () {
      expect(currencyName(en, 'XYZ'), 'XYZ');
      expect(currencyName(pt, 'XYZ'), 'XYZ');
    });
  });

  group('descrição de lançamento gerado', () {
    const installment = GeneratedTxInfo(
      kind: GeneratedTxKind.installment,
      name: 'Carro',
      installmentNumber: 1,
      installmentTotal: 12,
    );
    const granted = GeneratedTxInfo(kind: GeneratedTxKind.loanGranted, name: 'Irmão');
    const repaid = GeneratedTxInfo(kind: GeneratedTxKind.loanRepayment, name: 'Irmão');

    test('pt: o mesmo texto que o repositório gravava antes', () {
      expect(generatedTxText(pt, installment), 'Carro — parcela 1/12');
      expect(generatedTxText(pt, granted), 'Empréstimo para Irmão');
      expect(generatedTxText(pt, repaid), 'Recebimento de Irmão');
    });

    test('en: o nome vem do usuário e não é traduzido', () {
      expect(generatedTxText(en, installment), 'Carro — installment 1/12');
      expect(generatedTxText(en, granted), 'Loan to Irmão');
      expect(generatedTxText(en, repaid), 'Repayment from Irmão');
    });

    test('parcela sem número ou total não imprime "null"', () {
      const broken = GeneratedTxInfo(kind: GeneratedTxKind.installment, name: 'Carro');
      for (final l in [en, pt]) {
        expect(generatedTxText(l, broken), isNot(contains('null')), reason: l.localeName);
        expect(generatedTxText(l, broken), contains('Carro'), reason: l.localeName);
      }
    });
  });

  group('sistema de amortização', () {
    test('pt: o rótulo de antes', () {
      expect(amortizationName(pt, AmortizationSystem.price), 'Price (parcela fixa)');
      expect(amortizationName(pt, AmortizationSystem.sac), 'SAC (amortização fixa)');
    });

    test('en', () {
      expect(amortizationName(en, AmortizationSystem.price), 'Price (fixed installment)');
      expect(amortizationName(en, AmortizationSystem.sac), 'SAC (fixed amortization)');
    });
  });
}
