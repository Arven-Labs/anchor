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
pub mod bcb_sgs;
pub mod cache;
pub mod http;
pub mod provider;
pub mod tax_rules;
