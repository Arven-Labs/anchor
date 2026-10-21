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
// - `cvm_fii`: domínio 6 (indicadores mensais, imóveis e ticker→CNPJ dos FIIs).
// - `cvm_dfp`, `disk_cache`: domínio 5 (ROE, payout e fundamentos de DCF das
//   empresas, direto dos zips anuais da CVM; cache em disco compartilhado).
// - `cvm_ipe`: avisos de dividendos da CVM (categoria "Relatório Proventos" do
//   IPE: protocolo, data e link; valor e datas ficam no PDF). Fase 32, seção F.
// - `sec_edgar`: domínio 7 (fundamentos, DCF, payout e REITs dos EUA, direto do
//   XBRL da SEC; o e-mail de contato exigido por eles vem do keyring/ambiente).
// - `b3_options`, `black_scholes`, `option_greeks`: opções da B3 (Fase 32, seção H):
//   séries autorizadas e último preço do COTAHIST, Black-Scholes com vol implícita
//   e as gregas ponta a ponta (spot do chamador, taxa livre de risco da curva DI).
// - `fx`: câmbio multi-moeda (Fase 32, seção I): catálogo de 17 moedas e
//   validação do par; a cotação e o histórico vêm do `yahoo` (`EURUSD=X`).
// - `frankfurter`: segunda fonte de câmbio (taxas do BCE), atrás do Yahoo na
//   cascata de `yahoo::providers()`; só atende pares de moeda.
pub mod alpha_vantage;
pub mod api_keys;
pub mod b3_index_stats;
pub mod b3_options;
pub mod b3_taxa_swap;
pub mod bcb_ptax;
pub mod bcb_sgs;
pub mod black_scholes;
pub mod bolsai;
pub mod cache;
pub mod coingecko;
pub mod cvm_dfp;
pub mod cvm_fii;
pub mod cvm_ipe;
pub mod disk_cache;
pub mod eth_indicators;
pub mod fear_greed;
pub mod frankfurter;
pub mod fx;
pub mod http;
pub mod market;
pub mod metals;
pub mod option_greeks;
pub mod provider;
pub mod sec_edgar;
pub mod tax_rules;
pub mod yahoo;
