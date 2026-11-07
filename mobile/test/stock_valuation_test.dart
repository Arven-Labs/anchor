import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/stock_valuation.dart';
import 'package:anchor_mobile/domain/valuation_fields.dart';

// Casos portados de `desktop/src-tauri/src/domain/{bazin,graham,gordon,dcf,banks,rim,rnav,
// projected_ceiling}.rs` (mesmos números). Dados fictícios. As taxas são frações (0,06 = 6%).
// Diferença deliberada: o desktop não recusa preço justo <= 0 (o veredito sairia "Barata" com justo
// negativo); aqui isso é erro (`fairPriceMustBePositive`).
void main() {
  Matcher near(double expected) => closeTo(expected, 1e-9);

  Matcher refuses(ValuationErrorCode code) =>
      throwsA(isA<ValuationException>().having((e) => e.code, 'code', code));

  ValuationOutcome calc(ValuationModel model, Map<String, double> inputs, double price) =>
      calculateValuation(model, inputs, price);

  group('Bazin', () {
    const inputs = {'average_dividend': 3.0, 'desired_yield': 0.06};

    test('dividendo 3,00 e yield 6%: justo 50,00; cotação 40 dá margem 20% e Barata', () {
      final outcome = calc(ValuationModel.bazin, inputs, 40);
      expect(outcome.fairPrice, near(50));
      expect(outcome.safetyMargin, near(0.2));
      expect(outcome.verdict, ValuationVerdict.cheap);
    });

    test('cotação 60: margem negativa e Cara', () {
      final outcome = calc(ValuationModel.bazin, inputs, 60);
      expect(outcome.safetyMargin, lessThan(0));
      expect(outcome.verdict, ValuationVerdict.expensive);
    });

    test('cotação igual ao justo: margem 0 é Cara (não Barata)', () {
      final outcome = calc(ValuationModel.bazin, inputs, 50);
      expect(outcome.safetyMargin, near(0));
      expect(outcome.verdict, ValuationVerdict.expensive);
    });

    test('yield 0 é recusado', () {
      expect(
        () => calc(ValuationModel.bazin, {...inputs, 'desired_yield': 0}, 40),
        refuses(ValuationErrorCode.desiredYieldMustBePositive),
      );
    });
  });

  group('Graham', () {
    const inputs = {'eps': 2.0, 'book_value_per_share': 20.0};

    test('LPA 2 e VPA 20: justo 30,00 (raiz de 900); cotação 20 Barata, 40 Cara', () {
      final cheap = calc(ValuationModel.graham, inputs, 20);
      expect(cheap.fairPrice, near(30));
      expect(cheap.verdict, ValuationVerdict.cheap);
      expect(calc(ValuationModel.graham, inputs, 40).verdict, ValuationVerdict.expensive);
    });

    test('LPA 0 ou VPA negativo é recusado', () {
      expect(
        () => calc(ValuationModel.graham, {...inputs, 'eps': 0}, 20),
        refuses(ValuationErrorCode.epsAndBookValueMustBePositive),
      );
      expect(
        () => calc(ValuationModel.graham, {...inputs, 'book_value_per_share': -1}, 20),
        refuses(ValuationErrorCode.epsAndBookValueMustBePositive),
      );
    });
  });

  group('Gordon', () {
    const inputs = {'current_dividend': 2.0, 'expected_growth': 0.03, 'ke': 0.10};

    test('D0 2,00, g 3%, Ke 10%: justo 29,4285…; cotação 25 Barata, 40 Cara', () {
      final cheap = calc(ValuationModel.gordon, inputs, 25);
      expect(cheap.fairPrice, near(29.428571428571427));
      expect(cheap.verdict, ValuationVerdict.cheap);
      expect(calc(ValuationModel.gordon, inputs, 40).verdict, ValuationVerdict.expensive);
    });

    test('Ke igual ou menor que o crescimento é recusado', () {
      expect(
        () => calc(ValuationModel.gordon, {...inputs, 'ke': 0.03}, 25),
        refuses(ValuationErrorCode.keMustExceedGrowth),
      );
      expect(
        () => calc(ValuationModel.gordon, {...inputs, 'ke': 0.05, 'expected_growth': 0.08}, 25),
        refuses(ValuationErrorCode.keMustExceedGrowth),
      );
    });
  });

  group('DCF', () {
    const inputs = {
      'ebit': 1000.0,
      'tax_rate': 0.34,
      'depreciation_amortization': 200.0,
      'capex': 300.0,
      'nwc_change': 50.0,
      'total_debt': 2000.0,
      'cash': 500.0,
      'shares_outstanding': 100.0,
      'beta': 1.0,
      'risk_free_rate': 0.06,
      'market_risk_premium': 0.05,
      'kd': 0.08,
      'perpetuity_growth': 0.03,
    };

    // FCFF 510; Ke 11%; com cotação 10 o WACC é 7,1867% e o justo 110,4697…;
    // com cotação 200 o WACC sobe e o justo cai para 55,2272…. O Rust só confere o sinal da margem.
    test('cotação 10: justo 110,4697 (calculado à mão) e Barata', () {
      final outcome = calc(ValuationModel.dcf, inputs, 10);
      expect(outcome.fairPrice, closeTo(110.4697452229299, 1e-6));
      expect(outcome.verdict, ValuationVerdict.cheap);
    });

    test('cotação 200: o WACC depende da cotação, justo 55,2272 e Cara', () {
      final outcome = calc(ValuationModel.dcf, inputs, 200);
      expect(outcome.fairPrice, closeTo(55.22727272727271, 1e-6));
      expect(outcome.verdict, ValuationVerdict.expensive);
    });

    test('WACC menor ou igual ao crescimento é recusado', () {
      expect(
        () => calc(ValuationModel.dcf, {...inputs, 'beta': -10, 'perpetuity_growth': 0.05}, 10),
        refuses(ValuationErrorCode.waccMustExceedGrowth),
      );
    });

    test('zero ações é recusado', () {
      expect(
        () => calc(ValuationModel.dcf, {...inputs, 'shares_outstanding': 0}, 10),
        refuses(ValuationErrorCode.sharesMustBePositive),
      );
    });

    test('fluxo de caixa negativo dá justo negativo: recusado, nunca Barata', () {
      // No desktop: justo −214,28 e margem +104,7%, ou seja, Barata. Aqui é erro.
      expect(
        () => calc(ValuationModel.dcf, {...inputs, 'ebit': -1000}, 10),
        refuses(ValuationErrorCode.fairPriceMustBePositive),
      );
    });
  });

  group('Banks', () {
    const inputs = {
      'book_value_per_share': 20.0,
      'roe': 0.18,
      'payout': 0.5,
      'ke': 0.12,
    };

    test('VPA 20, ROE 18%, payout 50%, Ke 12%: g 9%, P/B 3, justo 60,00', () {
      final cheap = calc(ValuationModel.banks, inputs, 25);
      expect(cheap.fairPrice, near(60));
      expect(cheap.verdict, ValuationVerdict.cheap);
      expect(calc(ValuationModel.banks, inputs, 90).verdict, ValuationVerdict.expensive);
    });

    test('Ke igual ao crescimento sustentável é recusado', () {
      expect(
        () => calc(ValuationModel.banks, {...inputs, 'ke': 0.09}, 25),
        refuses(ValuationErrorCode.keMustExceedGrowth),
      );
    });
  });

  group('RIM', () {
    const inputs = {
      'book_value_per_share': 20.0,
      'roe_current': 0.18,
      'payout': 0.4,
      'ke': 0.12,
      'fade_years': 5.0,
    };

    test('base: justo 22,0997 (à mão); cotação 20 Barata, 200 Cara', () {
      final cheap = calc(ValuationModel.rim, inputs, 20);
      expect(cheap.fairPrice, closeTo(22.099753597667636, 1e-9));
      expect(cheap.verdict, ValuationVerdict.cheap);
      expect(calc(ValuationModel.rim, inputs, 200).verdict, ValuationVerdict.expensive);
    });

    test('ROE igual ao Ke: não há lucro residual e o justo é o valor patrimonial', () {
      final outcome = calc(
        ValuationModel.rim,
        {
          'book_value_per_share': 15.0,
          'roe_current': 0.12,
          'payout': 0.5,
          'ke': 0.12,
          'fade_years': 7.0,
        },
        20,
      );
      expect(outcome.fairPrice, near(15));
    });

    test('0 anos de convergência é recusado', () {
      expect(
        () => calc(ValuationModel.rim, {...inputs, 'fade_years': 0}, 20),
        refuses(ValuationErrorCode.fadeYearsMustBePositive),
      );
    });
  });

  group('RNAV', () {
    const inputs = {
      'landbank': 500.0,
      'inventory_at_market_value': 300.0,
      'net_cash': 200.0,
      'shares_outstanding': 100.0,
    };

    test('(500 + 300 + 200) / 100 = 10,00; cotação 8 Barata, 15 Cara', () {
      final cheap = calc(ValuationModel.rnav, inputs, 8);
      expect(cheap.fairPrice, near(10));
      expect(cheap.verdict, ValuationVerdict.cheap);
      expect(calc(ValuationModel.rnav, inputs, 15).verdict, ValuationVerdict.expensive);
    });

    test('caixa líquido negativo reduz o justo: −400 dá 4,00', () {
      expect(calc(ValuationModel.rnav, {...inputs, 'net_cash': -400}, 3).fairPrice, near(4));
    });

    test('zero ações é recusado', () {
      expect(
        () => calc(ValuationModel.rnav, {...inputs, 'shares_outstanding': 0}, 8),
        refuses(ValuationErrorCode.sharesMustBePositive),
      );
    });
  });

  group('Teto Projetado', () {
    const inputs = {
      'current_dividend': 3.0,
      'expected_growth': 0.05,
      'projection_years': 5.0,
      'desired_yield': 0.06,
      'ke': 0.10,
    };

    test('D0 3, g 5%, 5 anos, yield 6%, Ke 10%: justo 39,6235; cotação 30 Barata, 100 Cara', () {
      final cheap = calc(ValuationModel.projectedCeiling, inputs, 30);
      expect(cheap.fairPrice, closeTo(39.623521819175295, 1e-9));
      expect(cheap.verdict, ValuationVerdict.cheap);
      expect(
        calc(ValuationModel.projectedCeiling, inputs, 100).verdict,
        ValuationVerdict.expensive,
      );
    });

    test('0 anos vira o Bazin: 3 / 6% = 50,00', () {
      final outcome = calc(
        ValuationModel.projectedCeiling,
        {...inputs, 'projection_years': 0},
        40,
      );
      expect(outcome.fairPrice, near(50));
    });

    test('yield 0 é recusado', () {
      expect(
        () => calc(ValuationModel.projectedCeiling, {...inputs, 'desired_yield': 0}, 30),
        refuses(ValuationErrorCode.desiredYieldMustBePositive),
      );
    });
  });

  group('entradas inválidas', () {
    test('premissa que falta é recusada, com o nome dela', () {
      expect(
        () => calc(ValuationModel.bazin, {'average_dividend': 3.0}, 40),
        throwsA(
          isA<ValuationException>()
              .having((e) => e.code, 'code', ValuationErrorCode.missingInput)
              .having((e) => e.args['key'], 'key', 'desired_yield'),
        ),
      );
    });

    test('premissa ou cotação não finita (NaN, infinito) é recusada', () {
      const bazin = {'average_dividend': 3.0, 'desired_yield': 0.06};
      expect(
        () => calc(ValuationModel.bazin, {...bazin, 'average_dividend': double.nan}, 40),
        refuses(ValuationErrorCode.inputNotFinite),
      );
      expect(
        () => calc(ValuationModel.bazin, bazin, double.infinity),
        refuses(ValuationErrorCode.inputNotFinite),
      );
    });

    test('cotação zero ou negativa é recusada', () {
      const bazin = {'average_dividend': 3.0, 'desired_yield': 0.06};
      expect(
        () => calc(ValuationModel.bazin, bazin, 0),
        refuses(ValuationErrorCode.priceMustBePositive),
      );
      expect(
        () => calc(ValuationModel.bazin, bazin, -5),
        refuses(ValuationErrorCode.priceMustBePositive),
      );
    });

    test('anos que não são número inteiro, ou negativos, são recusados', () {
      const rim = {
        'book_value_per_share': 20.0,
        'roe_current': 0.18,
        'payout': 0.4,
        'ke': 0.12,
        'fade_years': 2.5,
      };
      expect(
        () => calc(ValuationModel.rim, rim, 20),
        refuses(ValuationErrorCode.wholeNumberRequired),
      );
      expect(
        () => calc(
          ValuationModel.projectedCeiling,
          {
            'current_dividend': 3.0,
            'expected_growth': 0.05,
            'projection_years': -1.0,
            'desired_yield': 0.06,
            'ke': 0.10,
          },
          30,
        ),
        refuses(ValuationErrorCode.projectionYearsNegative),
      );
    });

    test('dividendo negativo dá justo negativo: recusado, nunca Barata', () {
      expect(
        () => calc(
          ValuationModel.bazin,
          {'average_dividend': -3.0, 'desired_yield': 0.06},
          40,
        ),
        refuses(ValuationErrorCode.fairPriceMustBePositive),
      );
    });
  });

  group('campos de cada modelo', () {
    test('a lista de campos bate com o que o cálculo exige, nos 8 modelos', () {
      for (final model in ValuationModel.values) {
        final inputs = {for (final field in valuationFields[model]!) field.key: 0.5};
        // Se faltasse um campo, o cálculo acusaria `missingInput`; qualquer outro erro
        // (valores 0,5 podem violar uma guarda) significa que todas as chaves foram lidas.
        try {
          calculateValuation(model, inputs, 10);
        } on ValuationException catch (e) {
          expect(e.code, isNot(ValuationErrorCode.missingInput), reason: model.name);
        }
      }
    });

    test('nenhum campo a mais: sobrar uma chave também é erro de especificação', () {
      expect(valuationFields.keys.toSet(), ValuationModel.values.toSet());
      expect(valuationFields[ValuationModel.bazin]!.map((f) => f.key), [
        'average_dividend',
        'desired_yield',
      ]);
      expect(valuationFields[ValuationModel.dcf]!.length, 13);
    });
  });
}
