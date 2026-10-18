import '../models/asset.dart';
import '../models/quote.dart';
import 'coingecko_quote_service.dart';
import 'quote_exceptions.dart';
import 'yahoo_quote_service.dart';

/// Único lugar que decide a fonte de cotação por classe de ativo — os 3
/// lugares que buscam cotação (busca livre, cadastro de ativo, posição ao
/// vivo do Portfolio) chamam isto em vez de cada um decidir a fonte por
/// conta própria.
class QuoteDispatcher {
  final YahooQuoteService _yahoo;
  final CoinGeckoQuoteService _coingecko;

  QuoteDispatcher({YahooQuoteService? yahoo, CoinGeckoQuoteService? coingecko})
    : _yahoo = yahoo ?? YahooQuoteService(),
      _coingecko = coingecko ?? CoinGeckoQuoteService();

  /// Ticker digitado, ainda sem `Asset` salvo (busca livre / formulário de
  /// cadastro antes de salvar).
  Future<Quote> fetchQuoteForTicker(String ticker, AssetClass assetClass) {
    switch (assetClass) {
      case AssetClass.acaoBr:
      case AssetClass.fii:
      case AssetClass.etfBr:
      case AssetClass.etfRendaFixa:
      case AssetClass.bdr:
        return _yahoo.fetchQuote(ticker);
      case AssetClass.acaoInternacional:
      case AssetClass.reit:
      case AssetClass.etfUs:
        return _yahoo.fetchQuote(ticker, suffix: '');
      case AssetClass.metal:
        return _yahoo.fetchMetalQuote(ticker);
      case AssetClass.cripto:
        return _coingecko.fetchQuote(ticker);
      case AssetClass.tesouroDireto:
      case AssetClass.rendaFixa:
      case AssetClass.imovel:
      case AssetClass.empresaNaoListada:
        // Sem fonte automática — nem o desktop tem cotação pra essas 4
        // classes (`AssetClassMeta.hasAutoQuote`). Quem quiser "preço
        // atual" de `imovel` usa `PortfolioRepository.getLatestValuation`
        // direto, fora do dispatcher.
        throw QuoteNotFoundException(ticker);
    }
  }

  /// Ativo já cadastrado — cripto usa o `coin_id` já resolvido e salvo em
  /// `Asset.externalId` em vez de rebuscar por texto.
  Future<Quote> fetchQuoteForAsset(Asset asset) {
    if (asset.assetClass == AssetClass.cripto) {
      return _coingecko.fetchQuote(asset.ticker, coinId: asset.externalId);
    }
    return fetchQuoteForTicker(asset.ticker, asset.assetClass);
  }
}
