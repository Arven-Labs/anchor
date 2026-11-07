// Preço justo de uma ação por 8 modelos. Porta de `desktop/src-tauri/src/domain/{bazin,graham,
// gordon,dcf,banks,rim,rnav,projected_ceiling}.rs`: mesmas fórmulas, mesmas chaves de premissa
// (em snake_case, as mesmas do desktop) e as taxas como fração (0,06 = 6%).
//
// - Margem de segurança = (justo − cotação) / justo. Barata se a margem for > 0; margem 0 é Cara.
// - Diferença deliberada do desktop: preço justo não finito ou ≤ 0 é erro. O desktop não recusa e,
//   com justo negativo (ex.: fluxo de caixa negativo no DCF), a margem sai positiva e o veredito
//   "Barata". Aqui isso nunca chega à tela.
// - Anos (`fade_years`, `projection_years`) são inteiros: valor fracionário é recusado em vez de
//   truncado.
// - O que cada premissa significa (unidade, de onde vem) está em `valuation_fields.dart`.
//
// ATENÇÃO: o resultado é só a conta das premissas digitadas; não é recomendação de compra.

import 'dart:math' as math;

enum ValuationModel { bazin, graham, gordon, dcf, banks, rim, rnav, projectedCeiling }

extension ValuationModelMeta on ValuationModel {
  /// Identificador estável (o mesmo `model` do desktop), para guardar no banco.
  String get value => switch (this) {
    ValuationModel.bazin => 'bazin',
    ValuationModel.graham => 'graham',
    ValuationModel.gordon => 'gordon',
    ValuationModel.dcf => 'dcf',
    ValuationModel.banks => 'banks',
    ValuationModel.rim => 'rim',
    ValuationModel.rnav => 'rnav',
    ValuationModel.projectedCeiling => 'projected_ceiling',
  };
}

/// O modelo com este identificador (`ValuationModel.value`), ou null se não existir.
ValuationModel? valuationModelFromValue(String value) {
  for (final model in ValuationModel.values) {
    if (model.value == value) return model;
  }
  return null;
}

enum ValuationVerdict { cheap, expensive }

class ValuationOutcome {
  final double fairPrice;
  final double safetyMargin;
  final ValuationVerdict verdict;

  const ValuationOutcome({
    required this.fairPrice,
    required this.safetyMargin,
    required this.verdict,
  });
}

enum ValuationErrorCode {
  missingInput,
  inputNotFinite,
  priceMustBePositive,
  desiredYieldMustBePositive,
  epsAndBookValueMustBePositive,
  keMustExceedGrowth,
  waccMustExceedGrowth,
  sharesMustBePositive,
  fadeYearsMustBePositive,
  wholeNumberRequired,
  projectionYearsNegative,
  fairPriceMustBePositive,
}

/// Erro de valuation com código e argumentos (ADR 0001): o texto que o usuário lê vem dos
/// `.arb` pelo código; a mensagem aqui é inglês, para quem depura.
class ValuationException extends ArgumentError {
  final ValuationErrorCode code;
  final Map<String, String> args;

  ValuationException(this.code, [this.args = const {}])
    : super('Valuation error: ${code.name}${args.isEmpty ? '' : ' $args'}');
}

/// Calcula o preço justo de [model] a partir de [inputs] (chaves em `valuation_fields.dart`) e
/// da [currentPrice]. Lança [ValuationException] se faltar premissa, se algum valor não for
/// finito, se uma guarda do modelo falhar ou se o justo não for um número positivo.
ValuationOutcome calculateValuation(
  ValuationModel model,
  Map<String, double> inputs,
  double currentPrice,
) {
  if (!currentPrice.isFinite) {
    throw ValuationException(ValuationErrorCode.inputNotFinite, {'key': 'current_price'});
  }
  for (final entry in inputs.entries) {
    if (!entry.value.isFinite) {
      throw ValuationException(ValuationErrorCode.inputNotFinite, {'key': entry.key});
    }
  }
  if (currentPrice <= 0) {
    throw ValuationException(ValuationErrorCode.priceMustBePositive);
  }

  double value(String key) {
    final found = inputs[key];
    if (found == null) {
      throw ValuationException(ValuationErrorCode.missingInput, {'key': key});
    }
    return found;
  }

  int wholeYears(String key) {
    final years = value(key);
    if (years != years.truncateToDouble()) {
      throw ValuationException(ValuationErrorCode.wholeNumberRequired, {'key': key});
    }
    return years.toInt();
  }

  final fairPrice = switch (model) {
    ValuationModel.bazin => _bazin(value),
    ValuationModel.graham => _graham(value),
    ValuationModel.gordon => _gordon(value),
    ValuationModel.dcf => _dcf(value, currentPrice),
    ValuationModel.banks => _banks(value),
    ValuationModel.rim => _rim(value, wholeYears('fade_years')),
    ValuationModel.rnav => _rnav(value),
    ValuationModel.projectedCeiling => _projectedCeiling(value, wholeYears('projection_years')),
  };

  if (!fairPrice.isFinite || fairPrice <= 0) {
    throw ValuationException(ValuationErrorCode.fairPriceMustBePositive);
  }
  final safetyMargin = (fairPrice - currentPrice) / fairPrice;
  return ValuationOutcome(
    fairPrice: fairPrice,
    safetyMargin: safetyMargin,
    verdict: safetyMargin > 0 ? ValuationVerdict.cheap : ValuationVerdict.expensive,
  );
}

// Preço-teto = dividendo médio / yield desejado.
double _bazin(double Function(String) v) {
  final desiredYield = v('desired_yield');
  if (desiredYield <= 0) {
    throw ValuationException(ValuationErrorCode.desiredYieldMustBePositive);
  }
  return v('average_dividend') / desiredYield;
}

// Número de Graham = raiz(22,5 × LPA × VPA).
double _graham(double Function(String) v) {
  final eps = v('eps');
  final bookValue = v('book_value_per_share');
  if (eps <= 0 || bookValue <= 0) {
    throw ValuationException(ValuationErrorCode.epsAndBookValueMustBePositive);
  }
  return math.sqrt(22.5 * eps * bookValue);
}

// D1 = D0 × (1 + g); justo = D1 / (Ke − g).
double _gordon(double Function(String) v) {
  final growth = v('expected_growth');
  final ke = v('ke');
  if (ke <= growth) {
    throw ValuationException(ValuationErrorCode.keMustExceedGrowth);
  }
  return v('current_dividend') * (1 + growth) / (ke - growth);
}

// FCFF = EBIT × (1 − IR) + D&A − capex − ΔNWC; Ke = rf + beta × prêmio;
// E = cotação × ações; WACC = E/(E+D) × Ke + D/(E+D) × Kd × (1 − IR);
// valor da firma = FCFF × (1 + g) / (WACC − g); justo = (firma − dívida + caixa) / ações.
double _dcf(double Function(String) v, double currentPrice) {
  final shares = v('shares_outstanding');
  if (shares <= 0) {
    throw ValuationException(ValuationErrorCode.sharesMustBePositive);
  }
  final taxRate = v('tax_rate');
  final debt = v('total_debt');
  final growth = v('perpetuity_growth');
  final fcff =
      v('ebit') * (1 - taxRate) + v('depreciation_amortization') - v('capex') - v('nwc_change');
  final ke = v('risk_free_rate') + v('beta') * v('market_risk_premium');
  final equity = currentPrice * shares;
  final wacc =
      (equity / (equity + debt)) * ke + (debt / (equity + debt)) * v('kd') * (1 - taxRate);
  if (wacc - growth <= 0) {
    throw ValuationException(ValuationErrorCode.waccMustExceedGrowth);
  }
  final firmValue = fcff * (1 + growth) / (wacc - growth);
  return (firmValue - debt + v('cash')) / shares;
}

// g sustentável = ROE × (1 − payout); P/B justo = (ROE − g) / (Ke − g); justo = P/B × VPA.
double _banks(double Function(String) v) {
  final roe = v('roe');
  final ke = v('ke');
  final sustainableGrowth = roe * (1 - v('payout'));
  if (ke <= sustainableGrowth) {
    throw ValuationException(ValuationErrorCode.keMustExceedGrowth);
  }
  return (roe - sustainableGrowth) / (ke - sustainableGrowth) * v('book_value_per_share');
}

// ROE converge linearmente até o Ke em N anos; justo = VPA0 + soma do lucro residual
// descontado, sem valor terminal (em t = N o lucro residual já é zero).
double _rim(double Function(String) v, int years) {
  if (years < 1) {
    throw ValuationException(ValuationErrorCode.fadeYearsMustBePositive);
  }
  final ke = v('ke');
  final roeCurrent = v('roe_current');
  final payout = v('payout');
  final initialBookValue = v('book_value_per_share');
  var bookValue = initialBookValue;
  var stageValue = 0.0;
  for (var t = 1; t <= years; t++) {
    final roe = roeCurrent + (ke - roeCurrent) * (t / years);
    final residualIncome = (roe - ke) * bookValue;
    stageValue += residualIncome / math.pow(1 + ke, t);
    bookValue *= 1 + roe * (1 - payout);
  }
  return initialBookValue + stageValue;
}

// RNAV por ação = (landbank + estoque a valor de mercado + caixa líquido) / ações.
double _rnav(double Function(String) v) {
  final shares = v('shares_outstanding');
  if (shares <= 0) {
    throw ValuationException(ValuationErrorCode.sharesMustBePositive);
  }
  return (v('landbank') + v('inventory_at_market_value') + v('net_cash')) / shares;
}

// Dividendo projetado = D0 × (1 + g)^N; teto futuro = projetado / yield; justo = teto / (1 + Ke)^N.
double _projectedCeiling(double Function(String) v, int years) {
  final desiredYield = v('desired_yield');
  if (desiredYield <= 0) {
    throw ValuationException(ValuationErrorCode.desiredYieldMustBePositive);
  }
  if (years < 0) {
    throw ValuationException(ValuationErrorCode.projectionYearsNegative);
  }
  final projectedDividend = v('current_dividend') * math.pow(1 + v('expected_growth'), years);
  return projectedDividend / desiredYield / math.pow(1 + v('ke'), years);
}
