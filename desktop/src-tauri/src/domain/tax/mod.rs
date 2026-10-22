// Fase 24 — apuração de IR sobre renda variável (pessoa física, B3). Tudo
// aqui é puro (sem banco): o comando `commands::tax` monta as entradas a
// partir de `transactions`/`assets`/`tax_rules` e chama `assessment::assess`.
//
// Separação deliberada: os VALORES (alíquotas, limites, IRRF, mínimo de DARF)
// vêm de `RuleSet` — editáveis pelo usuário, atualizáveis pela tabela embarcada —, a
// LÓGICA (custo médio, day trade, gavetas, compensação) mora no código.

pub mod assessment;
pub mod calendar;
pub mod classify;
pub mod crypto;
pub mod foreign;
pub mod iof;
pub mod offshore;
pub mod rules;
