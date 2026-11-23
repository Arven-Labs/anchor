# Mapa de domínios do crate (`desktop/src-tauri/src`)

> Criado na Sessão 407 (2026-11-23), commit base `2bc2d9e`. Só leitura do código: **nada foi movido**. Origem: o "primeiro passo barato" da pesquisa de modularidade do workspace (2026-11-13), que este mapa corrige em dois pontos (ver "Correções à pesquisa"). Os números envelhecem; refaça a medição antes de agir com base neles.

## Como o mapa foi feito
Cada arquivo `.rs` (exceto `flow_tests.rs` e `test_support.rs`, e os `mod.rs`/`prelude.rs` estruturais) foi atribuído a **um** domínio pelo nome e pelo que faz. Em seguida, para cada arquivo, contei as referências `crate::<módulo>::<item>` (inclusive dentro de `use crate::entity::{a, b}`), **cortando o arquivo no primeiro `#[cfg(test)]`** (teste não conta como dependência), e agrupei por domínio de origem e de destino. Referências a `crate::error` e `crate::db` não contam (são transversais). `domain/tax/` conta como uma unidade da carteira.

Limites: é análise textual (regex), não o compilador. Não pega `use super::*`, nem importação por re-export, nem chamada por caminho relativo dentro do mesmo diretório fora de `commands/`. Os números dão a ordem de grandeza e a lista de arestas; para conferir uma aresta, rode, por exemplo: `grep -n 'entity::custodia' desktop/src-tauri/src/commands/tax.rs`.

## Domínios e membros

| Domínio | Arquivos | O que é |
|---|---|---|
| **teses** (58) | commands: backtest, banks, bazin, company_filings, crypto_indicator, cvm_notice, dcf, dividend_growth, fii, gordon, graham, projected_ceiling, reit, rim, rnav, screener, stock_notes, thesis, thesis_draft, valuation. domain: backtest, banks, bazin, benchmark, crypto_score, dcf, dividend_growth, gordon, graham, projected_ceiling, rim, rnav, thesis_draft, valuation_outcome. entity: asset_valuations, *_inputs (banks, bazin, dcf, gordon, graham, projected_ceiling, rim, rnav), crypto_fear_greed, crypto_indicators, fii_cnpj_cache, fii_cvm_monthly, fii_cvm_properties, indicator_thresholds, reit_fundamentals, reit_manual_indicators, stock_dcf_fundamentals, stock_fundamentals, stock_notes, stock_technicals, theses, thesis_attachments, valuation | Modelos de preço-teto, score cripto, teses, backtest. É o propósito original do app. |
| **carteira** (50) | commands: asset, broker_settlement_io, currency, dividend_suggestion, fixed_income_io, fund, offshore, option_position, options, pending, pension, portfolio, portfolio_io, portfolio_summary, profitability, property, proventos, tax, transaction, watchlist. domain: calendar, cost_basis, currency_conversion, dividend_suggestion, option_payoff, option_position, position_pricing, proventos, **tax/** (16 arquivos), transaction_ledger, twr. entity: asset_attachments, asset_favorites, assets, fund_detail, fund_quota_value, import_pending, offshore_* (4), option_position, portfolio, stock_dividend_payments, suggested_dividends, tax_payments, tax_rules, transactions, watchlist_items, watchlists | Posições, lançamentos, rentabilidade, proventos, impostos (IR, exterior, previdência, fundos). |
| **financas_pessoais** (23) | commands: balance_reconciliation, bank_account, custodia, general_transaction, general_transaction_category, liability, loan, net_worth, porquinho_estimate, statement_io. domain: liability, loan, net_worth, porquinho_yield. entity: bank_account, borrower, custodia, general_transaction, general_transaction_category, liability, liability_installment, loan, loan_repayment | Contas, lançamentos gerais, dívidas, empréstimos, patrimônio líquido (ADR 0005). |
| **mercado** (59) | commands: collector, data_source_credentials, rates. domain: ptax, rates. entity: ingestion_runs, macro_index_monthly, ptax_daily, selic_daily, stock_adjusted_close, stock_dividends_avg, stock_price_history, stock_quotes. **finance_api/** (14) e **finance_core/** (30): CVM, Yahoo, CoinGecko, SEC, BCB, bolsai etc. root: ingestion, crypto_sources | Coleta e cache de dados externos. **Não é "finanças pessoais"**, apesar do prefixo `finance_`. |
| **sync_backup** (18) | commands: backup, storage_settings, sync_registry, truthid, workspace_file. entity: storage_settings. storage/ (2). root: arweave, backup, dead_drop, ecies, ipns_key, lan_sweep, pin_content_cipher, sync_registry, sync_snapshot_cipher, workspace_file | Fase 8 (congelada, ADR 0006), backup local, arquivo de workspace. |
| **wallet_viewer** (13) | commands: wallet_viewer. domain: bitcoin_xpub, wallet_address. entity: wallet_viewer_address, wallet_viewer_settings, wallet_viewer_xpub. wallet_viewer/ (7) | Visualizador de wallets (Ledger, Trezor, Bitcoin). ADR 0002. |
| **alertas** (5) | commands: alert_rule. domain: alert_check. entity: alert_event, alert_rule. root: alert_checker | Regras de alerta e verificação periódica. |
| **app** (12) | commands: home_layout, update_check, update_install, workspace, workspace_summary. entity: home_widget_layout, workspace, workspace_settings. root: db, error, lib, main | Casca: atualização, tela inicial, workspaces, banco, erro. |

Fora do mapa: `flow_tests.rs` (7.577 linhas, testes de fluxo que atravessam domínios de propósito) e `test_support.rs`.

## Matriz de dependências (origem -> destino, nº de referências em código de produção)

| Origem \ Destino | teses | carteira | finanças | mercado | sync | wallet | alertas | app |
|---|---|---|---|---|---|---|---|---|
| **teses** | — | 14 | 0 | 17 | 4 | 0 | 0 | 0 |
| **carteira** | 5 | — | 12 | 20 | 3 | 0 | 0 | 1 |
| **financas_pessoais** | 0 | 25 | — | 2 | 0 | 0 | 0 | 0 |
| **mercado** | 18 | 13 | 2 | — | 0 | 0 | 0 | 1 |
| **sync_backup** | 0 | 0 | 0 | 0 | — | 0 | 0 | 0 |
| **wallet_viewer** | 0 | 0 | 0 | 2 | 0 | — | 0 | 0 |
| **alertas** | 5 | 0 | 0 | 1 | 0 | 0 | — | 0 |
| **app** | 0 | 5 | 0 | 0 | 0 | 0 | 0 | — |

Os 2 de wallet -> mercado são `wallet_viewer/{esplora,rpc}` usando `finance_core::http` (o cliente HTTP compartilhado). A medição também contou 1 referência de `wallet_viewer/tests.rs` a `entity::assets` (carteira), que é arquivo inteiro de teste e **não** está na tabela por ser teste. Os 1 de carteira -> app e mercado -> app são `commands/currency` -> `entity/workspace_settings` e `ingestion` -> `entity/workspace`.

## O que a matriz diz

**Folhas limpas (ninguém de dentro do núcleo depende delas, e elas quase não dependem de nada):**
- **sync_backup:** zero referências para os outros domínios (na medição de produção; o `dead_drop`/`workspace_file` não foram abertos um a um). É consumido só por `commands/property` e `commands/thesis` (ambos via `storage::resolve_active_provider`, que está em `storage/`, hoje classificado como sync) e por `flow_tests`. É o domínio mais fácil de extrair; o ADR 0006 manda decidir o destino dele (remover ou módulo opcional) quando o arca chegar na fase 5, então **mover agora pode ser retrabalho**.
- **wallet_viewer:** não é importado por ninguém além do `lib.rs`. Só usa de fora o cliente HTTP `finance_core::http` (2 referências). Também folha.
- **alertas:** depende de teses (5) e lê `stock_quotes`; ninguém depende dele.

**Nó emaranhado (teses, carteira, finanças, mercado), com ciclos:**
- `carteira -> financas_pessoais` (12) **e** `financas_pessoais -> carteira` (25): ciclo. Os 12 de ida vêm de `tax` (lê `bank_account`, `custodia`, `general_transaction`, `general_transaction_category`), `fixed_income_io` (usa `general_transaction`, `loan`, `statement_io`), `pending`, `broker_settlement_io` (ambos usam `statement_io`), `portfolio_io` e `transaction` (usam `custodia`). **A afirmação "finanças só dependem da carteira, nunca o contrário" que levantei no plano estava errada.**
- `mercado -> teses` (18) e `mercado -> carteira` (13): a coleta **grava** direto nas entidades dos outros domínios (`finance_api/stocks`, `us_stock`, `crypto`, `fii`, `reit` escrevem `stock_fundamentals`, `crypto_indicators`, `fii_cvm_*`, `stock_dividend_payments`...), e `ingestion.rs` chama `commands::currency` e `commands::profitability` (a camada de dados chamando comando de Tauri, direção invertida).
- `teses -> carteira` (14): `backtest` usa `tax`/`profitability`/`currency`; `cvm_notice` e `thesis_draft` leem `assets`/`portfolio`/`transactions`; `thesis` -> `portfolio_io`.
- `teses -> mercado` (17) e `carteira -> mercado` (20): esperado, mas por `commands::collector`/entidades de cotação e não por uma interface.

**Hubs (maiores fan-in, por onde qualquer fronteira passa):** `commands/currency` (15 importadores), `commands/tax` (11), `commands/portfolio_io` (7), `commands/transaction` (5). Todos são da carteira. Para outro domínio depender só de uma interface estreita, esses quatro precisam de dono e de superfície definidos primeiro.

**Padrão geral:** `commands/` importa `commands/` (por exemplo `update_install` <-> `update_check` é um ciclo; `thesis_draft` -> `crypto_indicator`, `screener`). Em contraste, `domain/` é pura: não importa `entity` (0 referências) e só importa `error` e `finance_core` (1). A lógica de domínio já está separada de I/O; o acoplamento mora em `commands/`, `entity/` e `finance_api/`.

## Correções à pesquisa de modularidade (2026-11-13)
1. `finance_core` e `finance_api` são **coletores de dados de mercado** (domínio mercado), não "finanças pessoais". As finanças pessoais são os 23 arquivos da tabela acima (a pesquisa estimava ~13 mil linhas de Rust e ~4 mil de front, número que vinha dessa confusão).
2. Hipótese "finances e portfolio compartilham tabelas": **confirmada em parte**. A carteira lê 4 entidades das finanças (`bank_account`, `custodia`, `general_transaction`, `general_transaction_category`, em `tax`) e as finanças dependem fortemente da carteira. Não são separáveis sem decidir de quem é `custodia` e como o IR enxerga lançamentos gerais.
3. Pergunta "`finance_core` é usado pelas teses?": é usado pela camada de mercado, que as teses alimentam e consomem (ver acima), e `domain/` o importa em 1 ponto.

## Direção proposta (a decidir; nada aplicado)
Ordem do mais barato para o mais caro, sempre sem mudar comportamento:
1. **Documentar e vigiar a fronteira dos domínios que já são folha** (sync_backup, wallet_viewer, alertas): um teste que falhe se um arquivo desses importar de outro domínio, ou se outro domínio importar além de uma lista curta. Custo baixo, não move código.
2. **Quebrar os dois ciclos pequenos antes de qualquer agrupamento em pasta:** `update_install <-> update_check`, e a inversão `ingestion -> commands::{currency, profitability}` (puxar a lógica para `domain/` e deixar o comando só como casca).
3. **Decidir o dono de `custodia` e dos leitores de `general_transaction` em `tax`** (finanças x carteira). Sem isso, agrupar `finanças_pessoais/` numa pasta só renomeia o emaranhado.
4. Mover para pastas por domínio **só depois** de 1 a 3, porque em Rust `mod` só organiza nomes e não impede importação cruzada: a pasta sem a vigilância do passo 1 volta ao estado atual em poucos PRs.

Não recomendado: dividir em vários crates (cada hub acima viraria uma API pública forçada, e as 117 migrations são de um banco só), nem plugins instaláveis (um usuário, um banco).

## Não verificado
- Front-end (`desktop/src/*`): não medi imports entre as pastas (`finances`, `portfolio`, `stock-lookup`...). O grep que tentei não achou nenhum import relativo ou por alias entre elas, o que pode ser padrão de import diferente do que procurei, não ausência de acoplamento.
- Acoplamento por `use super::*`, re-exports e macros (a análise é textual).
- Acoplamento por **tabela** (SQL cru ou joins entre entidades dentro de uma query), que não aparece como `use`.
- Atribuição de alguns arquivos é julgamento (por exemplo `commands/rates` e `domain/rates` em mercado, `commands/asset` e `watchlist` em carteira, `domain/benchmark` em teses). Se mudar a atribuição, a matriz muda.
