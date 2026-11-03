import '../models/asset.dart';
import 'app_localizations.dart';

// Textos do cadastro de ativo e da busca de cotação (ADR 0001). Um `switch` sem `default`
// faz o analisador acusar uma classe nova sem exemplo.

/// Exemplo de ticker (ou de nome, nas classes sem ticker) para o campo de cadastro e de busca.
/// Tickers reais (`PETR4`, `AAPL`) e o nome do título público (`Tesouro Selic 2029`) são iguais
/// nos dois idiomas; só os nomes inventados são traduzidos.
String assetTickerHint(AppLocalizations l, AssetClass assetClass) => switch (assetClass) {
  AssetClass.acaoBr => 'PETR4',
  AssetClass.fii => 'HGLG11',
  AssetClass.etfBr => 'BOVA11',
  AssetClass.etfRendaFixa => 'AUPO11',
  AssetClass.bdr => 'AAPL34',
  AssetClass.acaoInternacional => 'AAPL',
  AssetClass.reit => 'O',
  AssetClass.etfUs => 'SPY',
  AssetClass.cripto => 'ETH',
  AssetClass.metal => 'XAU',
  AssetClass.tesouroDireto => 'Tesouro Selic 2029',
  AssetClass.rendaFixa => l.hintFixedIncome,
  AssetClass.imovel => l.hintRealEstate,
  AssetClass.empresaNaoListada => l.hintUnlistedCompany,
  AssetClass.fundo => l.hintFund,
};
