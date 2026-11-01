// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Portuguese (`pt`).
class AppLocalizationsPt extends AppLocalizations {
  AppLocalizationsPt([String locale = 'pt']) : super(locale);

  @override
  String get cryptoScoreTitle => 'Score cripto';

  @override
  String get cryptoThresholdsTooltip => 'Limites';

  @override
  String get cryptoRefresh => 'Atualizar';

  @override
  String get cryptoRefreshing => 'Atualizando…';

  @override
  String get cryptoNoReadings =>
      'Nenhuma leitura ainda. Toque em Atualizar para coletar.';

  @override
  String cryptoUnexpectedError(String detail) {
    return 'Erro inesperado: $detail';
  }

  @override
  String get cryptoFailuresTitle => 'Falhas da última coleta';

  @override
  String cryptoValidReadings(int count) {
    return '$count com leitura válida';
  }

  @override
  String get cryptoStatusNoReading => 'Sem leitura';

  @override
  String get cryptoStatusExpired => 'Vencida';

  @override
  String greenCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count verdes',
      one: '1 verde',
      zero: '0 verdes',
    );
    return '$_temp0';
  }

  @override
  String loggedCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count registrados',
      one: '1 registrado',
      zero: '0 registrados',
    );
    return '$_temp0';
  }

  @override
  String greensOfTotal(String greens, int total) {
    return '$greens de $total';
  }

  @override
  String get historyTitle => 'Histórico';

  @override
  String get historyEmpty => 'Sem histórico ainda.';

  @override
  String get historyExplainer =>
      'Parte dos indicadores verdes em cada data. Uma leitura deixa de contar quando passa do prazo.';

  @override
  String historyDateCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count datas',
      one: '1 data',
      zero: '0 datas',
    );
    return '$_temp0';
  }

  @override
  String historySummary(String dates, String greensOfTotal, String date) {
    return '$dates · último: $greensOfTotal em $date';
  }

  @override
  String historyDetail(String date, String text) {
    return '$date — $text';
  }

  @override
  String historyPoint(String greensOfTotal, String logged, String verdict) {
    return '$greensOfTotal · $logged · $verdict';
  }

  @override
  String get periodAll => 'Tudo';

  @override
  String get verdictIntact => 'Tese intacta — manter / seguir aportando';

  @override
  String get verdictNeutral => 'Neutro — acompanhar de perto';

  @override
  String get verdictReduceRisk => 'Reduzir risco';

  @override
  String get verdictShortIntact => 'Tese intacta';

  @override
  String get verdictShortNeutral => 'Neutro';

  @override
  String get verdictShortReduceRisk => 'Reduzir risco';

  @override
  String get signalGreen => 'Verde';

  @override
  String get signalNeutral => 'Neutro';

  @override
  String get signalRed => 'Vermelho';

  @override
  String get groupOnChain => 'On-chain';

  @override
  String get groupFundamentals => 'Fundamentos da rede';

  @override
  String get groupDerivatives => 'Derivativos e opções';

  @override
  String get groupLiquidity => 'Liquidez de mercado';

  @override
  String get groupSentiment => 'Sentimento de mercado';

  @override
  String get indicatorMvrvZScore => 'MVRV Z-Score';

  @override
  String get indicatorNvtRatio => 'NVT (vs. média de 90 dias)';

  @override
  String get indicatorPuellMultiple => 'Múltiplo de Puell';

  @override
  String get indicatorNetIssuance => 'Emissão líquida (emissão − queima)';

  @override
  String get indicatorStakingYield => 'Staking yield real';

  @override
  String get indicatorTvlTrend => 'Tendência do TVL (30d)';

  @override
  String get indicatorActiveAddressesTrend => 'Endereços ativos (30d)';

  @override
  String get indicatorExchangeNetflow => 'Netflow das exchanges';

  @override
  String get indicatorFeesVsEmission => 'Taxas vs. emissão';

  @override
  String get indicatorFundingRate7d => 'Funding dos perpétuos (7d, anualizado)';

  @override
  String get indicatorOpenInterestChange30d => 'Open interest (variação 30d)';

  @override
  String get indicatorLongShortRatio => 'Razão long/short (7d)';

  @override
  String get indicatorDvol => 'DVOL (volatilidade implícita)';

  @override
  String get indicatorPutCallRatio => 'Put/call de opções';

  @override
  String get indicatorStablecoinSupplyChange30d =>
      'Oferta de stablecoins (30d)';

  @override
  String get indicatorExchangeSupplyChange30d => 'Oferta nas exchanges (30d)';

  @override
  String get indicatorEthBtcTrend30d => 'ETH/BTC (30d)';

  @override
  String get indicatorFearGreedIndex => 'Fear & Greed (0–100)';

  @override
  String get indicatorMayerMultiple => 'Múltiplo de Mayer';

  @override
  String get indicatorBtcDominance => 'Dominância do BTC';

  @override
  String get indicatorSopr => 'SOPR';

  @override
  String get indicatorNupl => 'NUPL';

  @override
  String thresholdsTitle(String coin) {
    return 'Limites · $coin';
  }

  @override
  String get thresholdDefault => 'Padrão';

  @override
  String get thresholdEdited => 'Editado';

  @override
  String get thresholdGreen => 'Verde';

  @override
  String get thresholdRed => 'Vermelho';

  @override
  String bandsHigherIsBetter(String green, String red) {
    return 'Verde ≥ $green · Vermelho ≤ $red';
  }

  @override
  String bandsLowerIsBetter(String green, String red) {
    return 'Verde ≤ $green · Vermelho ≥ $red';
  }

  @override
  String get thresholdHigherIsBetter => 'Maior é melhor';

  @override
  String get thresholdLowerIsBetter => 'Menor é melhor';

  @override
  String thresholdDefaultValues(String green, String red) {
    return 'Padrão: verde $green, vermelho $red';
  }

  @override
  String get thresholdSaveWarning =>
      'Salvar reclassifica as leituras já gravadas e muda o histórico: o passado passa a ser visto com estes limites.';

  @override
  String get thresholdNeedTwoNumbers => 'Digite dois números.';

  @override
  String get thresholdNeedDifferentNumbers => 'Use dois números diferentes.';

  @override
  String get actionReset => 'Redefinir';

  @override
  String get actionCancel => 'Cancelar';

  @override
  String get actionSave => 'Salvar';

  @override
  String get actionSaving => 'Salvando…';
}
