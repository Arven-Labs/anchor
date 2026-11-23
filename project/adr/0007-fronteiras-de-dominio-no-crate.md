# 0007: Fronteiras de domínio no crate: medir e vigiar antes de mover

**Data:** 2026-11-23

## Contexto
A pesquisa de modularidade do workspace (2026-11-13, confiança média-baixa) propôs, como primeiro passo, traçar a fronteira por domínio dentro do crate `desktop/src-tauri`. Medido na Sessão 407 (`project/MODULES.md`, commit base `2bc2d9e`): o crate tem 8 domínios, três deles folhas (`sync_backup`, `wallet_viewer`, `alertas`) e quatro emaranhados (teses, carteira, finanças pessoais, mercado), com ciclos (`carteira <-> financas_pessoais`, `update_install <-> update_check`) e uma inversão (`ingestion` chama `commands::currency` e `commands::profitability`). A pesquisa tinha confundido `finance_core`/`finance_api` (dados de mercado) com finanças pessoais. Há um usuário e um banco (117 migrations).

## Decisão
1. **Não mover arquivos para pastas por domínio agora.** Em Rust, `mod` só organiza nomes; sem algo que imponha a fronteira, o emaranhado volta em poucos PRs.
2. **Ordem de trabalho, cada passo um PR, sem mudar comportamento:**
   1. Vigiar as folhas (`sync_backup`, `wallet_viewer`, `alertas`): um teste que falha se um arquivo delas importar de outro domínio, ou se outro domínio importar além de uma lista curta.
   2. Quebrar os ciclos pequenos e a inversão do `ingestion`, puxando a lógica para `domain/`.
   3. O dono decide de quem é `custodia` e como `tax` enxerga `general_transaction`.
   4. Só então agrupar em pastas.
3. **O sync não é movido enquanto o ADR 0006 não decidir o destino dele.**
4. O mapa em `MODULES.md` é a fonte da fronteira; refazer a medição antes de agir com base nele.

## Alternativas
- **Mover já para pastas `teses/`, `carteira/`, `financas/`...:** descartada. Move ~240 arquivos sem impor nada, e os hubs (`currency`, `tax`, `portfolio_io`, `transaction`) continuariam importados por todos.
- **Dividir em vários crates (workspace Cargo):** descartada. Cada hub viraria API pública forçada, e as migrations e o banco são únicos. Reavaliar só se um domínio ganhar um segundo consumidor real.
- **Módulos instaláveis ou plugins:** descartada, um usuário e um banco (`padrao/modularidade.md`: separar no código agora, instalar depois).
- **Não fazer nada:** descartada porque o ciclo `carteira <-> financas_pessoais` já existe e cresce a cada fatia de Finanças que lê `tax`/`currency`; o custo de desfazer sobe com o tempo (previsão, não medida).

## Consequências
- Nenhum arquivo muda de lugar agora. O ganho imediato é a fronteira escrita e verificável.
- O teste de fronteira (passo 1) é código novo a manter, e é análise textual (regex), não o compilador: não pega `use super::*` nem re-export.
- **Custo de reverter:** nulo (só documento).
- **Não verificado:** o front (`desktop/src/*`), o acoplamento por SQL e a atribuição de alguns arquivos de fronteira, listada em `MODULES.md`.
