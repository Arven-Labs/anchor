// Premissas de cada modelo de valuation: a chave (a mesma do desktop e de `calculateValuation`) e
// como o campo é digitado e mostrado. O rótulo de cada campo vem dos `.arb`, pela chave.
// A tela de cálculo é gerada por esta lista, então um modelo novo só precisa de uma entrada aqui.

import 'stock_valuation.dart';

enum ValuationFieldFormat {
  /// Valor em reais (R$ por ação, ou valor da empresa nos modelos que usam o balanço).
  money,

  /// Digitado em % (6 = 6%) e dividido por 100 antes de calcular.
  percent,

  /// Número puro (beta, ações em milhões).
  number,

  /// Inteiro (anos).
  integer,
}

class ValuationField {
  final String key;
  final ValuationFieldFormat format;

  const ValuationField(this.key, this.format);
}

const _money = ValuationFieldFormat.money;
const _percent = ValuationFieldFormat.percent;
const _number = ValuationFieldFormat.number;
const _integer = ValuationFieldFormat.integer;

const valuationFields = <ValuationModel, List<ValuationField>>{
  ValuationModel.bazin: [
    ValuationField('average_dividend', _money),
    ValuationField('desired_yield', _percent),
  ],
  ValuationModel.graham: [
    ValuationField('eps', _money),
    ValuationField('book_value_per_share', _money),
  ],
  ValuationModel.gordon: [
    ValuationField('current_dividend', _money),
    ValuationField('expected_growth', _percent),
    ValuationField('ke', _percent),
  ],
  ValuationModel.dcf: [
    ValuationField('ebit', _money),
    ValuationField('tax_rate', _percent),
    ValuationField('depreciation_amortization', _money),
    ValuationField('capex', _money),
    ValuationField('nwc_change', _money),
    ValuationField('total_debt', _money),
    ValuationField('cash', _money),
    ValuationField('shares_outstanding', _number),
    ValuationField('beta', _number),
    ValuationField('risk_free_rate', _percent),
    ValuationField('market_risk_premium', _percent),
    ValuationField('kd', _percent),
    ValuationField('perpetuity_growth', _percent),
  ],
  ValuationModel.banks: [
    ValuationField('book_value_per_share', _money),
    ValuationField('roe', _percent),
    ValuationField('payout', _percent),
    ValuationField('ke', _percent),
  ],
  ValuationModel.rim: [
    ValuationField('book_value_per_share', _money),
    ValuationField('roe_current', _percent),
    ValuationField('payout', _percent),
    ValuationField('ke', _percent),
    ValuationField('fade_years', _integer),
  ],
  ValuationModel.rnav: [
    ValuationField('landbank', _money),
    ValuationField('inventory_at_market_value', _money),
    ValuationField('net_cash', _money),
    ValuationField('shares_outstanding', _number),
  ],
  ValuationModel.projectedCeiling: [
    ValuationField('current_dividend', _money),
    ValuationField('expected_growth', _percent),
    ValuationField('projection_years', _integer),
    ValuationField('desired_yield', _percent),
    ValuationField('ke', _percent),
  ],
};
