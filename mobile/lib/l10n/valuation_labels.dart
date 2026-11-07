import '../domain/stock_valuation.dart';
import 'app_localizations.dart';

// Textos do valuation de ações (ADR 0001). Um `switch` sem `default` nos enums faz o
// analisador acusar um valor novo sem texto; os campos são identificados por chave de texto,
// então o teste confere que todo campo de `valuation_fields.dart` tem rótulo.

String valuationModelName(AppLocalizations l, ValuationModel model) => switch (model) {
  ValuationModel.bazin => l.valuationModelBazin,
  ValuationModel.graham => l.valuationModelGraham,
  ValuationModel.gordon => l.valuationModelGordon,
  ValuationModel.dcf => l.valuationModelDcf,
  ValuationModel.banks => l.valuationModelBanks,
  ValuationModel.rim => l.valuationModelRim,
  ValuationModel.rnav => l.valuationModelRnav,
  ValuationModel.projectedCeiling => l.valuationModelProjectedCeiling,
};

/// Rótulo da premissa de chave [key]; uma chave sem rótulo volta como ela mesma (o teste
/// garante que nenhum campo real cai nisso).
String valuationFieldLabel(AppLocalizations l, String key) => switch (key) {
  'average_dividend' => l.valuationFieldAverageDividend,
  'desired_yield' => l.valuationFieldDesiredYield,
  'eps' => l.valuationFieldEps,
  'book_value_per_share' => l.valuationFieldBookValuePerShare,
  'current_dividend' => l.valuationFieldCurrentDividend,
  'expected_growth' => l.valuationFieldExpectedGrowth,
  'ke' => l.valuationFieldKe,
  'ebit' => l.valuationFieldEbit,
  'tax_rate' => l.valuationFieldTaxRate,
  'depreciation_amortization' => l.valuationFieldDepreciationAmortization,
  'capex' => l.valuationFieldCapex,
  'nwc_change' => l.valuationFieldNwcChange,
  'total_debt' => l.valuationFieldTotalDebt,
  'cash' => l.valuationFieldCash,
  'shares_outstanding' => l.valuationFieldSharesOutstanding,
  'beta' => l.valuationFieldBeta,
  'risk_free_rate' => l.valuationFieldRiskFreeRate,
  'market_risk_premium' => l.valuationFieldMarketRiskPremium,
  'kd' => l.valuationFieldKd,
  'perpetuity_growth' => l.valuationFieldPerpetuityGrowth,
  'roe' => l.valuationFieldRoe,
  'payout' => l.valuationFieldPayout,
  'roe_current' => l.valuationFieldRoeCurrent,
  'fade_years' => l.valuationFieldFadeYears,
  'landbank' => l.valuationFieldLandbank,
  'inventory_at_market_value' => l.valuationFieldInventoryAtMarketValue,
  'net_cash' => l.valuationFieldNetCash,
  'projection_years' => l.valuationFieldProjectionYears,
  _ => key,
};

String valuationErrorText(AppLocalizations l, ValuationException error) {
  final field = valuationFieldLabel(l, error.args['key'] ?? '');
  return switch (error.code) {
    ValuationErrorCode.missingInput => l.valuationErrMissingInput(field),
    ValuationErrorCode.inputNotFinite => l.valuationErrInputNotFinite(field),
    ValuationErrorCode.priceMustBePositive => l.valuationErrPriceMustBePositive,
    ValuationErrorCode.desiredYieldMustBePositive => l.valuationErrDesiredYieldMustBePositive,
    ValuationErrorCode.epsAndBookValueMustBePositive =>
      l.valuationErrEpsAndBookValueMustBePositive,
    ValuationErrorCode.keMustExceedGrowth => l.valuationErrKeMustExceedGrowth,
    ValuationErrorCode.waccMustExceedGrowth => l.valuationErrWaccMustExceedGrowth,
    ValuationErrorCode.sharesMustBePositive => l.valuationErrSharesMustBePositive,
    ValuationErrorCode.fadeYearsMustBePositive => l.valuationErrFadeYearsMustBePositive,
    ValuationErrorCode.wholeNumberRequired => l.valuationErrWholeNumberRequired(field),
    ValuationErrorCode.projectionYearsNegative => l.valuationErrProjectionYearsNegative,
    ValuationErrorCode.fairPriceMustBePositive => l.valuationErrFairPriceMustBePositive,
  };
}

String valuationVerdictText(AppLocalizations l, ValuationVerdict verdict) => switch (verdict) {
  ValuationVerdict.cheap => l.valuationVerdictCheap,
  ValuationVerdict.expensive => l.valuationVerdictExpensive,
};

String valuationVerdictShort(AppLocalizations l, ValuationVerdict verdict) => switch (verdict) {
  ValuationVerdict.cheap => l.valuationVerdictCheapShort,
  ValuationVerdict.expensive => l.valuationVerdictExpensiveShort,
};
