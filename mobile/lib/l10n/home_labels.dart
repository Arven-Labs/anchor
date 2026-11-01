import '../domain/home_layout.dart';
import '../models/asset.dart';
import 'app_localizations.dart';

// Textos do Início e das classes de ativo (ADR 0001). Um `switch` sem `default` nos enums
// faz o analisador acusar um valor novo sem texto.

String homeWidgetName(AppLocalizations l, HomeWidgetKey key) => switch (key) {
  HomeWidgetKey.totalValue => l.homeWidgetTotalValue,
  HomeWidgetKey.profit => l.homeWidgetProfit,
  HomeWidgetKey.allocation => l.homeWidgetAllocation,
  HomeWidgetKey.netWorth => l.homeWidgetNetWorth,
};

/// Nome da classe no idioma da tela. `AssetClass.label` (português) segue existindo para as
/// telas ainda não migradas e sai quando a última migrar.
String assetClassName(AppLocalizations l, AssetClass assetClass) => switch (assetClass) {
  AssetClass.acaoBr => l.assetClassAcaoBr,
  AssetClass.fii => l.assetClassFii,
  AssetClass.etfBr => l.assetClassEtfBr,
  AssetClass.etfRendaFixa => l.assetClassEtfRendaFixa,
  AssetClass.bdr => l.assetClassBdr,
  AssetClass.acaoInternacional => l.assetClassAcaoInternacional,
  AssetClass.reit => l.assetClassReit,
  AssetClass.etfUs => l.assetClassEtfUs,
  AssetClass.cripto => l.assetClassCripto,
  AssetClass.metal => l.assetClassMetal,
  AssetClass.tesouroDireto => l.assetClassTesouroDireto,
  AssetClass.rendaFixa => l.assetClassRendaFixa,
  AssetClass.imovel => l.assetClassImovel,
  AssetClass.empresaNaoListada => l.assetClassEmpresaNaoListada,
  AssetClass.fundo => l.assetClassFundo,
};
