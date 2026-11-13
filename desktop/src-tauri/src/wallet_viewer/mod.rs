//! Visualizador de wallets (ADR 0002): módulo opcional e isolado, só leitura.
//!
//! Regras do ADR que este módulo cumpre e `tests.rs` impõe:
//! - nenhuma chave estrangeira, consulta ou escrita para as tabelas de investimento;
//! - nenhum código de assinar nem de chave privada.

pub mod rpc;
pub mod store;

#[cfg(test)]
mod tests;
