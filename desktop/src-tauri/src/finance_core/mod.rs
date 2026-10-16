// Fase 26 — núcleo de dados financeiros do Anchor, que substitui o Lume
// (sidecar Python) domínio a domínio. `finance_api::client` segue sendo a
// fachada que os `collect_*` usam; cada função dela passa a chamar daqui em
// vez de falar HTTP com o sidecar. Sem dependência de UI (nada de Tauri
// aqui), pra poder virar crate própria/headless depois.
//
// - `http`: cliente HTTP compartilhado e o mapeamento de falhas pra
//   `NoData` × `ProviderUnavailable`.
// - `provider`: `DataProvider`, `Sourced` e a cascata entre providers.
// - `cache`: cache em memória com TTL e fallback `stale`, pros dados que não
//   são gravados no `anchor.db` (os que são já têm `fetched_at`/`source`).
// - `bcb_sgs`, `tax_rules`: domínio 1 (macro BCB, Selic e regras de IR).
// - `market`, `yahoo`, `metals`: domínio 2 (cotação, histórico, técnicos e
//   dividendos via `MarketDataProvider`; catálogo de metais), `bolsai` e
//   `api_keys` (primeiro provider BYO-key; chave no keyring ou no ambiente).
// - `b3_index_stats`, `b3_taxa_swap`: domínio 3 (índices IFIX/SMLL/IDIV e curva DI).
// - `coingecko`, `fear_greed`, `eth_indicators`: domínio 4 (cotação/histórico de
//   cripto, Fear & Greed e os 8 indicadores do ETH que o Lume servia).
pub mod api_keys;
pub mod b3_index_stats;
pub mod b3_taxa_swap;
pub mod bcb_sgs;
pub mod bolsai;
pub mod cache;
pub mod coingecko;
pub mod eth_indicators;
pub mod fear_greed;
pub mod http;
pub mod market;
pub mod metals;
pub mod provider;
pub mod tax_rules;
pub mod yahoo;
