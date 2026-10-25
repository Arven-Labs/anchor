import '../domain/general_finance.dart' show ValuedItem;
import '../models/asset.dart';
import '../models/position.dart';
import 'portfolio_repository.dart';
import 'quote_dispatcher.dart';
import 'quote_exceptions.dart';

/// Uma posição com o preço atual da fonte certa para a classe dela.
class LivePositionRow {
  final Position position;
  final double? livePrice;

  /// A fonte de cotação não respondeu agora (limite de requisições, rede
  /// fora) — diferente de `livePrice == null` sem isto, que é "sem cotação".
  final bool priceUnavailable;

  const LivePositionRow(
    this.position,
    this.livePrice, {
    this.priceUnavailable = false,
  });

  /// `quantidade × preço` na moeda do ativo, ou `null` sem preço.
  double? get marketValue {
    final price = livePrice;
    return price == null ? null : position.netQuantity * price;
  }
}

/// As posições que entram no patrimônio, cada uma com o valor de mercado na
/// moeda do ativo. Fora ficam as zeradas (já vendidas) e as de
/// `empresaNaoListada`, cujo valor é a participação e não quantidade × preço.
/// Posição aberta sem cotação entra com valor nulo, que o patrimônio conta
/// como item de fora.
List<ValuedItem> portfolioValuedItems(Iterable<LivePositionRow> rows) => [
  for (final row in rows)
    if (row.position.netQuantity > 1e-9 &&
        row.position.asset.assetClass != AssetClass.empresaNaoListada)
      ValuedItem(row.position.asset.currency, row.marketValue),
];

/// Preço atual de cada posição, pela fonte da classe do ativo: avaliação
/// manual (imóvel), última cota lançada (fundo), CoinGecko numa chamada só
/// (cripto) ou o dispatcher (as demais). Compartilhado pelo Portfolio e pelo
/// patrimônio de Finanças.
Future<List<LivePositionRow>> loadLivePositions(
  List<Position> positions, {
  required PortfolioRepository repository,
  required QuoteDispatcher dispatcher,
}) async {
  // Todas as criptos já cadastradas (com `coin_id`) numa chamada só; se a
  // fonte falhar, todas ficam "indisponível agora" juntas.
  final coinIds = {
    for (final p in positions)
      if (p.asset.assetClass == AssetClass.cripto &&
          p.asset.externalId != null)
        p.asset.externalId!,
  };
  var cryptoPrices = <String, Map<String, double>>{};
  var cryptoUnavailable = false;
  try {
    cryptoPrices = await dispatcher.fetchCryptoPrices(coinIds);
  } on QuoteUnavailableException {
    cryptoUnavailable = true;
  } catch (_) {}

  return Future.wait(
    positions.map((position) async {
      final assetClass = position.asset.assetClass;

      // `imovel`: quantidade é sempre 1, então a última avaliação manual
      // serve direto como "preço atual" (sem chamar o dispatcher — não tem
      // fonte automática nenhuma pra essa classe).
      if (assetClass == AssetClass.imovel) {
        final latest = await repository.getLatestValuation(position.asset.id!);
        return LivePositionRow(position, latest?.value);
      }

      // `fundo`: sem cotação automática; o "preço atual" é a última cota
      // lançada à mão (aba de IR de fundos).
      if (assetClass == AssetClass.fundo) {
        final latest = await repository.getLatestFundQuota(position.asset.id!);
        return LivePositionRow(position, latest?.value);
      }

      // `empresaNaoListada`: o valor de participação já é o total da
      // posição (percentual × valuation da empresa), não um preço por
      // unidade — não entra na conta quantidade×preço, aparece como linha
      // separada no card.
      if (assetClass == AssetClass.empresaNaoListada) {
        return LivePositionRow(position, null);
      }

      final coinId = position.asset.externalId;
      if (assetClass == AssetClass.cripto && coinId != null) {
        return LivePositionRow(
          position,
          // Cada ativo lê o par da própria moeda (BRL novo, USD antigo).
          cryptoPrices[coinId]?[position.asset.currency.toLowerCase()],
          priceUnavailable: cryptoUnavailable,
        );
      }

      try {
        final quote = await dispatcher.fetchQuoteForAsset(position.asset);
        return LivePositionRow(position, quote.price);
      } on QuoteUnavailableException {
        return LivePositionRow(position, null, priceUnavailable: true);
      } catch (_) {
        return LivePositionRow(position, null);
      }
    }),
  );
}
