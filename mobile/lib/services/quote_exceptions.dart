/// Lançada por qualquer serviço de cotação (`YahooQuoteService`,
/// `CoinGeckoQuoteService`) quando o ticker/símbolo não resolve.
class QuoteNotFoundException implements Exception {
  final String ticker;
  QuoteNotFoundException(this.ticker);
}

/// A fonte não respondeu de forma utilizável (limite de requisições, erro do
/// servidor, timeout ou rede fora) — diferente de `QuoteNotFoundException`:
/// o ativo pode existir, só não deu para cotar agora.
class QuoteUnavailableException implements Exception {
  final String source;
  QuoteUnavailableException(this.source);
}
