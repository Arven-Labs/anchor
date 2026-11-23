# 0007: Fronteiras de domínio no crate: medir e vigiar antes de mover

**Data:** 2026-11-23

## Contexto
A pesquisa de modularidade do workspace (2026-11-13, confiança média-baixa) propôs, como primeiro passo, traçar a fronteira por domínio dentro do crate `desktop/src-tauri`. Medido na Sessão 407 (`project/MODULES.md`, commit base `2bc2d9e`): o crate tem 8 domínios, três deles folhas (`sync_backup`, `wallet_viewer`, `alertas`) e quatro emaranhados (teses, carteira, finanças pessoais, mercado), com ciclos (`update_install <-> update_check` e, na medição original, `carteira <-> financas_pessoais`; **revisto na Sessão 408:** o ciclo entre carteira e finanças era quase só efeito de duas atribuições erradas, e depois de corrigi-las sobram 3 referências de `carteira` para `finanças`, todas do pagamento de DARF em `tax.rs`) e uma inversão (`ingestion` chama `commands::currency` e `commands::profitability`). A pesquisa tinha confundido `finance_core`/`finance_api` (dados de mercado) com finanças pessoais. Há um usuário e um banco (117 migrations).

## Decisão
1. **Não mover arquivos para pastas por domínio agora.** Em Rust, `mod` só organiza nomes; sem algo que imponha a fronteira, o emaranhado volta em poucos PRs.
2. **Ordem de trabalho, cada passo um PR, sem mudar comportamento:**
   1. Vigiar as folhas (`sync_backup`, `wallet_viewer`, `alertas`): um teste que falha se um arquivo delas importar de outro domínio, ou se outro domínio importar além de uma lista curta.
   2. Quebrar os ciclos pequenos e a inversão do `ingestion`, puxando a lógica para `domain/`.
   3. ~~O dono decide de quem é `custodia` e como `tax` enxerga `general_transaction`.~~ **Decidido em 2026-11-23 (Sessão 408):** `custodia` é da **carteira** (corretora, exchange ou wallet onde o ativo está: FK em `transactions`, custo médio por custódia, IR de exterior; nenhum arquivo de finanças a usa). `mark_tax_paid`, `unmark_tax_paid` e as categorias `Taxes`/`Yield`/`Fees` (hoje em `commands/tax.rs`) são das **finanças pessoais**; o cálculo do IR fica na carteira. **Feito na Sessão 409 (PR #223)**, movimento puro: `mark_tax_paid`, `mark_tax_paid_in` e `unmark_tax_paid` em `commands/tax_payment.rs`, as categorias em `general_transaction_category.rs`; medido depois, `carteira -> finanças` = 0. `PAYMENT_KIND_*` e `parse_date` ficam em `tax.rs`, para a carteira não depender das finanças.
   4. Tirar `read_spreadsheet_preview` de `portfolio_io` para um módulo neutro (`property` e `thesis` o usam).
   5. Só então agrupar em pastas.

Regra de dependência entre os domínios do núcleo, depois dos passos 3 e 4: `importacao -> {carteira, financas_pessoais}`, `financas_pessoais -> carteira`, e `carteira` não depende de finanças nem de importação. `importacao` é o bloco que cola extrato bancário e corretora (`statement_io`, `fixed_income_io`, `broker_settlement_io`, `pending`, `porquinho_estimate`, `portfolio_io`).
3. **O sync não é movido enquanto o ADR 0006 não decidir o destino dele.**
4. O mapa em `MODULES.md` é a fonte da fronteira; refazer a medição antes de agir com base nele.

## Alternativas
- **Mover já para pastas `teses/`, `carteira/`, `financas/`...:** descartada. Move ~240 arquivos sem impor nada, e os hubs (`currency`, `tax`, `portfolio_io`, `transaction`) continuariam importados por todos.
- **Dividir em vários crates (workspace Cargo):** descartada. Cada hub viraria API pública forçada, e as migrations e o banco são únicos. Reavaliar só se um domínio ganhar um segundo consumidor real.
- **Módulos instaláveis ou plugins:** descartada, um usuário e um banco (`padrao/modularidade.md`: separar no código agora, instalar depois).
- **Não fazer nada:** descartada porque o acoplamento entre carteira, finanças e importação cresce a cada fatia de Finanças que lê `tax`/`currency`; o custo de desfazer sobe com o tempo (previsão, não medida; a medição da 408 mostrou que o ciclo carteira<->finanças é menor do que a 407 supôs, o que reduz, mas não elimina, o argumento).

## Consequências
- Nenhum arquivo muda de lugar agora. O ganho imediato é a fronteira escrita e verificável.
- O teste de fronteira (passo 1) é código novo a manter, e é análise textual (regex), não o compilador: não pega `use super::*` nem re-export.
- **Custo de reverter:** nulo (só documento).
- **Não verificado:** o front (`desktop/src/*`), o acoplamento por SQL e a atribuição de alguns arquivos de fronteira, listada em `MODULES.md`.
