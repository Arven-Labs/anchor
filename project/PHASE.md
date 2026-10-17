## Fases Detalhadas

### Fase 0 — Fundamentos & Decisões de Arquitetura

**Objetivo**: decidir, com calma e com explicação de trade-offs, a stack do projeto antes de escrever código de verdade.

**Etapas**:
- [x] 0.1 — Nome do projeto → **Practice Valuation** (repo: `practice-valuation`), decidido na Sessão 1
- [x] 0.2 — Framework do app desktop → **Tauri + Rust + React/TypeScript** (mesmo padrão do TruthID), decidido na Sessão 1
- [x] 0.3 — Banco de dados local → **SQLite** (compartilhado entre o app Tauri/Rust e os coletores em Python), decidido na Sessão 1
- [x] 0.4 — Stack/lib de UI e direção visual → **Tailwind + shadcn/ui + TanStack Table**, visual **arejado tipo dashboard** (não denso), decidido na Sessão 1
- [x] 0.5 — Estrutura inicial do repositório, criada na Sessão 1:
  - `desktop/` — projeto Tauri + React + TS, gerado via `create-tauri-app` e renomeado (`practice-valuation`). Tailwind v4 já plugado (`@tailwindcss/vite`, `src/index.css`) — shadcn/ui e TanStack Table entram quando a Fase 4 começar a construir telas de verdade
  - `desktop/Dockerfile` + `docker-compose.yml` + `dev.sh` — ambiente de dev (ver "Ambiente de Desenvolvimento")
  - `data-collector/` — pasta reservada pro coletor Python (Fase 2), com só um `README.md` e `requirements.txt` vazio por enquanto — implementação real ainda não começou
  - Ainda falta: README.md na raiz do repo, LICENSE
- [ ] 0.6 — Checklist de segurança aplicado desde o primeiro commit (ver "Diretriz de segurança" acima)

---

### Fase 1 — Modelo de Dados

**Objetivo**: desenhar o schema do banco local que sustenta tudo — ativos, premissas, cálculos salvos, indicadores e alertas.

**Fonte da verdade pro schema**: as fórmulas completas de cada metodologia estão na Fase 3 (inputs, cálculo e guarda de erro, modelo por modelo).

**Entidades decididas** (revisado 2x depois da spec chegar — ver as duas notas de mudança de abordagem abaixo):
- `asset` — ativo acompanhado (ação BR ou cripto), com tipo, ticker/símbolo, nome
- **`valuation`** — tabela compartilhada por todos os modelos, com os campos comuns da "regra geral": `ticker`, `ano_ref`, `preco_atual`, `model` (qual dos 7 modelos), `preco_justo` (calculado, cacheado, nulo se a guarda de erro impediu o cálculo), `margem_seguranca`, `veredito`, `data_ultima_atualizacao`. Cada linha é um cálculo salvo — nada é sobrescrito, dá pra ter várias linhas do mesmo ticker com premissas diferentes (o "múltiplos preços-teto salvos" pedido desde o início). É a tabela que alimenta a tela de listagem (Fase 4.1) — uma consulta só, sem `UNION`
- **Uma tabela pequena de inputs por modelo**, ligada a `valuation` por FK (`valuation_id`), só com os campos específicos daquele modelo: `bazin_inputs`, `graham_inputs`, `gordon_ddm_inputs`, `dcf_fcff_inputs`, `bank_pb_roe_gordon_inputs`, `realty_rnav_inputs`, `projected_ceiling_price_inputs` (ver campos de cada um na Fase 3)
- `cripto_indicadores` — série temporal do score cripto: `moeda`, `data`, `indicador`, `valor_bruto`, `sinal` (verde/vermelho), `fonte` — permite plotar a evolução do score, não só o snapshot do dia
- `tracked_indicator` / `alert` — ainda a desenhar (Fase 5), quando entrarmos no motor de monitoramento/zona de compra

**Mudança de abordagem #1 (Sessão 1, depois da spec chegar)**: a ideia anterior de premissas genéricas em JSON (`assumption_set` flexível) foi **substituída** por tabelas rígidas por modelo, como o próprio spec funcional sugere — agora que os campos de cada metodologia são conhecidos e estáveis (não é mais "esperando a lista"), colunas tipadas por modelo são mais simples de validar (ex: as guardas `WACC−g <= 0`, `Ke <= g`) e mais fáceis de consultar do que um blob JSON.

**Mudança de abordagem #2 (Sessão 1, revisão pedida pelo usuário)**: a primeira versão dessa correção tinha virado "uma tabela por modelo" **auto-contida** (7 tabelas, cada uma repetindo os campos comuns tipo `ticker`/`ano_ref`/`preco_justo`). O usuário pediu uma revisão pensando em manutenção de longo prazo, e isso foi trocado por **`valuation` compartilhada + tabela de inputs por modelo** (acima) — evita repetir os campos comuns 7 vezes (mudar um campo comum = 1 migration, não 7) e deixa a tela de "listar tudo" trivial. Os inputs continuam tipados por modelo (não regrediu pra JSON) — só o que era comum foi extraído.

**Regra geral, comum a todos os modelos de ação** (ver spec): `margem_segurança = (preço_justo − preço_atual) / preço_justo`; `veredito` = CHEAP se margem > 0, senão EXPENSIVE. Todo modelo também carrega `ticker`, `ano_ref` (o app calcula `anos_desatualizado = ano_atual − ano_ref` e sinaliza: ≤0 em dia, ==1 atenção, ≥2 desatualizado) e `preço_atual` (API com fallback manual).

**Como Rust e Python acessam o mesmo banco**: os dois já rodam dentro do mesmo container (decisão da Fase 0 — stack híbrida), então não precisa de rede/API entre eles — só apontar os dois pro mesmo arquivo `.db`. Arquivo físico decidido: `data-collector/practice_valuation.db` (já coberto pelo `*.db` do `.gitignore` da raiz; a pasta já é bind-mount, então o arquivo sobrevive entre execuções do container).

**Etapas**:
- [x] 1.1 — Entidades validadas — desbloqueado pela chegada do spec funcional (Sessão 1). Ver "Mudança de abordagem" acima
- [x] 1.2 — Driver/ORM Rust: **SeaORM** — decidido na Sessão 1 (revisado depois de decidir `rusqlite` na mesma sessão). Motivo: o usuário já tem hábito de pensar em ORM (estilo Django/SQLAlchemy/ActiveRecord); `rusqlite` exigiria escrever SQL cru e mapear linha a linha na mão, atrito maior do que ganho de simplicidade pra quem tá aprendendo Rust e banco ao mesmo tempo. SeaORM imita bem esse modelo mental (`Entity::find().all(&db)`, migrations via `sea-orm-cli`, geração de entity a partir do schema). É assíncrono, mas isso não é custo extra real — o Tauri já roda sobre `tokio`. Trade-off aceito: SeaORM é mais novo/menos batalhado que Diesel (a alternativa "ORM maduro", descartada pela sintaxe de query mais macro-pesada e curva de compilador mais dura)
- [x] 1.2b — **Modo WAL** (Write-Ahead Log) do SQLite será ligado por padrão — Rust e Python são processos diferentes lendo/escrevendo o mesmo arquivo, e WAL deixa isso coexistir melhor (menos "database is locked")
- [x] 1.3 — Migrations iniciais (abordagem simples: arquivos SQL versionados aplicados em ordem, sem framework pesado). Rodando normalmente desde a Sessão 2 — cada modelo/indicador novo ganha sua própria migration (`sea-orm-cli migrate generate`), aplicada com `migrate up`. Marco final: 9 migrations aplicadas (`valuation`+`bazin_inputs`, uma por modelo de ação, `indicator_thresholds`+`crypto_indicators`)

---

### Fase 2 — Coleta de Dados

**Objetivo**: puxar o máximo de dado possível de fontes externas, com fallback manual quando a fonte automática não cobre.

**Histórico**: o levantamento abaixo foi desenhado pelo usuário antes deste projeto virar app desktop, pensando em escrever direto numa planilha do Google Sheets (via Service Account + `gspread`). Essa rota foi abandonada na Sessão 1 — o desenho de fontes/APIs e o pipeline de dados continuam válidos, só o destino final mudou de "planilha" pra "banco de dados local do app" (o módulo `sheets/writer.py` e a autenticação via Service Account descritos na ideia original não se aplicam mais).

**Fontes já mapeadas**:
| Categoria | Dado | Fonte primária | Fallback |
|---|---|---|---|
| Ações BR | Fundamentos (P/L, P/VP, ROE, ROIC, margens, EV/EBITDA — 27 indicadores TTM) | bolsai (200 req/dia grátis) | — |
| Ações BR | Cotação atual | Yahoo Finance (não-oficial, grátis pra qualquer ticker — trocou a brapi na Sessão 10, que exigia token pago fora de 4 tickers demo) | — |
| Ações BR | Balanço/DRE/DFC histórico (contas CVM brutas) | bolsai / CVM Dados Abertos (DFP/ITR) | — |
| Ações BR | Dividendos históricos | Yahoo Finance | — |
| Cripto | Preço, market cap, volume | CoinGecko | — |
| Cripto | TVL (DeFi) | DefiLlama | — |
| Cripto | Emissão líquida (issuance − burn, ETH) | ultrasound.money | — |
| Cripto | Endereços ativos/transações diárias | Etherscan (rate limit baixo) | — |
| Cripto | Exchange netflow, MVRV Z-Score, Puell Multiple | CryptoQuant/Glassnode (pago, sem alternativa gratuita boa) | manual, link pro dashboard |
| Cripto | Staking Yield líquido | stakingrewards.com (free tier limitado) | manual |
| PDF/release não estruturado | Campos qualitativos (landbank, comentários) | pdfplumber/PyMuPDF + API Claude (schema fixo) | preenchimento manual |

Cobre bem os indicadores de **triagem** (P/L, P/VP, ROE, DY, EV/EBITDA, CAGR receita) e 6 dos 8 indicadores de cripto de graça. Pros inputs finos do DCF completo (Capex de expansão vs manutenção, ΔNWC detalhado) o script deixa pré-preenchido com o dado bruto da CVM, mas ainda vale conferir contra o release nos casos historicamente problemáticos (banco, incorporadora).

**Estrutura de módulos planejada** (pasta `data-collector/`, ver Fase 0.5):
```
data-collector/
├── main.py                    # orquestrador — roda tudo ou um módulo específico
├── config.yaml                # lista de tickers/moedas a acompanhar, chaves de API
├── requirements.txt
└── sources/
    ├── acoes_bolsai.py         # cliente da API bolsai (fundamentos B3)
    ├── acoes_yahoo.py          # cliente do Yahoo Finance (cotação + dividendo médio)
    ├── cvm_dfp.py              # baixa o zip trimestral da CVM, mapeia conta → campo do modelo
    ├── pdf_extractor.py        # pdfplumber/PyMuPDF + chamada à API Claude com schema fixo
    ├── cripto_coingecko.py     # preço, market cap, volume
    ├── cripto_defillama.py     # TVL
    ├── cripto_ultrasound.py    # emissão líquida ETH (issuance − burn)
    ├── cripto_etherscan.py     # endereços ativos / transações
    └── cripto_stakingrewards.py # staking yield líquido
```
Chaves de API ficam em `.env`/`config.yaml` fora do controle de versão (ver "Diretriz de segurança").

**CVM Dados Abertos — como funciona na prática** (fonte principal pro DCF/RNAV/Bancos): não é uma API tipo REST (não dá pra chamar `/empresa/FIQE3`). É um **arquivo zip por ano**, com o balanço de todas as ~500 empresas abertas dentro:
```
https://dados.cvm.gov.br/dados/CIA_ABERTA/DOC/DFP/DADOS/dfp_cia_aberta_2025.zip
```
Dentro do zip, vários CSVs (um por demonstração: Balanço Ativo `BPA`, Balanço Passivo `BPP`, Resultado `DRE`, Fluxo de Caixa `DFC_MI` — sempre com versão `_con` = consolidado e `_ind` = individual). Cada linha: `CNPJ_CIA | DENOM_CIA | CD_CVM | DT_REFER | CD_CONTA | DS_CONTA | VL_CONTA`. `CD_CONTA` é o código fixo da conta (ex: `3.11` = Lucro Líquido, `2.03` = Patrimônio Líquido, `2.01.04` = Estoques — igual pra qualquer empresa aberta) e `VL_CONTA` o valor daquele período. É o mesmo dataset que bolsai/brapi consultam por trás — baixando direto, dá pra montar o próprio mapeamento conta → campo do DCF (Receita, EBIT, D&A, Capex, Dívida) sem depender de a API "empacotar" exatamente o campo necessário, e não quebra quando o layout de um PDF muda.

Fluxo do `cvm_dfp.py`:
1. `baixar_zip_ano(ano)` — baixa o zip do ano uma vez (todas as empresas vêm juntas)
2. Abre os CSVs com pandas, filtra pelas linhas da(s) empresa(s) de interesse (por `CNPJ_CIA` ou `DENOM_CIA`)
3. `ticker_para_cnpj(ticker)` — a CVM identifica empresa por CNPJ, não por ticker; resolve com uma chamada rápida à API bolsai/brapi só pra traduzir
4. `extrair_contas(cnpj, lista_codigos_conta)` — pivota só os `CD_CONTA` que interessam pro modelo (mapeamento fixo)

Pra maioria das empresas "normais" (o grosso da lista), esse caminho sozinho já cobre praticamente tudo — Capex, D&A, ΔNWC, dívida, tudo vem de contas padronizadas do DFP.

**Extração via PDF (fallback, só quando o dado não é estruturado)**: coisas como composição de landbank de uma incorporadora, ou comentário qualitativo do release, não vêm no DFP — só no PDF/apresentação. Pra esses casos: `pdfplumber`/`PyMuPDF` extrai texto e tabelas → vai pra API da Anthropic (Claude) com um prompt fixo pedindo só JSON com os campos que faltam (a mesma coisa que fazer manualmente mandando o PDF no chat, só que como script) → o script valida o JSON e grava no banco junto com a fonte ("Source: Release 4T25, pág. X") pra conferência rápida.

**Etapas**:
- [x] 2.1 — Decidir onde/como a coleta roda → **processo Python separado**, disparado **manualmente por um botão na UI** ("Run"/"Atualizar dados"), sem cron/scheduler (a ideia original de cron — 1x/ano ações, 1x/dia cripto — foi descartada, ver Sessão 1). Mecanismo:
  - Frontend: botão chama `invoke()` de um comando Tauri
  - Backend (Rust): comando assíncrono dispara o script Python como subprocesso (não trava a UI), espera terminar
  - Python: puxa os dados das fontes e grava direto no SQLite compartilhado
  - Frontend: enquanto roda, mostra spinner; ao terminar, mostra um resumo (quantos ativos, sucesso/erro) — sem log ao vivo linha a linha por enquanto (pode vir depois se sentir falta)
  - **Guarda contra clique duplo/spam**: desabilitar o botão no frontend enquanto roda **e** ter uma trava no lado Rust (ex: `Mutex`/flag no estado do app) que recusa uma segunda chamada concorrente mesmo se disparada rápido demais — evita dois processos Python escrevendo no mesmo SQLite ao mesmo tempo e evita estourar rate limit das APIs gratuitas
  - A Fase 5 (alertas) pode um dia precisar de checagem periódica dos indicadores **já salvos** — isso é diferente de "puxar dado novo" e fica pra quando chegarmos lá
- [x] 2.2 — Implementar clientes de fonte de dados de ações — **`acoes_brapi.py` (cotação) concluído na Sessão 4**; **`acoes_bolsai.py` (fundamentos LPA/VPA/ROE) e `cvm_dfp.py` (fundamentos do DCF, incluindo alíquota efetiva) concluídos na Sessão 5**; **`acoes_yahoo.py` (dividendo médio 5 anos, via API não-oficial do Yahoo Finance) concluído na Sessão 6**, substituindo a bolsai (bloqueada, 403 Pro-only) e resolvendo pra qualquer ticker real (não só demo, diferente da brapi). **Sessão 10**: `acoes_brapi.py` removido inteiramente — sua limitação de token pago fora dos 4 tickers demo quebrava a busca ad hoc por ticker recém-adicionada nos formulários; cotação migrou pra `acoes_yahoo.py::fetch_quotes` (mesmo endpoint não-oficial já usado pro dividendo médio, testado contra ticker real fora da demo — BBAS3 — antes de trocar). Todas as entradas contábeis do DCF automatizadas; só sobram as 5 premissas de mercado (Beta, Rf, prêmio de risco, Kd, g), que nunca vêm de balanço. **Nenhuma pendência de dado de ações sobrando**
- [~] 2.3 — Implementar clientes de fonte de dados de cripto — **`cripto_defillama.py` (TVL Trend) e `cripto_ultrasound.py` (Net Issuance) concluídos na Sessão 5**, ambos sem cadastro. Achado corrigido: a suposição inicial de que ultrasound.money não tinha API pública (só WebSocket) estava **errada** — o backend é open source (`eth-analysis-rs`, axum) e expõe rotas REST reais em `/api/v2/fees/*`, achadas lendo o código-fonte no GitHub em vez de só a doc/site. `cripto_etherscan.py` tentado (Sessão 5) e descartado — todo o módulo `stats` de séries diárias é Etherscan API Pro-only, sem workaround gratuito razoável pra `active_addresses_trend`. `staking_yield` (stakingrewards.com) investigado na Sessão 6 e descartado — sem free tier de verdade, só planos pagos. `mvrv_z_score`, `puell_multiple` e `exchange_netflow` seguem sem fonte gratuita conhecida — continuam manuais (ver mapeamento de fontes). **NVT Ratio e Fees de Rede vs Emissão concluídos na Sessão 21** (ver detalhe abaixo). **Sessão 92 (continuação)**: pesquisa nova (não só por memória — testada ao vivo) achou a CoinMetrics Community API cobrindo `mvrv_z_score`/`puell_multiple`/`exchange_netflow`/`active_addresses_trend` de graça — os 4 automatizados via `finance_api::crypto::ETH_INDICATORS` (Anchor) + `cripto_coinmetrics.py` (Fase 1.12 do easybusiness). Só `staking_yield` segue sem fonte grátis (beaconcha.in exige chave desde 2026, testado ao vivo) — é o único dos 9 que continua manual.
  - **Fees de Rede vs Emissão** (indicador 9): `cripto_ultrasound.py` ganhou `fetch_fees_vs_emission_ratio` — o cliente já usava `/api/v2/fees/supply-over-time` (variação líquida de supply); adicionado `/api/v2/fees/burn-sums` (confirmado real, também público/sem chave) pra pegar a queima (fees) no mesmo período de 30 dias. A API não devolve emissão bruta pronta — reconstruída como `variação_líquida + queima`. Ratio final = `queima / emissão_bruta`.
  - **NVT Ratio** (indicador 2, só "Parcial" na spec original): **decisão consultada com o usuário** entre duas fontes reais confirmadas — Blockchair (volume on-chain "de verdade", mas só valor de hoje, sem histórico grátis pra montar a MA de 90d que a regra de sinal pede) vs CoinGecko (`/coins/{id}/market_chart`, grátis/sem chave, dá market cap **e** volume dos últimos 90 dias numa chamada só, mas o volume é de exchange/trading, não liquidado on-chain — proxy mais fraca da definição original de Willy Woo). Usuário escolheu a proxy do CoinGecko por funcionar desde o primeiro dia, sem esperar ~90 dias de coleta acumulada. Novo `cripto_coingecko.py::fetch_nvt_ratio_vs_ma90` — NVT de hoje dividido pela média dos 90 dias fechados anteriores (exclui o dia de hoje, que ainda está em andamento, do cálculo da própria média).
  - Nenhuma migration nova — os thresholds de `nvt_ratio` (0.9/1.3) e `fees_vs_emission` (0.5/0.1) já estavam semeados desde a Sessão 1 (`m20260709_212958_create_crypto_score_tables.rs`), só nunca tinham fonte automatizada. `main.py::main_crypto` ganhou `collect_crypto_fees_vs_emission`/`collect_crypto_nvt_ratio`, mesmo padrão de `_record_crypto_indicator` dos outros 2. `CryptoScorePanel.tsx`: botão e comentário atualizados pra citar os 4 indicadores automatizados.
  - Testado ponta a ponta contra as APIs reais: `docker compose run ... .venv/bin/python3 main.py crypto` rodado de verdade (NVT Ratio 0,982 → NEUTRAL, Fees vs Emission 0,0139 → RED — ambos conferidos manualmente antes de codar), linhas reais confirmadas em `crypto_indicators` via `sqlite3` do módulo Python (o binário `sqlite3` da imagem Docker não está no PATH — achado: um shim de node-modules faz `node` tentar carregar `./sqlite3` como módulo, gerando `Cannot find module`; contornado usando `python3 -c "import sqlite3..."` em vez do binário CLI). `npx tsc --noEmit` limpo. **Confirmado pelo usuário no app real** ("deu boa, os dois indicadores apareceram")
- [~] 2.4 — Fallback de extração via PDF, **redesenhado na Sessão 29**: a ideia original (script Python isolado, `pdfplumber` + API da Anthropic direto) foi trocada por pedido do dono do projeto — reaproveitar a infraestrutura de IA que já existe no app (chat multi-provider da Fase 7: Gemini/Claude/OpenAI, chave via keyring, `list_api_keys`/`read_api_key_secret`) e nascer **genérica** ("IA lê um documento → devolve dado estruturado"), pensando também na ideia futura de teses a partir de releases (Fase 10.5, não iniciada). **Achado de pesquisa**: os 3 provedores já aceitam PDF nativo (sem `pdfplumber`/PyMuPDF nenhum) + JSON validado contra schema — Claude via bloco `document`+`output_config.format`, Gemini via `inline_data`+`generationConfig.response_schema` (**não confirmado contra chamada real nesta sessão** — só verificado contra doc, é o primeiro lugar a olhar se a extração via Gemini der 400), OpenAI Chat Completions via bloco `file`+`response_format.json_schema`. **Decisão de arquitetura**: comando avulso (`extract_document_data` em `commands/document_extraction.rs`, novo módulo — não mexe nos `ask_*_api` de `chat.rs`, que carregam histórico/tool-calling incompatíveis), sem tabela nova no banco e sem o mecanismo pesado de `ai_valuation_proposal` (aquele existe pra proteger uma escrita irreversível; aqui só preenche um `<Input>` já editável/revisado antes do submit, mesmo espírito do auto-fill de CVM da 2.5). Frontend: `DocumentExtractDialog.tsx` genérico (escolhe PDF via `tauri-plugin-dialog`, novo — nenhum file-picker existia no app antes —, escolhe chave/modelo com o mesmo padrão do chat, mostra preview com Usar/Descartar antes de gravar no formulário) — recebe `instructions`/`jsonSchema`/`parseResult`/`renderPreview`/`onAccept` de fora, então um futuro consumidor (Fase 10.5) não precisa tocar Rust nem esse componente de novo. Primeiro consumidor: botão "Extract from PDF" ao lado do campo `landbank` do RNAV (nunca teve fonte automática — só existe em texto/tabela de release, nunca no balanço formal da CVM). Refactor de passagem: `DEFAULT_MODEL_BY_PROVIDER` (duplicado em `ChatPanel.tsx`/`useConversationRuntime.ts`) extraído pra `desktop/src/ai/modelDefaults.ts`. `cargo check`/`cargo test --lib` (83/83, 6 novos) e `npx tsc --noEmit` limpos. **Pendência pra próxima sessão**: nenhum teste real contra API rodou ainda (usuário sem PDF à mão nesta sessão) — falta confirmar que o file-picker abre de verdade, que os 3 provedores aceitam o formato do body (principalmente o Gemini, sinalizado acima), e que o fluxo Extrair → preview → Usar este valor funciona ponta a ponta no app rodando.
- [x] 2.5 — Auto-fill de `net_cash` e `inventory_at_market_value` no RNAV a partir do balanço CVM já baixado. **Concluída na Sessão 29**: planejada com `/plan` (2 agentes `Explore` em paralelo levantaram o estado atual do RNAV nas 3 camadas e os códigos de conta reais já usados no `cvm_dfp.py`). **Achado importante**: os códigos anotados na ideia original da Sessão 28 (caixa `1.01`, dívida `2.01.04.01`/`2.02.01.04`, estoque `2.03`) estavam errados — `2.03`, por exemplo, já é Patrimônio Líquido no resto do próprio arquivo. Os códigos corretos (caixa `1.01.01`, dívida `2.01.04`+`2.02.01`) já são extraídos e testados desde a Sessão 5/20 como `cash`/`total_debt` de `fetch_dcf_fundamentals` — então `net_cash = cash − total_debt` não precisou de nenhuma mudança de backend, só conta no frontend. Só `inventory` (estoque, conta `1.01.04` do BPA, já uma dependência implícita do ΔNWC mas nunca devolvida isolada) era campo novo: adicionado ao retorno de `fetch_dcf_fundamentals`, nova coluna nullable em `stock_dcf_fundamentals` (migration `m20260728_103434_add_inventory_to_stock_dcf_fundamentals`, mesmo padrão de `payout`/`tax_rate`/`revenue`). `RnavForm.tsx::handleFetch` ganhou o cálculo de `net_cash` (sem guarda de nulidade — `cash`/`total_debt` são sempre não-nulos quando `dcfFundamentals` existe) e o prefill de `inventory_at_market_value` (guarda `!== null`, mesmo padrão de payout/tax_rate), com aviso de UI que o valor vem do custo contábil da CVM, não necessariamente o de mercado. `landbank` continua 100% manual, sem fonte automática. **Achado à parte, resolvido com aprovação do usuário**: o banco de dev tinha 2 migrations órfãs (`ai_alert_proposal` + coluna `target_valuation_id`), aplicadas em 2026-07-24 sem nenhum arquivo/commit correspondente no repo — revertidas do banco local antes de rodar a migration nova (sem perda de dado real: tabela vazia, coluna sempre `NULL`). `cargo check`/`cargo test --lib` (77/77) e `npx tsc --noEmit` limpos. Testado contra a CVM real pra 3 incorporadoras (CYRE3, EZTC3, MRVE3) — valores de estoque plausíveis frente à receita de cada uma, conferidos direto no banco. **Testado pelo usuário no app real** ("já testei e deu boa").
- [x] 2.6 — Bugfix em `cash` de `fetch_dcf_fundamentals`, achado auditando o RNAV da LAVV3 (Sessão 72): `cash` só lia a conta `1.01.01` (Caixa e Equivalentes), ignorando `1.01.02` (Aplicações Financeiras, Ativo Circulante) — pra Lavvi isso subestimava o caixa em ~R$722 milhões e inflava o `net_cash` negativo do RNAV em quase a mesma proporção (-R$1.123,7mi calculado vs. -R$393,5mi de dívida líquida real do release 4T25). Corrigido em `cvm_dfp.py` pra `cash = 1.01.01 + 1.01.02`. Resultado real pra LAVV3 (2025): -R$402,1mi — mais perto do release, mas ainda não exato porque falta a parcela de Aplicações Financeiras do Ativo **não circulante** (código CVM diferente, fora do escopo desse fix pontual, não implementado). **Achado à parte**: `inventory` (`1.01.04`) também não captura a parcela de estoque não circulante (`1.02.01.05`), mesma limitação, não corrigida ainda.
- [ ] 2.7 — **Ideia registrada, ainda não implementada (Sessão 72)**: dono do projeto perguntou se dá pra automatizar o processo usado nesta sessão pra montar o RNAV da LAVV3 — cruzar o PDF de release (`DocumentExtractDialog`, já existe desde a 2.4) com os dados já baixados da CVM, em vez de só extrair o `landbank` isolado feito hoje manualmente/via chat. Ideia consiste em duas partes:
  - **Landbank a valor de mercado**: releases de incorporadoras só trazem o *VGV* do landbank (potencial de receita futura), nunca uma avaliação de mercado do terreno em si — confirmado hoje lendo as 24 páginas do release 4T25 da Lavvi, nenhuma menção a valor de mercado do landbank. A extração por IA (2.4) hoje devolve o VGV cru, o que superestima muito o RNAV se o usuário não souber que precisa converter. Ideia: expandir o prompt/schema de `DocumentExtractDialog` pro landbank pra também aplicar (ou pelo menos sugerir) um percentual configurável de custo de terreno sobre VGV (a sessão usou 15%, mas é uma escolha de julgamento do usuário, não um número fixo do sistema) — resultado: campo continua editável, só nasce com uma estimativa melhor que "copiar o VGV direto".
  - **Cross-check automático de `net_cash`/`inventory` contra o release**: releases de incorporadoras costumam ter uma tabela "Dívida (Caixa) Líquida" com Caixa+Equivalentes, Títulos e Valores Mobiliários, Dívida Total e Dívida Líquida já calculada (foi exatamente isso que confirmou o bug do item 2.6 nesta sessão). Ideia: quando o usuário já tiver o PDF do release à mão, rodar uma segunda extração (reaproveitando a mesma infra 2.4) só pra pegar esses números "oficiais" da empresa e mostrar como comparação lado a lado com o que veio da CVM — sinaliza automaticamente se os dois divergem muito (ex.: >10%), sem substituir o dado da CVM (que é auditável/estruturado) por padrão. Não decidido: se isso vira um botão novo no RNAV, ou uma extensão do fluxo de extração do landbank pra também devolver esses campos extras no mesmo JSON.

---

### Fase 3 — Motor de Cálculo (Preço-Teto/Valuation)

**Objetivo**: calcular e salvar preços-teto/valuation com premissas customizáveis, permitindo múltiplos cálculos por ativo. Metodologias entregues pelo usuário na Sessão 1 — esta seção é a fonte da verdade completa (não precisa consultar outro arquivo).

#### Regra geral (vale pra todos os modelos de ação)

Todo modelo carrega 3 campos fixos além dos específicos — **ticker** (texto, ex: `FIQE3`), **ano de referência** (`ano_ref`, ano-base dos dados usados) e **preço atual** (R$, de API com fallback manual) — e termina com a mesma "cauda final":
```
margem_seguranca = (preco_justo − preco_atual) / preco_justo
veredito         = "CHEAP" se margem_seguranca > 0, senão "EXPENSIVE"
```
O app também calcula `anos_desatualizado = ano_atual − ano_ref` e sinaliza: `<=0` em dia, `==1` atenção, `>=2` desatualizado — é o campo que avisa quando revisar aquela empresa.

**Preço-teto/veredito salvos são uma fotografia, não recalculam sozinhos (achado/esclarecido na Sessão 73)**: `preco_justo`/`margem_seguranca`/`veredito` são gravados uma única vez, no momento do "Save" — usam o `preco_atual` digitado/buscado naquele instante, nunca são recalculados depois. "Refresh data" (tela de análise) só atualiza a cotação exibida, nunca as valuations já salvas — por isso o card "Saved valuation" pode continuar mostrando "EXPENSIVE" mesmo depois do preço cair abaixo do teto. Corrigido **na exibição** (não no dado salvo): o card "Saved valuation" da tela de análise (`StockAnalysisSection.tsx`) recalcula margem/veredito contra a cotação ao vivo em vez de usar os campos congelados do banco — mesma fórmula acima, reproduzida no frontend (`computeSafetyMargin`/`computeVerdict` em `stock-lookup/shared.tsx`). A linha da tabela em "All Saved Valuations" continua mostrando o valor congelado de propósito (é o registro histórico daquele cálculo).

**Bug conhecido, não corrigido (achado na Sessão 19, investigação a pedido do usuário)**: a fórmula da margem de segurança só dá o sinal certo quando `preco_justo > 0`. Nenhum dos 8 modelos garante isso antes de dividir (só o Graham é seguro por construção, já que exige `eps > 0` e `vpa > 0`). Se alguma entrada gerar `preco_justo <= 0` — ex. Bazin com dividendo médio negativo, Gordon/Projected Ceiling com dividendo não positivo, Banks com ROE muito baixo, RNAV com dívida líquida grande, DCF com alavancagem extrema — a divisão **inverte o sinal** (preço justo negativo classifica como "CHEAP" com margem alta, quando deveria sempre ser "EXPENSIVE") ou produz `NaN`/`Infinity` gravado silenciosamente no banco e exibido sem aviso na UI. Fix sugerido (não implementado): guarda `if preco_justo <= 0.0 { return Err(...) }` em cada `domain::<modelo>::calculate`, mesmo padrão dos guards `WACC−g`/`Ke vs g` já existentes, cada um coberto por um teste `rejects_non_positive_fair_price`.

#### 1. DCF / FCFF (empresas "normais")

**Quando usar**: empresa com capital de giro e capex previsíveis (varejo, indústria, tech, utilities). Não usar em banco ou incorporadora.

| Input | Unidade |
|---|---|
| Receita Líquida | R$ milhões |
| EBIT | R$ milhões |
| Alíquota Efetiva de IR | % |
| D&A (Depreciação/Amortização) | R$ milhões |
| Capex | R$ milhões |
| ΔNWC (variação capital de giro) | R$ milhões |
| Dívida Total | R$ milhões |
| Caixa | R$ milhões |
| Nº de Ações | milhões |
| Beta | número |
| Rf (taxa livre de risco) | % |
| Prêmio de Risco de Mercado | % |
| Kd (custo da dívida) | % |
| g (crescimento na perpetuidade) | % |

```
FCFF         = EBIT × (1 − IR) + D&A − Capex − ΔNWC
Ke (CAPM)    = Rf + Beta × Prêmio_Risco_Mercado
E (equity)   = Preço_Atual × Nº_Ações
WACC         = [E / (E + Dívida)] × Ke + [Dívida / (E + Dívida)] × Kd × (1 − IR)
Valor_Firma  = FCFF × (1 + g) / (WACC − g)
Valor_Equity = Valor_Firma − Dívida_Total + Caixa
Preco_Justo  = Valor_Equity / Nº_Ações
```
**Guarda**: se `WACC − g <= 0`, não calcular (modelo quebra matematicamente) — mostrar aviso em vez de número.

#### 2. Gordon / DDM (Dividend Discount Model)

**Quando usar**: boa pagadora de dividendo, crescimento previsível.

| Input | Unidade |
|---|---|
| Dividendo Atual (D0) | R$/ação |
| Crescimento Esperado dos Dividendos (g) | % |
| Ke (retorno exigido) | % |

```
D1          = D0 × (1 + g)
Preco_Justo = D1 / (Ke − g)
```
**Guarda**: `Ke > g`, senão inválido.

#### 3. Bazin

**Quando usar**: "vaca leiteira" (bancão, elétrica, saneamento), foco em yield de dividendo.

| Input | Unidade |
|---|---|
| Dividendo Médio por Ação (últimos 5 anos) | R$/ação |
| Yield Desejado | % (default sugerido: 6%) |

```
Preco_Teto = Dividendo_Médio / Yield_Desejado
```

#### 4. Graham (Graham Number)

**Quando usar**: filtro rápido de margem de segurança, qualquer empresa com lucro e patrimônio positivos.

| Input | Unidade |
|---|---|
| LPA (Lucro por Ação) | R$/ação |
| VPA (Valor Patrimonial por Ação) | R$/ação |

```
Graham_Number = RAIZ(22.5 × LPA × VPA)
```
**Guarda**: se LPA ≤ 0 ou VPA ≤ 0, não calcular (empresa com prejuízo ou PL negativo não se encaixa nesse método).

#### 5. Bancos (P/B via ROE-Gordon)

**Quando usar**: bancos e instituições financeiras — FCFF não serve porque dívida é matéria-prima do negócio, não uma alavancagem a evitar.

| Input | Unidade |
|---|---|
| VPA (Valor Patrimonial por Ação) | R$/ação |
| ROE | % |
| Payout | % |
| Ke (retorno exigido) | % |

```
g_sustentável = ROE × (1 − Payout)
P/B_Justo     = (ROE − g_sustentável) / (Ke − g_sustentável)
Preco_Justo   = P/B_Justo × VPA
```
**Guarda**: `Ke > g_sustentável`.

#### 6. RIM — Lucro Residual (Bancos)

**Quando usar**: bancos e instituições financeiras, como alternativa mais robusta ao modelo Bancos (P/B via ROE-Gordon, seção 5) — em vez de assumir ROE constante pra sempre, projeta o ROE convergindo (fade linear) do patamar atual até o próprio Ke (custo de capital) ao longo de N anos explícitos, sob a leitura de que a vantagem competitiva de um banco erode até ele criar exatamente zero lucro econômico no limite. Não precisa de valor terminal separado: em t=N o ROE já é Ke, então o lucro residual dali em diante é zero por construção. Quando ROE atual = Ke, o preço justo bate exatamente no valor patrimonial — mesmo caso particular do modelo Bancos (P/B_Justo = 1 quando ROE = Ke).

| Input | Unidade |
|---|---|
| VPA (Valor Patrimonial por Ação) | R$/ação |
| ROE Atual | % |
| Payout | % |
| Ke (retorno exigido, também o ROE de convergência) | % |
| N — Anos de Fade | inteiro |

```
Para t = 1..N:
  ROE_t          = ROE_atual + (Ke − ROE_atual) × (t/N)
  LucroResid_t   = (ROE_t − Ke) × VPA_(t-1)
  VPA_t          = VPA_(t-1) × (1 + ROE_t × (1 − Payout))
Preco_Justo       = VPA0 + Σ VP(LucroResid_t)
```
**Guarda**: `N >= 1`.

**Nominal, não real**: o modelo não faz nenhum ajuste/desconto de inflação em nenhum ponto (sem IPCA, sem deflator) — VPA e ROE de entrada são valores nominais correntes (contábeis/mercado), e o desconto usa `(1+Ke)^t` direto sobre esses fluxos nominais. Por isso o **Ke informado precisa ser nominal**; um Ke real infla artificialmente o preço justo. Aviso adicionado no formulário (`RimForm.tsx`, Sessão 74) porque o código não valida isso.

#### 7. Incorporadoras (RNAV)

**Quando usar**: construtoras/incorporadoras — o "estoque" é imóvel, não dá pra projetar FCFF de forma suave trimestre a trimestre.

| Input | Unidade |
|---|---|
| Landbank a Valor de Mercado | R$ milhões |
| Estoque a Valor de Mercado | R$ milhões |
| Caixa Líquido (caixa − dívida, pode ser negativo) | R$ milhões |
| Nº de Ações | milhões |

```
RNAV_Total = Landbank + Estoque + Caixa_Líquido
RNAV/Ação  = RNAV_Total / Nº_Ações
```
(`RNAV/Ação` entra no lugar de `preco_justo` na regra geral.)

#### 8. Preço Teto Projetivo

**Quando usar**: mesma lógica do Bazin, mas trazendo N anos de crescimento esperado pra frente e descontando a valor presente — útil quando se quer o teto "olhando pra frente", não só o dividendo de hoje.

| Input | Unidade |
|---|---|
| Dividendo Atual (D0) | R$/ação |
| Crescimento Esperado (g) | % |
| Anos de Projeção (N) | inteiro (default sugerido: 5) |
| Yield Desejado (alvo, estilo Bazin) | % (default sugerido: 6%) |
| Ke (taxa de desconto) | % |

```
Dividendo_Projetado_N = D0 × (1 + g)^N
Preco_Teto_Futuro_N   = Dividendo_Projetado_N / Yield_Desejado
Preco_Teto_Projetivo  = Preco_Teto_Futuro_N / (1 + Ke)^N
```
(`Preco_Teto_Projetivo` entra como `preco_justo` na regra geral.)

#### Persistência (ver Fase 1)

Tabela `valuation` compartilhada (`ticker`, `ano_ref`, `preco_atual`, `model`, `preco_justo` cacheado, `margem_seguranca`, `veredito`, `data_ultima_atualizacao`) + uma tabela pequena de inputs por modelo (só os campos específicos, ligada por FK). Permite mostrar histórico ("como essa margem evoluiu ano a ano") sem recalcular tudo toda vez, salvar quantos cálculos o usuário quiser por ativo (premissas diferentes = linhas diferentes, nada sobrescreve), e listar tudo com uma consulta só na `valuation`.

#### Score de Cripto (Ethereum) — score contínuo

Diferente de ação (1x/ano), aqui é um **score contínuo**: cada indicador vira verde (bom pra compra/manter) ou vermelho (sinal de reduzir risco), e o app soma quantos estão verdes de um total de 9 — contagem objetiva, não "vibe".

| # | Indicador | O que mede | Fonte | Automatizável? | Regra de sinal (ponto de partida — ajustável) |
|---|---|---|---|---|---|
| 1 | MVRV Z-Score | Preço vs custo-base médio da rede | Glassnode (pago) | Não (fallback manual) | Verde se < 0 · Vermelho se > 7 |
| 2 | NVT Ratio | "P/L" da rede (valor de mercado / volume transacionado) | Calculável com dado on-chain | Parcial | Verde se abaixo da média móvel de 90d · Vermelho se muito acima |
| 3 | Puell Multiple | Emissão diária (USD) vs média histórica | Glassnode (pago) | Não (fallback manual) | Verde se < 0.5 · Vermelho se > 4 |
| 4 | Emissão Líquida (issuance − burn) | ETH é deflacionário ou não no período | ultrasound.money | Sim | Verde se negativa (deflacionário) · Vermelho se fortemente positiva |
| 5 | Staking Yield Líquido | Retorno real do staking, descontada a diluição | DefiLlama (APY do stETH) menos inflação do CoinMetrics, Sessão 106 (stakingrewards.com não foi usado) | Sim | Verde se yield real > 2% · Vermelho se perto de 0 ou negativo |
| 6 | TVL DeFi (Ethereum) | Uso real da rede em DeFi | DefiLlama | Sim | Verde se em tendência de alta (MoM) · Vermelho se queda consistente |
| 7 | Endereços Ativos / Transações Diárias | Adoção/atividade da rede | Etherscan | Sim | Verde se crescendo (MoM/YoY) · Vermelho se caindo |
| 8 | Exchange Netflow | Saída (acumulação) ou entrada (venda) líquida das corretoras | CryptoQuant/Glassnode (pago) | Não (fallback manual) | Verde se saída líquida (negativo) · Vermelho se entrada líquida forte |
| 9 | Fees de Rede vs Emissão | "Receita líquida" real do protocolo pós EIP-1559 | ultrasound.money / Etherscan | Sim | Verde se fees líquidas cobrindo bem a emissão · Vermelho se dependente de emissão alta |

**Score final** = verdes / 9. Leitura sugerida (ajustável depois de ver rodando um tempo): **7-9 verdes** → tese intacta, manter/aportar · **4-6 verdes** → neutro, observar de perto · **0-3 verdes** → considerar reduzir risco/posição.

**Persistência**: tabela `cripto_indicadores` com `moeda`, `data`, `indicador`, `valor_bruto`, `sinal` (verde/vermelho), `fonte` — dá histórico de série temporal, dá pra plotar a evolução do score ao longo do tempo, não só o snapshot do dia.

**⚠️ Importante**: os thresholds acima (`< 0`, `> 7`, `> 2%`, etc.) são ponto de partida razoável baseado em uso histórico de mercado, **não são regra imutável** — o app precisa deixar esses números configuráveis (não hardcoded), porque o usuário provavelmente vai querer calibrar depois de ver como cada indicador se comporta na prática.

**Etapas**:
- [x] 3.1 — Lista de metodologias entregue (Sessão 1) — ver esta seção completa
- [x] 3.2 — Modelar cada metodologia (dos 7 modelos acima) como função pura Rust: inputs (tabela específica do modelo) → resultado (`preco_justo`, `margem_seguranca`, `veredito`), aplicando as guardas de erro — **concluído na Sessão 3**: os 7 modelos (Bazin, Graham, Gordon/DDM, DCF/FCFF, Bancos, RNAV, Preço Teto Projetivo) fechados ponta a ponta
- [x] 3.3 — Motor do score cripto — **concluído na Sessão 4**: sinal verde/neutro/vermelho por indicador com threshold configurável (tabela `indicator_thresholds`), leituras salvas em série temporal (`crypto_indicators`), score somado no front (verdes/9). Ver Log de Sessões pra detalhes de schema/domínio
- [ ] 3.4 — Permitir salvar quantos cálculos o usuário quiser por ativo (já é a natureza do schema — cada linha é um cálculo, nada sobrescreve), todos comparáveis lado a lado na UI — a parte de schema já está resolvida, falta só a tela de comparação (Fase 4)
- [x] 3.5 — 8º modelo, RIM — Lucro Residual (Bancos) — **concluído na Sessão 17**: generaliza o modelo Bancos (seção 5) permitindo o ROE convergir (fade) até o próprio Ke ao longo de N anos, em vez de assumir ROE constante pra sempre. Ver seção 6 acima pra fórmula/guarda e Log de Sessões pra detalhes de schema/domínio

**Sessão 56 (2026-08-02) — migração de UI + separação Calculate/Save.** Pedido explícito do dono do projeto: "migrar a tela de valuation pro research de ações" — os 8 modelos servem pra "empresas" (não só Ação BR; quando Stocks internacionais ganharem análise em Research, Fase 10 item 8, os modelos servem lá também), então a antiga aba própria "Valuation" saiu do nav principal. `StockAnalysisSection.tsx` (Research → Ação BR) ganhou dois botões no bloco "Saved valuation": **"New Valuation"** (abre `models/NewValuationDialog.tsx`, um popup com o mesmo seletor de modelo que a aba antiga tinha + o form escolhido, ticker pré-preenchido — editável, igual antes) e **"All Saved Valuations"** (abre `SavedValuationsPanel.tsx`, sem mudança nele, dentro de um popup — mantém a visão de todos os tickers, só muda de onde se abre). `App.tsx` perdeu `MODELS`/`selectedModel`/`valuationView` e a entrada `valuation` de `SECTIONS` — aba default virou "Research".

**Segundo pedido, mesma sessão**: "o calculate não vai mais salvar o preço teto, vai ter um botão de save, e um de calculate apenas pra calcular" — antes, `calculate_<model>` (as 8 chamadas Tauri, uma por modelo) calculava **e** já inseria no banco na mesma chamada. Separado em dois comandos por modelo: `calculate_<model>` virou preview puro (só roda `domain::<model>::calculate`, sem `INSERT`, devolve `ValuationOutcomeResponse` — um struct novo, compartilhado pelos 8, em `commands/valuation.rs`) e `save_<model>` novo (o que `calculate_<model>` fazia antes — recalcula do zero, insere `valuation`+`<model>_inputs`, mesmo princípio de não confiar em número já computado no cliente). `update_valuation` (editar um cálculo já salvo, dentro de "Saved Valuations") **não mudou** — o pedido foi sobre a criação de um cálculo novo, editar já era uma ação única sem estado de preview. Frontend: hook novo `models/useValuationActions.ts`, parametrizado por modelo (`calculate_${model}`/`save_${model}`), reaproveitado pelos 8 forms em vez de repetir a mesma `useMutation` 8 vezes — decisão consciente de abrir uma exceção ao "não abstrair" de Research (ali a duplicação é de conteúdo, aqui é wiring IPC mecanicamente idêntico). `ValuationResult.tsx` teve o tipo da prop afrouxado (só `fair_price`/`safety_margin`/`verdict`, os únicos campos renderizados) já que "Calculate" agora devolve um shape sem `id`/`ticker`/`updated_at`.

**Verificação**: `cargo check`/`cargo test --lib` **102/102** (sem teste novo — a matemática de cada modelo, testada via `domain::<model>::calculate`, não mudou; `save_<model>` é código novo mas CRUD fino, mesmo padrão de toda sessão anterior) e `npx tsc --noEmit` limpos. Rust recompilou e relançou sozinho (watch do `tauri dev`), frontend via HMR do Vite pros 8 forms + `NewValuationDialog.tsx` + `StockAnalysisSection.tsx` + `App.tsx`, sem reiniciar o container, sem erro em nenhuma etapa.

**Marco**: os 8 modelos de valuation deixam de ser uma tela solta no nav principal e passam a viver dentro da análise de cada ativo — "Calculate" e "Save" agora são ações distintas, como pedido, sem quebrar o fluxo de editar um cálculo já salvo (`SavedValuationsPanel.tsx`/`EditValuationForm.tsx`, inalterados).

#### 9. Preço-Teto para Commodities (ideia registrada na Sessão 65, ainda não decidida com o dono do projeto)

Dono do projeto trouxe um rascunho à parte (`feature_preco_teto_commodities.md`, conteúdo transcrito aqui e arquivo removido) — diferente dos 8 modelos acima, ainda não passou por `AskUserQuestion`/planejamento, é brainstorm de arquitetura puro. Já é antecipado pelo `ROADMAP.md` ("Mais metodologias de valuation... Bazin/preço-teto").

**Contexto/motivação**: DCF ingênuo com preço spot não funciona bem pra empresa de commodity — o valuation dança conforme o ciclo (value trap na baixa, falsa margem de segurança no topo). A ideia é calcular preço-teto usando inputs normalizados através do ciclo (preço de longo prazo, custo caixa through-cycle), em vez de projeções derivadas do preço/custo atual do trimestre.

**Escopo inicial proposto**: celulose/papel (Suzano, Klabin) e minério de ferro (Vale). Arquitetura pensada pra ser genérica o suficiente pra adicionar outras commodities (petróleo, soja, etc.) sem refatorar.

**Modelo de dados proposto** (3 entidades, não o par `valuation`+`<model>_inputs` que os 8 modelos acima usam — ver tensão de design abaixo):

```
CommodityAsset {
  ticker: string
  commodity_type: enum ("pulp", "iron_ore", ...)
  capacity: float          // capacidade produtiva, ton/ano
  currency: enum (BRL, USD)
}

CommodityAssumptions {
  ticker: string
  long_term_price: float         // preço de LP normalizado, consenso de research, por tonelada
  cash_cost_c1: float            // custo caixa normalizado (through-cycle, não último trimestre)
  cost_curve_percentile: float   // posição na curva de custo global (0-100)
  demand_growth_scenario: enum ("structural_growth", "cyclical_neutral", "structural_decline")
  sustaining_capex_per_ton: float
  replacement_cost_per_ton: float // custo de reposição de capacidade greenfield, referência p/ cross-check
  source: string                  // ex: "Wood Mackenzie Q2 2026", "Itaú BBA Commodities"
  as_of: date                     // data da última atualização — usado pra flag de "assumption desatualizada"
}

MarketData {
  ticker: string
  spot_price: float
  fx_rate: float
  net_debt: float
  shares_outstanding: float
  ev: float
  source: string
  fetched_at: timestamp
}
```

**Princípio de design proposto**: `CommodityAssumptions` é dado manual, versionado, atualizado esporadicamente (não existe API confiável pra preço de LP/C1 de commodity). `MarketData` é automatizável via B3/Yahoo (mesmo padrão dos coletores já existentes). O módulo nunca deveria misturar os dois como se tivessem o mesmo grau de atualização — a UI deveria mostrar `as_of` das assumptions com destaque quando desatualizada (ex.: > 6 meses), mesmo espírito do `anos_desatualizado` que a regra geral do topo desta fase já calcula pros 8 modelos atuais.

**Lógica de cálculo proposta**:
1. Receita normalizada = `capacity × long_term_price`
2. Custo normalizado = `capacity × cash_cost_c1` (conversão de câmbio se C1 estiver em USD e ativo reportar em BRL)
3. FCFF normalizado = receita − custo − `sustaining_capex_per_ton × capacity` − impostos, com WACC padrão do modelo geral (mesmo CAPM/WACC do modelo DCF/FCFF, seção 1)
4. Preço-teto (DCF) = valor presente do FCFF normalizado, perpetuidade com crescimento = 0 real (capacidade não muda no caso base)
5. Cross-check de reposição = `EV / capacity` vs. `replacement_cost_per_ton` — sinaliza desconto/prêmio em relação ao custo de construir a planta do zero
6. Ajuste por `demand_growth_scenario`: multiplicador de longo prazo sobre a receita normalizada — `structural_growth` (ex.: celulose, embalagem/tissue) fator 1.0 sem penalidade; `cyclical_neutral` fator 1.0; `structural_decline` (ex.: minério, risco de pico de aço chinês) fator configurável < 1.0, aplicado gradualmente sobre o horizonte de projeção, não como corte abrupto

**Fora de escopo, v1 (proposto)**: atualização automática de `long_term_price`/`cash_cost_c1` (permanece manual); cálculo de opcionalidade de capex de expansão (valor separado do caso base, feature futura); curva de custo dinâmica — v1 usa `cost_curve_percentile` como input estático.

**Tensão de design em aberto, não resolvida**: o rascunho propõe 3 entidades novas (`CommodityAsset`/`CommodityAssumptions`/`MarketData`) em vez de reaproveitar o par `valuation`+`<model>_inputs` que os 8 modelos acima já usam — faz sentido, já que `CommodityAssumptions` é mais persistente/compartilhada entre cálculos do mesmo ticker do que um `<model>_inputs` típico (que morre com o cálculo). Precisa decidir com o dono do projeto se isso vira um 9º par `valuation`+`commodity_inputs` (consistente com o resto, mas perde a persistência entre cálculos) ou se justifica mesmo um esquema à parte.

**Outras questões em aberto do rascunho**: onde essa tela se encaixa no fluxo atual (aba nova no cadastro de ação, ou fluxo separado só pra commodities?); fonte de `replacement_cost_per_ton` precisa de pesquisa específica por commodity, atualização provavelmente anual.

---

### Fase 4 — Interface Desktop

**Objetivo**: telas simples, "planilha-like", que dão espaço pra edição manual quando preciso.

**⚠️ Nota (Sessão 3)**: as telas dos modelos já implementados (Bazin, Graham) são propositalmente cruas — `<input>` HTML puro com classes utilitárias do Tailwind, sem os componentes do shadcn/ui instalados ainda. É rascunho funcional pra provar a fatia vertical (cálculo → banco → tela) de cada modelo, não a interface final. Decisão: terminar a Fase 3 (os 7 modelos + cripto) com esse padrão cru primeiro, e só então entrar na Fase 4 de verdade — instalar shadcn/ui, desenhar a navegação real (lista de ativos, histórico de cálculos salvos) e vestir os formulários de uma vez, em vez de estilizar um por um sem ainda saber todos os inputs que a navegação final vai precisar acomodar.

**Etapas**:
- [x] 4.1 — Tela: lista de ativos acompanhados — **concluída na Sessão 4** como "Saved Valuations": tickers distintos derivados da tabela `valuation` (sem tabela `asset` própria — ver Log de Sessões)
- [x] 4.2 — Tela: detalhe do ativo (histórico de cálculos salvos) — **concluída na Sessão 4**: comparação lado a lado dos campos comuns, detalhe fino (linha expansível "Assumptions" por cálculo) e **edição/exclusão em lugar** (coluna "Actions": View/Edit/Delete) — corrige um cálculo salvo sem virar linha nova, e remove um cálculo (cascade limpa a tabela de inputs específica sozinho)
- [x] 4.3 — Tela: cripto/indicadores — **vestida com shadcn/ui na Sessão 4** (mesmo painel de registro/placar, agora com Card/Select/Table/Badge em vez de HTML cru)
- [ ] 4.4 — Tela: alertas/zona de compra
- [x] 4.5 — Direção visual → **arejado, tipo dashboard** (Tailwind + shadcn/ui + TanStack Table), decidido na Sessão 1; **identidade de cor definida na Sessão 4** — dark navy + verde claro, inspirada no TruthID (ver Log de Sessões)

---

### Fase 5 — Monitoramento & Alertas

**Objetivo**: cadastrar premissas de compra por ativo e avisar o usuário quando o indicador entrar na zona configurada.

**Etapas**:
- [x] 5.1 — Cadastro de regra de alerta (tabela `alert_rule` polimórfica: `target_type` "stock_price" reusa o `fair_price` já calculado numa valuation salva, "crypto_indicator" reusa o signal GREEN/RED já calculado via `indicator_thresholds`; sem checagem periódica nem notificação ainda — só o CRUD)
- [x] 5.2 — Verificação periódica (background task no próprio app — `tauri::async_runtime::spawn` + `tokio::time::interval` a cada 5 min, reavalia contra dado já coletado, sem disparar o coletor; estado rastreado via tabela append-only `alert_event`)
- [x] 5.3 — Notificação nativa do SO, disparada só ao entrar em triggered. Validada de verdade na Sessão 12 (ver histórico da sessão) — precisou trocar `tauri-plugin-notification` por `notify-rust` direto com urgência crítica, já que KDE Plasma 6/Wayland suprime o popup de notificações normais vindas de apps sem `.desktop` registrado

---

### Fase 6 — Publicação (GitHub Público)

**Etapas**:
- [x] 6.1 — Checklist de segurança final (ver "Diretriz de segurança"). **Rodado na Sessão 47 (2026-08-02)**, item por item, contra o repo real (não só lido — testado): `.gitignore` cobre `.env`/`*.db`/cache/credenciais; `.env.example` sem valor real, única chave usada (`BOLSAI_API_KEY`) documentada, nenhuma outra `env::var`/`os.environ` no projeto; busca por padrão de chave/token/private key **em todos os arquivos trackeados e em todo o histórico do git** (`git log --all -G`, não só a árvore atual) — zero achado real (o único match foi a mnemonic de teste pública do `forge-std`, lib de terceiro); `config.yaml` já usa tickers fictícios/públicos com comentário explícito (decisão de sessão anterior); deploy do `SyncRegistry` usado Ledger físico, `contracts/broadcast/` só tem dado público (endereço + tx assinada, já on-chain); chaves de IA via keyring do SO, nunca arquivo. Dois achados de hardening/consistência (UI em português, CSP desabilitada) inicialmente registrados como pendência pra depois — **resolvidos na Sessão 48, mesmo dia**, ver abaixo.
- **Sessão 48 (2026-08-02) — os dois achados da 6.1 resolvidos**: `tauri.conf.json` ganhou `security.csp` (`default-src 'self'`, `img-src`/`connect-src`/`frame-src` liberando o protocolo de asset do Tauri — `asset:`/`https://asset.localhost`, usado pelo preview de anexo em Teses — e `https://icons.brapi.dev`, CDN do logo). Risco real identificado antes de codar: o app só roda via `tauri dev` hoje (nunca empacotado), e não estava claro se a CSP também restringe o modo dev (Vite injeta script/websocket pro HMR) — perguntado ao dono do projeto, que escolheu tentar direto em vez da alternativa mais robusta (config `--config` separada pra dev). Testado ao vivo: HMR do Vite continuou funcionando normal sob a CSP nova. Tradução da UI pra inglês: passada completa pelos ~20 arquivos que ainda tinham texto em português (toda a Fase 10, parte da Fase 9/FII, Settings, chat em tela cheia, extração de documento). Escopo deliberadamente restrito a texto visível ao usuário — nomes de campo do modelo de dados (`instituicao`/`titular`, valores de enum tipo `compra`/`pais`) e nomes próprios/siglas brasileiras sem equivalente direto (B3, FII, CPF, CNPJ, Tesouro Direto, CDI/IPCA/SELIC) ficaram fora, por serem mudança de schema maior/mais arriscada do que o achado original sinalizava. `cargo test --lib` 102/102 e `npx tsc --noEmit` limpos o tempo todo, testado ao vivo via HMR sem reiniciar o container.
- [x] 6.2 — README explicando o projeto (em inglês, já que o repo é público). **Concluído na Sessão 25** — adiantado por pedido do dono do projeto ("por mais que eu ache que é cedo"), mesmo o resto da Fase 6 não tendo começado; README pode ser revisado/expandido mais pra frente. Seções: o quê/por quê, features, tabela de arquitetura (desktop/data-collector/contracts), build from source (Docker pro app, Foundry pros contratos), status (aponta pro `PROJECT_STATE.md` pra detalhe), segurança (keyring, DB gitignored, não auditado), license
- [x] 6.3 — LICENSE. **Concluído na Sessão 25** junto com o README — MIT, mesmo texto/copyright (`masterlxz`, 2026) já usado no TruthID, pra consistência entre os dois projetos do mesmo dono
- [x] 6.4 — `git init` + primeiro commit. **Já estava satisfeita, corrigido na Sessão 26**: o checklist nunca tinha sido marcado, mas o repo já tinha `git init`/commits desde a Fase 0/Sessão 1 e histórico publicado em `origin` (GitHub) e `gitea` bem antes desta sessão — confirmado ao rodar `git push && git push gitea` (o range enviado foi só os commits novos, `81d77ac..7cc07d6`, não o histórico inteiro, prova de que o remoto já tinha tudo até `81d77ac`)
- [ ] 6.5 — **Ideia registrada na Sessão 10** (não planejada em detalhe ainda, só anotada): aviso de atualização disponível — um indicador discreto num cantinho da tela quando existir uma versão nova do app. Candidato natural: `tauri-plugin-updater` (checa contra um manifesto de release, ex. GitHub Releases) — só faz sentido depois que 6.1–6.4 existirem (precisa ter release/versionamento publicado pra ter contra o que comparar)

---

### Fase 7 — Chat de IA Integrado (ideia levantada na Sessão 10; **removida por inteiro na Sessão 101, Fase 20** — texto abaixo é registro histórico)

**Objetivo**: um chat de IA dentro do próprio app, num painel lateral flutuante (sobrepõe a tela atual, não é uma aba nova), onde o usuário usa a própria chave de API (Gemini/Claude/ChatGPT) pra tirar dúvidas sobre suas valuations e alertas salvos ("cara, essa valuation deu estranho, por quê?"). O chat tem acesso **só de leitura** ao banco (sem criar/editar nada) e a um "repertório" fixo de como os modelos do sistema funcionam. Histórico da conversa vive em memória enquanto o app está aberto — reseta só quando o app fecha, não quando o painel é fechado/reaberto.

Planejado com `/plan` na Sessão 10 (pesquisa real contra a doc do Gemini via `WebFetch`, e contra a doc/discussão do GitHub do Tauri sobre storage seguro — Stronghold, a opção mais óbvia, está sendo **descontinuada** na v3, então não é a escolha certa pra começar algo novo agora). Escopo combinado com o usuário: implementar em fatias, começando só pelo Gemini funcionando de ponta a ponta; Claude e OpenAI ficam desenhados na abstração mas com erro claro de "ainda não implementado" até uma sessão futura continuar.

**Decisões já tomadas com o usuário** (Sessão 10, antes de qualquer código):
- Painel lateral flutuante (shadcn `Sheet`, ainda não instalado no projeto — hoje só tem `badge`/`button`/`card`/`input`/`label`/`select`/`table`/`tabs`), não uma aba nova na `Tabs` existente
- Acesso ao banco **só leitura** por enquanto (sem criar valuation/disparar coletor via chat)
- Chave de API guardada no keyring do SO (não em texto puro em lugar nenhum, nunca vai pro SQLite nem pro git) — **risco em aberto**: o Linux usa Secret Service via D-Bus (mesmo barramento que a notificação nativa da Fase 5.3 precisou), mas ter o barramento montado não garante que exista um keyring daemon do outro lado dentro do container Docker de dev. Tratamento combinado: tentar de verdade, e se não achar um keyring disponível, mostrar erro claro na hora de salvar a chave em vez de travar silenciosamente — só descobrimos testando de verdade
- Histórico reseta ao fechar o **app**, não o painel — state React vive acima do `Sheet`, sobrevive abrir/fechar

**Etapas** (cada uma pode ser uma sessão/fatia separada):
- [x] 7.1 — Storage seguro da chave: crate `keyring` (v4, não `tauri-plugin-stronghold`, que está sendo descontinuado) chamada direto de comandos Tauri novos (`store_api_key`/`get_api_key_status`/`delete_api_key` — a chave em si nunca volta pro frontend, só um booleano "tem chave guardada"; já cientes de provedor, `SUPPORTED_PROVIDERS = ["gemini"]` por enquanto). Testado de verdade contra o container de dev (roundtrip + persistência entre processos separados via KWallet real do host, pelo mesmo D-Bus da Fase 5.3) **antes** de qualquer UI (Sessão 12) — maior risco técnico da feature inteira, resolvido
- [x] 7.2 — Cliente HTTP do Gemini em Rust: `reqwest` novo no `Cargo.toml` (nenhum outro comando fazia chamada HTTP de fora até aqui, só subprocess Python — primeira vez que o Rust fala com uma API de terceiro direto), testado de ponta a ponta contra a API real do Gemini (Sessão 13)
- [x] 7.3 — Contexto de leitura do banco: `ask_gemini` agora busca `valuation` (todas) + `alert_event` (últimos 20), formata como texto compacto e concatena com um repertório fixo (escrito à mão) explicando os 7 modelos de valuation e o score cripto — vira o `system_instruction` de cada chamada ao Gemini. Testado de ponta a ponta contra o banco real e a API real do Gemini (Sessão 14)
- [x] 7.4 — UI do painel: shadcn `Sheet` instalado via `npx shadcn add sheet` (gerou só `src/components/ui/sheet.tsx`, nenhuma dependência npm nova — o Radix Dialog por baixo já vinha junto do pacote unificado `radix-ui`), pasta nova `desktop/src/chat/` (`types.ts`, `ApiKeyGate.tsx`, `ChatToggleButton.tsx`, `ChatPanel.tsx`), estado do histórico e do aberto/fechado levantado pra `App.tsx` via `useState` simples (sem Context — só dois consumidores diretos, prop-drilling de um nível só) pra sobreviver abrir/fechar o painel, resetando só quando o app reinicia. Planejado com `/plan` (dois agentes `Explore` levantaram convenções exatas do frontend — imports relativos `../types` pros arquivos de feature, padrão `useMutation`/`useQuery` + `AppError`, estilo shadcn "radix-nova" — antes de escrever código) e testado de ponta a ponta pelo próprio usuário no app real rodando via `dev.sh`: abriu o painel sem chave salva (gate apareceu), salvou uma chave real do Gemini (gate sumiu, chat apareceu), mandou uma pergunta real e recebeu resposta usando o contexto do banco (Sessão 15)
- [x] 7.5 — Abstração multi-provider: novo `domain::chat_provider::Provider` (enum `Gemini`/`Claude`/`OpenAi`, `parse`/`as_str`, testado com 3 testes unitários) substitui a lista solta `SUPPORTED_PROVIDERS` que só aceitava "gemini". `commands/gemini.rs` renomeado pra `commands/chat.rs`; o comando público virou `ask_ai(provider, db, history)` (era `ask_gemini(db, history)`), com um `match` que chama o Gemini de verdade pro braço `Provider::Gemini` e devolve o novo erro tipado `AppError::ProviderNotImplemented` pros braços `Claude`/`OpenAi` — distinto de `UnknownProvider` (que continua existindo pra string que não é nenhum dos 3 IDs). `store_api_key`/`get_api_key_status`/`delete_api_key` passaram a aceitar os 3 IDs também (decisão consultada e aceita: liberar o storage pros 3 desde já, mesmo sem o HTTP client de Claude/OpenAI existir ainda). Planejado com `/plan` antes de codar (leitura direta de `gemini.rs`/`api_key.rs`/`error.rs`/`ChatPanel.tsx` em vez de agentes `Explore`, já que os arquivos são pequenos e já tinham sido lidos na sessão). `cargo test --lib` (48/48, incl. os 3 novos de `chat_provider`) e `npx tsc --noEmit` limpos; teste temporário (`#[test]`, escrito e removido) confirmou roundtrip real no keyring do host pra `claude`/`openai`. **Testado de ponta a ponta pelo usuário no app real** (`./desktop/dev.sh`): abriu o painel (chave Gemini já salva de sessão anterior), mandou uma pergunta real, resposta veio certa via `ask_ai` — confirma que o dispatcher genérico não quebrou o caminho do Gemini (Sessão 16)
- [x] 7.6/7.7 — **Claude e OpenAI implementados na Sessão 21**, os dois de uma vez (mesmo formato de request, fez sentido fazer junto). `ask_claude_api` (`api.anthropic.com/v1/messages`, header `x-api-key` + `anthropic-version: 2023-06-01`, `system` como campo próprio) e `ask_openai_api` (`api.openai.com/v1/chat/completions`, header `Authorization: Bearer`, sem campo `system` separado — vira a primeira mensagem do array) — a pergunta em aberto da 7.7 sobre migrar pra Responses API não foi revisitada, ficou em Chat Completions mesmo (mais simples, suficiente pra um chat de pergunta e resposta sem tool-calling ainda). As duas convertem o histórico compartilhado `GeminiContent` (role `"user"`/`"model"`) pro formato de cada API (`"user"`/`"assistant"`) via uma função `gemini_role_to_assistant_style` reaproveitada pelas duas. Erros novos no `error.rs`: `ClaudeApi`/`OpenAiApi`, mesmo molde do `GeminiApi` já existente. `AppError::ProviderNotImplemented` **removido** (não sobrou nenhum branch que o usasse, os 3 providers têm cliente real agora).
  - **Escolha de modelo virou parâmetro, não constante fixa**: `ask_ai(provider, model, db, history)` ganhou o campo `model` (o Gemini também passou a receber o model por parâmetro em vez do `GEMINI_MODEL` fixo, pra ficar simétrico aos outros 2). Decisão consultada com o usuário: em vez de eu escolher um modelo default pra cada provider e travar, o `ChatPanel.tsx` ganhou um seletor de provider (`Select` do shadcn, já usado noutras telas) + um campo de texto livre pro modelo (mesmo padrão do campo "Coin" do Crypto Score), com um valor sugerido por provider (`gemini-3.1-flash-lite`, `claude-haiku-4-5`, `gpt-5-mini` — todos tier barato/rápido, mesmo raciocínio do Gemini já usado: é um widget de pergunta rápida sobre dado já salvo, não um agente de código). Trocar de provider já reseta o campo de modelo pro sugerido daquele provider. `ApiKeyGate.tsx` também parou de ter "gemini" fixo — recebe `provider` como prop e mostra o rótulo/placeholder certo (Claude usa chave `sk-ant-...`, OpenAI `sk-...`).
  - `cargo check`/`cargo test --lib` (52/52) e `npx tsc --noEmit` limpos. **Teste contra API real ainda pendente** — usuário não tinha chave de teste de Claude/OpenAI à mão nesta sessão (o padrão de teste seguro via arquivo no scratchpad + `--env-from-file`, já usado no Gemini na Sessão 13, está pronto pra usar assim que houver uma chave real disponível). Até lá, o código está verificado por `cargo check`/tipos e revisão manual da forma exata dos dois requests (conferida contra a doc oficial), mas não confirmado ponta a ponta contra a API de verdade.
- [ ] 7.8 (ideia levantada na Sessão 15) — Conversas salvas, múltiplas, nomeadas. **Absorvida pela ideia maior da 7.10 (Sessão 20, ver abaixo)** — vira a etapa 7.10.2, dentro da tela cheia nova, em vez de um item isolado do widget flutuante atual
- [ ] 7.9 (ideia levantada na Sessão 16, logo depois do smoke test da 7.5) — Página de configurações de verdade com múltiplas chaves nomeadas por provider + redesenho do painel de chat. Duas perguntas de escopo já esclarecidas com o usuário na Sessão 16: (1) "quantas APIs eu quiser, com nome de cada uma" significa **várias chaves por provider** (ex: 2+ chaves de Gemini, cada uma com um apelido) — não é só um apelido pro provider inteiro, então o storage deixa de ser "1 chave = 1 provider" (`keyring::Entry::new(service, provider.as_str())`) e vira uma lista de entradas nomeadas; (2) a tela mora numa **rota/página de Configurações nova**, fora das `Tabs` que já existem na tela principal, não uma aba a mais. Quebrada em 5 etapas (cada uma pode ser uma sessão/fatia separada, mesmo padrão da Fase 7 original — a ordem prioriza o ganho visual rápido e de baixo risco primeiro, depois a mudança de storage de maior risco técnico, só then a UI que depende dela):
  - [x] 7.9.1 — Redesenho do painel de chat — **concluído na Sessão 17**: `ChatPanel.tsx` trocou o `Sheet` (Radix Dialog, ocupava a tela inteira de cima a baixo) por uma `div` flutuante de tamanho fixo (360×672px, proporção ~9:16 ajustada com o usuário testando ao vivo), ancorada perto do `ChatToggleButton` (canto inferior direito), sem overlay/backdrop — mais parecido com um widget de chat (Intercom-style) do que um modal. Fechar com Escape precisou de um listener manual (`document.addEventListener`), já que deixou de vir de graça do Radix Dialog. Só CSS/layout, nenhuma mudança de backend
  - [x] 7.9.2 — Storage de múltiplas chaves nomeadas. **Concluída na Sessão 21**: tabela nova `ai_api_key` (`id`, `provider`, `name`, `created_at`) via migration SeaORM (`m20260716_005530_create_ai_api_key`) — o segredo continua só no keyring, agora sob username `"{provider}:{id}"` (`api_key.rs::keyring_username`) em vez do provider puro, então renomear só toca a coluna `name`, nunca o keyring. `store_api_key`/`get_api_key_status`/`delete_api_key` substituídos por `create_api_key(provider, name, key) -> id`, `list_api_keys() -> Vec<ApiKeySummary>` (nunca devolve o segredo), `rename_api_key(id, name)`, `delete_api_key(id)`; novo `read_api_key_secret(db, key_id)` (não é comando Tauri, só usado pelo `ask_ai`). **Migração automática das chaves antigas** (decisão consultada com o usuário): `list_api_keys` detecta, na primeira chamada, uma chave ainda no esquema antigo (`username = provider` puro) sem nenhuma linha nova ainda pra aquele provider, cria uma linha `"Default"` e migra o segredo pro novo username — idempotente, sem perder a chave do Gemini já configurada desde a Sessão 15
  - [x] 7.9.3 — `ask_ai` concluída na Sessão 21 junto com a 7.9.2: passou a receber `key_id: i32` em vez de `provider: String` — busca a linha em `ai_api_key` via `read_api_key_secret` pra descobrir o provider e o segredo, despacha pro mesmo `match` de `Provider` que a 7.5/7.6/7.7 já criaram
  - [x] 7.9.4 — Página de Configurações, concluída na Sessão 21. **Decisão de arquitetura consultada com o usuário**: sem lib de rotas — app desktop de janela única não ganha nada com URL/histórico de navegador. `App.tsx` ganhou um `useState<"main" | "settings">` que troca a tela inteira (mesmo padrão do toggle form/saved já existente dentro da aba Valuation, só que no nível do App), acionado por um ícone de engrenagem ao lado da lista de abas. `settings/SettingsPage.tsx` novo: barra lateral com só a seção "IA" por enquanto (sem abstração de registro de seções — seria over-engineering pra 1 item só), conteúdo lista chaves agrupadas por provider, formulário de nova chave, rename inline (clique no nome vira input) e delete com o mesmo padrão de confirmação dupla já usado no `SavedValuationsPanel` (`confirmingDeleteId`, botão vira "Confirm?" destructive no primeiro clique)
  - [x] 7.9.5 — Seletor de chave no chat, concluído na Sessão 21: `ChatPanel.tsx` trocou o seletor de provider (da 7.6/7.7) por um seletor de chave nomeada (`Select` populado por `list_api_keys`, rótulo "{nome} ({provider})"), troca a qualquer momento sem resetar o histórico da conversa (requisito já dado antes: "que seja possível alternar na conversa"). Campo de modelo (texto livre) mantido, só a sugestão de modelo padrão passou a vir do provider da chave escolhida em vez de um seletor de provider próprio. `ApiKeyGate.tsx` (gate de "nenhuma chave ainda") também virou provider-agnóstico — pede pra escolher o provider inline e cria a primeira chave chamada "Default" via `create_api_key`
  - **Planejada com `/plan` (EnterPlanMode)** antes de codar, dado o tamanho (migration+entity, reescrita de 4 comandos, troca de esquema do keyring, `ask_ai` mudando de assinatura, página nova). Um agente `Explore` levantou a estrutura de navegação real do `App.tsx`, confirmou que não existe lib de rotas instalada, e trouxe o padrão exato de migration/entity/registro de comando a seguir antes de qualquer código ser escrito. `cargo check`/`cargo test --lib` (52/52) e `npx tsc --noEmit` limpos a cada etapa; `sea-orm-cli migrate up` + `generate entity` + `chown` rodados de verdade contra o banco real. **Testado de ponta a ponta pelo usuário no app real** rodando via `dev.sh` (o `tauri dev` recompilou e reabriu a janela sozinho ao detectar as mudanças no Rust): confirmou a chave do Gemini migrando sozinha pra "Default", criação de chave nova, rename, delete com confirmação dupla, e envio de mensagem real pelo chat usando o seletor de chave — "deu boa, testei tudo e funcionou"

- [x] 7.10 (ideia grande levantada na Sessão 20) — **Chat em tela cheia, estilo ChatGPT**, como uma segunda experiência de chat dentro do app, separada do widget flutuante atual (que **continua existindo do jeito que está hoje**, pra perguntas rápidas — não é substituído). **Completa**: alicerce (7.10.1/7.10.2/7.10.5) na Sessão 26, contador de tokens (7.10.3) e proposta de valuation com aprovação humana (7.10.4) na Sessão 27 — ver detalhe completo logo abaixo dos itens. Requisitos trazidos pelo usuário, em ordem de menção:
  - [x] 7.10.1 — Tela/rota dedicada em tela cheia. **Concluída na Sessão 26**: sem lib de rotas (mesma decisão da 7.9.4) — `AppView` ganhou `"chat"`, mesmo padrão `useState`+branch condicional de `"settings"`, novo ícone (`MessageSquareIcon`) ao lado da engrenagem
  - [x] 7.10.2 — Conversas salvas, múltiplas, nomeadas — absorve a ideia da antiga 7.8 (ver acima). **Concluída na Sessão 26**: tabelas `ai_conversation`/`ai_message`, primeira escrita no banco feita pelo chat (reverte a restrição só-leitura da Sessão 10 só pra essa parte — salvar a conversa em si, não criar/editar valuations, isso continua sendo só a 7.10.4)
  - [x] 7.10.3 — Contador de tokens visível pro usuário, pra ter noção de custo/uso. **Concluída na Sessão 27**: `generate_reply` (e os 3 `ask_*_api`) passam a devolver `(texto, TokenUsage)` em vez de só o texto, lendo `usageMetadata` (Gemini)/`usage` (Claude/OpenAI) reais de cada provedor — nenhuma chamada extra, nenhuma estimativa local. Persistido em `ai_message.input_tokens`/`output_tokens` (nova migration, só na mensagem do modelo, não na do usuário). Frontend mostra os dois níveis pedidos pelo usuário: total acumulado da conversa no cabeçalho do `ChatScreen.tsx`, e custo por mensagem no rodapé de cada resposta (`MessageTokenUsage` novo em `thread.tsx`, lendo `metadata.custom` que o `assistant-ui` já suporta nativamente). Widget flutuante não mudou — fora do escopo pedido
  - [x] 7.10.4 — **A IA passa a poder propor criar uma valuation nova** (não editar/apagar — escopo reduzido de propósito, perguntado direto ao usuário). **Concluída na Sessão 27**, planejada com `/plan` dedicado (3 agentes `Explore` + 1 `Plan` em paralelo, mais verificação real via `WebFetch` contra a doc de tool-calling dos 3 provedores). Nova tool genérica `propose_valuation` declarada nos 3 provedores (`chat.rs` ganhou `AiOutcome`/`ToolCall`, shape de tool diferente por provedor — Gemini `functionDeclarations`, Claude `input_schema`, OpenAI `tools[].function` com `arguments` como string). A IA nunca escreve sozinha: a proposta vira uma linha `ai_valuation_proposal` (nova tabela, `pending`/`approved`/`rejected`) presa à mensagem que a exibe, renderizada no chat como um card próprio (`ValuationProposalCard.tsx`, no estilo do `ValuationResult.tsx` manual) com botões Criar/Descartar — só o clique explícito do usuário roda a escrita real (`respond_to_valuation_proposal`, reaproveitando o despacho por-string-de-modelo já usado em `update_valuation`). Mecanismo de aprovação humana do `assistant-ui` (`onRespondToToolApproval`/`approval`) confirmado funcionando com `useExternalStoreRuntime` (não é exclusivo do `useLocalRuntime`). Decisões de simplicidade deliberadas: sem round-trip de volta pro modelo depois da aprovação (mensagem sintética "✅ Valuation criada..." em vez disso) e proposta inválida é beco sem saída sem retry automático. Achado real no meio da implementação: `GeminiPart.text` precisou virar `Option<String>` (resposta de function-call do Gemini não traz esse campo). Testado ponta a ponta pelo usuário no app real: proposta aprovada (valuation real criada no banco, conferido via `sqlite3`), proposta descartada (nada escrito), e reload da conversa confirmando que o card aprovado não volta a mostrar os botões (estado vem do banco). `cargo test --lib`: 77/77
  - [x] 7.10.5 — Troca de API/provider/modelo dentro da tela cheia — reusa a mesma ideia da 7.9.5. **Concluída na Sessão 26**, mesmo seletor de chave/campo de modelo livre, agora no cabeçalho da tela nova
  - [~] 7.10.6 — **Pesquisa real feita na Sessão 21** (via `WebSearch`/`WebFetch`, não por memória) — decisão de usar ou não ainda em aberto, **não decidir sozinho** (usuário pediu só a pesquisa nesta sessão, não a execução). Achados:
    - **`assistant-ui`** ([github.com/assistant-ui/assistant-ui](https://github.com/assistant-ui/assistant-ui)) — candidato forte. Lib React/TypeScript, MIT, ativa de verdade (11k stars, push no dia da pesquisa, 1094 forks, mantida por empresa YC — conferido via `gh api repos/assistant-ui/assistant-ui`). Suporta Gemini/Claude/OpenAI nativamente. **`useLocalRuntime`**: o adapter que fala com o "modelo" é só uma função async arbitrária, sem exigir servidor HTTP — encaixa direto no `invoke()` do Tauri, sem precisar de ponte/sidecar Node. Resolve de graça os dois pedaços mais arriscados da 7.10: tool-calling com pausa-pra-aprovação já é conceito de primeira classe (`status: "requires-action"`, mapeia direto no requisito da 7.10.4) e múltiplas conversas ("threads") com persistência 100% customizável (não depende do "Assistant Cloud" pago deles — a gente escreveria nosso próprio adapter salvando no SQLite, mesmo padrão de sempre). Tem hook de contador de token (`useThreadTokenUsage`), mas só funciona se `ask_claude_api`/`ask_openai_api`/`ask_gemini_api` passarem a extrair e devolver o uso de tokens da resposta (hoje só devolvem o texto) — isso continuaria sendo trabalho nosso independente da lib. Sem servidor de terceiro envolvido: é só mais um pacote no `package.json`, roda dentro do bundle do próprio app, sem SaaS no meio — só precisaria de trabalho de tema pra combinar com o visual atual (vem sem estilo pronto, mas tem variante shadcn/ui, que o projeto já usa)
    - **`prompt-kit`** ([prompt-kit.com/chat-ui](https://www.prompt-kit.com/chat-ui)) — alternativa mais enxuta: só componentes de UI (input, lista de mensagem, markdown, streaming) construídos em shadcn/ui + Tailwind, sem resolver conversas múltiplas nem aprovação de tool-call sozinho — mais "peças soltas" que framework completo
    - **LibreChat/Chatbot UI** (clones completos de ChatGPT) — descartados: são aplicações full-stack que esperam rodar servidor Node próprio, contra a arquitetura atual (tudo via Rust/Tauri, sem servidor Node)
    - Claude recomendou `assistant-ui` quando perguntado diretamente (a aprovação de ação é a parte mais arriscada da 7.10 e a lib já resolve isso testado por terceiros, sem risco de arquitetura) — usuário ainda não decidiu, só registrou a pesquisa
    - **Decisão tomada na Sessão 26**: perguntado diretamente, o usuário escolheu **`assistant-ui`** (framework) e escopar a sessão só no **alicerce** (7.10.1/7.10.2/7.10.5), deixando 7.10.3 (contador de token) e 7.10.4 (IA escrevendo valuations) pra sessões futuras dedicadas — 7.10.4 principalmente por já estar marcada acima como o maior risco técnico da fase, merecendo `/plan` próprio

**Sessão 26 (2026-07-21) — alicerce da 7.10 implementado**: planejado com `/plan` (3 agentes `Explore` em paralelo levantaram o widget flutuante existente incl. `ask_ai`/contexto de leitura do banco, o padrão de navegação `AppView`/`SettingsPage` e libs já instaladas, e os comandos de criação/edição de valuation como referência pro desenho futuro da 7.10.4). Pesquisa real contra a doc do `assistant-ui` (`www.assistant-ui.com/docs`, via `WebFetch`) antes de codar, não só memória da Sessão 21 — confirmou a API exata de `useExternalStoreRuntime`/`ChatModelAdapter`/`RemoteThreadListAdapter`.

- **Decisão de arquitetura**: usar `useExternalStoreRuntime` do `assistant-ui` (runtime dono só da conversa atual) em vez do `RemoteThreadListAdapter` (API maior, pensada pra multi-thread 100% no molde do framework). A lista de conversas (criar/selecionar/renomear/apagar) foi construída do jeito de sempre do projeto — CRUD fino em Rust + SQLite, mesmo padrão de `SettingsPage.tsx`/`SavedValuationsPanel.tsx` — deixando o `assistant-ui` responsável só pela parte que economiza trabalho de verdade (lista de mensagens, markdown, composer, auto-scroll, via o componente `Thread` escafoldado).
- **Backend**: migration nova (`ai_conversation`: título, `key_id` nullable com `ON DELETE SET NULL`, `model`, timestamps; `ai_message`: `conversation_id` com `ON DELETE CASCADE`, `role`/`content`/`created_at`) — `.schema` real conferido, sem o bug de coluna com dígito (nenhuma aqui termina em dígito). `ask_ai` (`commands/chat.rs`) refatorado: a lógica de resolver chave+montar contexto+chamar o provider virou `generate_reply` (função solta, não comando), reaproveitada pelos comandos novos — evita duplicar o `SYSTEM_REPERTOIRE`/contexto entre os dois pontos de entrada. `commands/conversation.rs` novo: `list_conversations`/`create_conversation`/`rename_conversation`/`delete_conversation`/`get_conversation_messages`/`send_conversation_message` (este último insere a mensagem do usuário, chama `generate_reply`, insere a resposta, e atualiza `key_id`/`model`/`updated_at` da conversa). `cargo check`/`cargo test --lib`: **68/68**, sem regressão.
- **Frontend**: `npm install @assistant-ui/react @assistant-ui/react-markdown remark-gfm zustand` + `npx shadcn@latest add @assistant-ui/thread` (escafoldou `thread.tsx` e componentes de apoio — `tooltip`/`avatar`/`collapsible`/`dialog`, `tw-shimmer` como dependência nova, tudo aditivo no `index.css`, tema dark+verde intacto). **Achado**: o `thread.tsx` escafoldado usa `Object.hasOwn` (ES2022) — `tsconfig.json` precisou subir de `target"ES2020"` pra `"ES2022"` (`lib` junto), sem impacto prático no WebKitGTK do Tauri. Pasta nova `desktop/src/chat-full/`: `ConversationSidebar.tsx` (lista+rename inline+delete com confirmação dupla, mesmo idioma de `SettingsPage`), `useConversationRuntime.ts` (monta o `useExternalStoreRuntime`, com atualização otimista da mensagem do usuário no cache do TanStack Query antes da resposta chegar — mesmo padrão do próprio exemplo mínimo da doc do `assistant-ui`), `ChatScreen.tsx` (layout sidebar+thread, seletor de chave/modelo igual ao `ChatPanel.tsx`). `App.tsx` ganhou a view `"chat"`. `npx tsc --noEmit` limpo.
- **Testado ponta a ponta pelo usuário no app real** (`./desktop/dev.sh`, rebuild do zero com as crates novas — `alloy-*`, `ark-*`, `secp256k1` etc. — baixadas e compiladas dentro do container): confirmado funcionando ("tá funcionando perfeitamente, mt massa"). Observação do próprio usuário: tem espaço pra melhorar UX, deixado pra depois, sem detalhar ainda.

---

### Fase 8 — Sync Multi-Dispositivo via TruthID + Arweave (ideia levantada na Sessão 11, desenho revisado na Sessão 22, replanejada pós-migração do TruthID pro Arweave na Sessão 87, não iniciada)

**Objetivo**: sincronizar valuations/alertas salvos entre dispositivos (celular, outro PC) de forma descentralizada — sem servidor próprio do Practice Valuation — reaproveitando a identidade e a infraestrutura já existentes no TruthID (outro projeto do usuário, `~/Documents/workspace/truthid`).

**Sessão 18 (2026-07-14/15) — prova de conceito do canal de assinatura delegada, feita a partir de
uma sessão do TruthID**: `desktop/src-tauri/src/commands/truthid.rs` (descobre o TruthID Desktop
rodando na mesma máquina, faz handshake, manda 1 pedido de assinatura de teste sem efeito
econômico) + aba "TruthID Sync" (`desktop/src/truthid/TruthIdPanel.tsx`). Prova que o canal HTTP
local do TruthID (`local_signer_server.rs`/`sign_request.rs`, portas 47950-47954) funciona — não é
a Fase 8 em si, é a base que ela usa.

**Sessão 22 (2026-07-15) — três suposições do brainstorm original corrigidas, desenho revisado
registrado (só documentação, nenhum código tocado)**: as ideias abaixo (Sessões 11-96, preservadas
por histórico logo depois) assumiam (1) um Paymaster no TruthID cobrindo o gas de apps terceiros —
não existe, descartado deliberadamente lá desde a Sessão 52; (2) generalizar o `VaultRegistry`
(`identityId → CID`) pra caber múltiplos apps — rejeitado duas vezes pelo dono do TruthID (Sessões
95 e 102): o vault de senhas não muda, cada app terceiro traz e mantém o próprio contrato; (3) uma
session key escopada pra aprovação de assinatura — também descartada (Sessão 102) em favor de algo
mais simples que **já foi construído e testado**: o canal de sign-request acima, aprovação humana
por clique único, sem escopo/sessão. Ver `PROJECT_STATE.md` do TruthID, seção "Vault genérico
multi-app...", pra o histórico completo da correção.

**Desenho revisado (Sessão 22)**:
1. **Registro de CID — contrato próprio do Practice Valuation**, não o `VaultRegistry` do TruthID. Mesmo formato de dado (`{cid, contentHash, version, updatedAt}`), mas gated por `msg.sender` (a smart account que chamou) em vez de `identityId` — como todo device do usuário assina pela mesma smart account, qualquer um escreve no mesmo slot sem precisar de conceito de identidade do TruthID no contrato em si.
2. **Escrita do CID — reaproveita o canal de assinatura delegada como já está** (`discover` → `handshake` → `POST /truthid/v1/sign-request`, `commands/truthid.rs` sem mudança de desenho). Cada dispositivo roda sua própria instância do TruthID local, assinando com a autoridade daquele device — não precisa de pareamento novo nem canal P2P entre devices.
3. **Leitura do CID — não precisa de assinatura**, é `eth_call` público via qualquer RPC.
4. **Chave de criptografia — depende de uma rota nova no TruthID, `/truthid/v1/sign-message`, ainda NÃO implementada** (pendência cross-repo, registrada no `PROJECT_STATE.md` do TruthID, Sessão 106). Mesmo padrão do `/sign-request`: app terceiro manda `{appName, purpose}`, TruthID monta uma mensagem padronizada com domain separation, assina via `personal_sign` com aprovação humana, devolve a assinatura — o app deriva a chave simétrica localmente (HKDF) sem o TruthID nunca saber pra que serve. **Decisão explícita do dono do projeto**: essa rota tem que ser genérica pra qualquer app terceiro, não um privilégio hardcoded pro Practice Valuation — o Practice Valuation é só o primeiro caso de uso real que força o desenho a existir.
5. **Pinning permanente — `/truthid/v1/pin`, capacidade genérica do TruthID pra qualquer app terceiro** (ideia trazida pelo dono do projeto na Sessão 22, também registrada no `PROJECT_STATE.md` do TruthID, Sessão 106; implementada do lado do TruthID nas Sessões 108-121 de lá). **Atualização, Sessão 87**: o TruthID migrou o backend inteiro dessa rota de IPFS pinning (`PinningProvider`/Kubo/Filebase/Pinata) pra **Arweave** (armazenamento pago-uma-vez-permanente, sem depender de nenhum provider configurado nem de manter um node/serviço pago no ar) — deletou por completo o subsistema de pinning IPFS do próprio TruthID. A rota (`/truthid/v1/pin`) e o formato de wire (`{appName, contentBase64} → {status, cid, contentHash, providersOk, providersFailed, error?}`) continuam idênticos por compatibilidade, mas `cid` agora carrega `ar://<tx_id>` em vez de um CID IPFS — quem ler esse campo precisa tratar como ponteiro Arweave, não IPFS. Aprovação continua por chamada, sem quota/autorização persistida (bate com o que já estava assumido aqui). **O "fallback com `PinningProvider` próprio" abaixo foi descartado** — o próprio TruthID chegou à mesma conclusão pelo mesmo motivo (esse modelo não combina com armazenamento permanente pago-uma-vez): ~~módulo próprio no Practice Valuation reaproveitando a mesma abstração `PinningProvider{name, kind: "kubo"|"psa", endpoint_url, api_key}` + `pin_vault()` do TruthID (cópia adaptada do padrão, crates separadas), Pinata (PSA, free tier) como default, Kubo local como opção~~ — risco aceito de depender só do TruthID pra essa capacidade, mesmo raciocínio já aceito pra `/sign-message`/`/sign-request`.
6. **Correção feita ainda na Sessão 22, depois de uma pergunta do dono do projeto — o desenho acima (itens 1-5) só funciona quando o Practice Valuation e o TruthID rodam na MESMA máquina**, porque `local_signer_server.rs` escuta estritamente em `127.0.0.1` (nunca a rede), por desenho de segurança do TruthID. Cenário real que quebra isso: Practice Valuation só no computador, TruthID só instalado no celular — hoje **não existe canal nenhum** entre os dois nesse caso. **Não é mais tratado como resolvido** (uma versão anterior desta seção, ainda na Sessão 22, chegou a marcar essa questão como "não é mais necessária" — estava errada, corrigida ainda na mesma sessão). Fica **em aberto**, com um caminho claro pra resolver: o TruthID já tem esse problema resolvido pra outro caso de uso (extensão de navegador, Fase 13.9 dele) — dois transportes, tentados em paralelo:
   - **Descoberta na mesma rede local (LAN)**: `mobile/lib/services/vault_lan_server_service.dart` sobe um servidor HTTP efêmero (portas 47850-47854) que serve exatamente 1 request e fecha.
   - **Dead-drop assíncrono via IPFS/IPNS**: funciona em redes diferentes, mas com propagação mais lenta (~1-2min) — o dispositivo com o TruthID publica um blob cifrado sob um nome IPNS derivado deterministicamente do QR; o outro lado recalcula o mesmo nome e resolve. **Confirmado na Sessão 87**: esse transporte continua em IPFS/IPNS de propósito, mesmo depois do TruthID migrar o *armazenamento permanente* pro Arweave — Arweave não tem conceito de ponteiro mutável/efêmero de curto TTL, que o dead-drop precisa; IPFS/IPNS segue sendo a ferramenta certa só pra essa peça (`ipns_key.rs`/`dead_drop.rs` do Anchor continuam válidos sem mudança nenhuma).
   - **Segurança confirmada no código real, não é "qualquer dispositivo na rede consegue"**: o QR carrega um `sessionId` de 128 bits (imprevisível, não adivinhável varrendo a rede), o servidor LAN devolve 404 uniforme pra qualquer path errado (não vaza sinal de "quase certo"), e o payload em si é cifrado via ECIES pra uma chave pública efêmera que só existe no QR — quem não escaneou o QR de verdade (proximidade física/visual) não consegue nem achar nem decifrar o blob, mesmo estando na mesma rede. TTL curto (3min) e resposta única (o servidor atende 1 request e morre) limitam a janela de exposição.
   - **Ressalva honesta**: essa mesma peça de criptografia (ECIES no pareamento) teve um bug real que passou batido por várias sessões do TruthID antes de ser achado testando contra hardware de verdade (Sessão 99 de lá) — não é motivo pra não usar o padrão, mas reforça que validar isso em hardware real antes de confiar é obrigatório, não opcional.
   - **Ainda não decidido**: estender as rotas genéricas (`/sign-message`, `/pin`, e talvez até `/sign-request`) pra também aceitar esses dois transportes (hoje só funcionam loopback) é trabalho novo do lado do TruthID, não desenhado em detalhe — fica registrado como pendência em aberto, junto com as outras duas rotas (Sessão 106 do `PROJECT_STATE.md` do TruthID).

**Fatias propostas (cada uma uma sessão futura, mesmo padrão das outras fases)**:
- [x] 8.1 — contrato próprio (deploy + leitura pública). **Deployado de verdade em Base Sepolia na Sessão 30**, via Ledger física (`cast wallet address --ledger --mnemonic-derivation-path "m/44'/60'/1'/0/0"`, mesmo deployer dos contratos do TruthID, `0xB54fe9909D76d98e87a9fD76bDB5C69fABe10265`), simulação (`forge script` sem `--broadcast`) revisada com o dono do projeto antes do broadcast real. Endereço: `0x9e2fd1a4146e3c40ecfef9d35a09e75e00ac30e8`, tx `0x52b347795c113a30cfac614497d43c62d1efce6a3c2b3cb7990d2a888aa4bfae` (`status: 0x1`, custo real ~0,0000107 ETH). `SYNC_REGISTRY_ADDRESS` em `sync_registry.rs` atualizado do placeholder zerado pro endereço real; `cast call ... hasRecord(address)` confirmou o contrato vivo antes de qualquer código novo ser escrito contra ele. **Redeployado em Base Mainnet na Sessão 87** (decidido na mesma sessão, ver bloco "Sessão 87" no fim desta fase): mesmo script/deployer (Ledger, `0xB54fe9909D76d98e87a9fD76bDB5C69fABe10265`), simulação revisada antes do broadcast real (custo estimado ~0,0000098 ETH contra saldo de ~0,00859 ETH em Mainnet). Novo endereço: `0x778771ba4a7a2e867385980f1616602a1a175115`, tx `0xbb795effb8e6f2ed8739b49184cb6a37cdbb7c4a007c022665cd545339c6fdec` (`status: 0x1`, gas real 750565 — mais barato que estimado). `SYNC_REGISTRY_ADDRESS`/`RPC_URL` em `sync_registry.rs` atualizados pro endereço/RPC de Mainnet; `cast call ... hasRecord(address)` confirmou o contrato vivo em Mainnet antes de qualquer uso. O endereço antigo em Sepolia (`0x9e2fd1a4146e3c40ecfef9d35a09e75e00ac30e8`) nunca recebeu escrita real (todo `/sign-request` contra ele sempre executou em Mainnet, no-op silencioso) — fica só como histórico, não precisa de migração de dado nenhuma.
- [x] 8.2 — escrita via canal de assinatura delegada já existente, só o caso "TruthID na mesma máquina" (cross-device fica pra depois). **Implementada e validada ponta a ponta na Sessão 30**: `sync_registry.rs` ganhou `updateRecord` no bloco `sol!{}`, `build_update_record_calldata`/`parse_content_hash` (3 testes novos, unitários offline); `commands/truthid.rs::update_sync_record` reaproveita o `discover()` já existente e monta o mesmo formato de `/truthid/v1/sign-request` que `send_test_sign_request` já usava, trocando `dest`/`callData`/`functionSignature` pelos reais (`updateRecord(string,bytes32)`); nova seção "Fase 8.2 — Update sync record" em `TruthIdPanel.tsx` (CID/content hash inseridos manualmente por ora, já que 8.3/8.4 ainda não geram isso de verdade). `cargo check`/`cargo test --lib` (86/86, 3 novos) e `npx tsc --noEmit` limpos. **Testado pelo usuário no app real** (TruthID Desktop + Practice Valuation rodando juntos via Docker, CID/hash de teste, aprovado no TruthID — "funcionou").
- [~] 8.3 — pinning via `/truthid/v1/pin`. **Implementada e parcialmente validada na Sessão 32 (2026-07-28)**: novo comando `commands/truthid.rs::pin_database_snapshot` lê os bytes reais do arquivo SQLite atual (`db::DATABASE_FILE_PATH`, novo `pub(crate) const`), manda em base64 pro `/truthid/v1/pin` (`AppError::Io` novo, `#[from] std::io::Error`, pro erro de leitura do arquivo); nova seção "Fase 8.3 — Pin database snapshot" em `TruthIdPanel.tsx`, com botão "Use this result in Update sync record" que preenche os campos da seção 8.2 automaticamente a partir do `cid`/`content_hash` devolvidos, fechando o loop 8.3→8.2→8.1 sem copiar/colar manual. `cargo check`/`cargo test --lib` (86/86) e `npx tsc --noEmit` limpos. **Achado no meio do caminho**: rodar TruthID Desktop e Practice Valuation ao mesmo tempo via Docker quebrava — os dois `docker-compose.yml` usam `network_mode: host` e os dois projetos tinham Vite/Tauri fixados na mesma porta (1420, `strictPort: true`); quem "ganhava" o bind servia conteúdo pros dois, então a janela do Practice Valuation chegou a mostrar a UI do TruthID. Corrigido só do lado deste repo (não mexeu no TruthID): porta de dev do Practice Valuation movida pra 1430 (`vite.config.ts` + `tauri.conf.json::devUrl`). **Testado pelo usuário no app real**: com um node Kubo local subido (`systemctl --user start ipfs`, provider já configurado em `~/.truthid/pinning_providers.json`), o pin funcionou de verdade (aprovado no TruthID, `status: pinned` com CID/hash reais) e o update subsequente também foi aprovado e executado no TruthID (`status: executed`). **Suspeita de rede errada, CONFIRMADA na Sessão 33 (2026-07-29), sem precisar repetir o teste ao vivo — bastou ler o código do TruthID**: `~/.truthid/bundler_config.json` tem `{"network": "base"}`; `desktop/src/services/pimlicoBundlerClient.ts` interpola esse valor direto na URL do bundler Pimlico (`https://api.pimlico.io/v2/base/rpc?apikey=...`); `desktop/src/services/userOpExecutor.ts:30` tem `const CHAIN_ID = 8453n` (Base Mainnet) **hardcoded**, usado tanto pro bundler quanto pra computar o hash/assinar a UserOperation. O `SignRequestBody` que `/sign-request` recebe (`sign_request.rs`) só tem `dest`/`value`/`callData`/`functionSignature` — **nenhum campo de chain/network que um app terceiro possa usar pra pedir outra rede**. O check `chainId !== base.id` em `App.tsx` é outra coisa (só valida a rede da carteira externa/Ledger conectada), não tem relação com o bundler. **Conclusão**: todo `/sign-request` do TruthID hoje executa sempre em Base Mainnet, não importa o `dest` pedido. O update da 8.3 (e quase certamente o da 8.2 na Sessão 31 também, mesma arquitetura) foi assinado/executado em Mainnet contra `0x9e2fd1a4146e3c40ecfef9d35a09e75e00ac30e8`, endereço sem código lá — por isso a UserOp "deu certo" (chamar endereço sem código é sempre um no-op bem-sucedido na EVM) mas não gravou nada no `SyncRegistry` real (só existe em Base Sepolia); gastou um pouco de ETH real de mainnet à toa. **Isto não é bug deste repo** — é uma limitação real, cross-repo, do canal `/sign-request` do TruthID: falta um mecanismo de seleção de rede por chamada antes que ele possa ser usado com segurança por qualquer contrato fora de Base Mainnet. **8.3 permanece bloqueada** até o TruthID ganhar esse mecanismo (decisão explícita do dono do projeto na Sessão 33: só documentar por ora, sem desenhar/implementar a correção do lado do TruthID ainda). Vale reconferir a validação da 8.2 quando isso for retomado, já que ela também só foi confirmada pela aprovação do TruthID, nunca por releitura bem-sucedida. **Atualização, Sessão 87**: investigação confirmou que esse bloqueio de chain segue exatamente como descrito acima, sem mudança nenhuma da migração pro Arweave — mas o pinning em si (a parte que a rota `/truthid/v1/pin` faz) passou a ser backed por Arweave em vez de IPFS, então **8.3 deixa de estar bloqueada pelo pinning** — o único bloqueio restante é o de chain (resolvido pela decisão de redeploy em Mainnet, ver bloco "Sessão 87"). Ver também o achado de segurança novo (dados pinados hoje vão crus, sem cifra — ver 8.4).
- [x] 8.4 — criptografia real via `/truthid/v1/sign-message`. **Deixou de estar bloqueada** — a rota e os dois transportes (LAN+dead-drop) já existem do lado do TruthID (Sessões 108-109 de lá); ainda não consumida aqui. **Prioridade elevada na Sessão 87**: `pin_database_snapshot`/`push_pin_content` (`commands/truthid.rs`) hoje mandam os bytes do SQLite **crus, sem cifra** pro `/truthid/v1/pin` — a cifra que existe (ECIES/simétrica) só protege o transporte cross-device, nunca o dado em repouso. Com o backend agora em Arweave (permanente, público, irreversível — diferente de um pin IPFS que podia ao menos ser removido/expirar), **8.4 vira pré-requisito obrigatório antes de qualquer pin real de dado de portfólio**, não mais uma fatia adiável à vontade. **Implementada ainda na Sessão 87 (continuação), os dois caminhos juntos** (loopback e cross-device, decisão explícita do dono do projeto via `AskUserQuestion` — escopo maior que o padrão "loopback primeiro" da Fase 8.2, aceito porque cobre o pré-requisito de segurança dos dois fluxos de uma vez): novo `sync_snapshot_cipher.rs` (chave AES-256 via HKDF-SHA256 sobre os bytes de uma assinatura `personal_sign` — determinística, então pedir de novo com a mesma `purpose` sempre deriva a mesma chave, nenhum segredo persistido pelo Anchor); `pin_database_snapshot` (loopback) passa a pedir `/sign-message` antes do `/pin` (2 aprovações sequenciais no TruthID atrás de 1 clique); `push_pin_content` (cross-device) ganha um novo fluxo cross-device próprio de `/sign-message` (`create_cross_device_sign_message_request`/`await_cross_device_sign_message_response`, schema do QR — `action: 'truthid-sign-message'` — confirmado lendo o código real do TruthID Mobile, `sign_message_approval_screen.dart`, não adivinhado) e cifra em **duas camadas**: `sync_snapshot_cipher` (at rest, chave da assinatura) por dentro, `pin_content_cipher` (transporte, chave da sessão do QR) por fora — o celular só decifra a camada de transporte e pina o blob ainda cifrado, sem nunca ver a chave real. UI (`TruthIdSettingsSection.tsx`) ganhou um 2º QR no fluxo cross-device (1/2 assinatura, 2/2 pin) e teve o rótulo incorreto "Fase 8.4" tirado do bloco de pin cross-device (era na verdade extensão da 8.6, correção já registrada na Sessão 40) — de caminho corrigido também o texto desatualizado "Fase 8.1 — SyncRegistry (Base Sepolia)" pra "(Base Mainnet)", que tinha ficado pra trás no redeploy desta mesma sessão. `cargo test --lib` **173/173** (3 novos, `sync_snapshot_cipher`), `tsc --noEmit` limpo.
  - **Achado real testando ao vivo (TruthID Desktop subido via Docker pra validar)**: 1ª tentativa falhou com `error decoding response body` no `/truthid/v1/pin` — não era bug desta fatia, era um limite pré-existente do lado do TruthID (`local_signer_server.rs` monta o `Router` do axum sem nenhum `DefaultBodyLimit`, então vale o padrão de 2MB). O banco de dev do Anchor já tem 9,3MB — cifrado e em base64 vira ~12,5MB, bem acima do limite, rejeitado com 413 em corpo texto puro (não JSON) antes de qualquer aprovação pro `/pin` aparecer. **Corrigido no TruthID** (repo separado, mesmo dono, aprovado por ele antes de mexer): `.layer(DefaultBodyLimit::max(128 * 1024 * 1024))` em `local_signer_server.rs`, `cargo check`/`cargo test --lib local_signer_server` conferido (14/15 — 1 falha pré-existente e não relacionada, confirmada via `git stash`/reteste que já falhava antes desta mudança, registrada como achado à parte, não corrigida aqui). Reteste real: as 2 aprovações aconteceram, `/pin` devolveu um `PinResult` bem formado com `status: "failed"` e `error: "nenhuma wallet Arweave configurada — gere ou importe uma antes de usar o /pin"` — confirma que a fatia 8.4 do Anchor está correta de ponta a ponta; o que falta pra validar com sucesso é puramente operacional do lado do TruthID (gerar/importar uma wallet Arweave), fora do escopo deste repo.
- [ ] 8.5 — integração fim a fim: publish/pull automático, merge por replay causal (`valuation`/`alert_event` já são append-only, formato natural pra isso sem precisar desenhar CRDT do zero). Não iniciada
- [x] 8.6 — suporte a "TruthID num dispositivo diferente" via LAN/dead-drop, no molde da 13.9 do TruthID. **Validado ponta a ponta pro par `/sign-request`** na Sessão 23 (celular físico real). **Estendido pro `/pin` na Sessão 39 (2026-08-01)**: novo `pin_content_cipher.rs` (HKDF+AES-256-GCM simétrico, chave derivada do `sessionId` do QR — nem app nem celular tem a chave pública um do outro nessa direção, diferente do ECIES usado no resultado) + lado PUT em `lan_sweep.rs` (`sweep_push_once`, espelha `RemoteSignerLanServer.receiveOnce` do Mobile) + 3 comandos Tauri (`create_cross_device_pin_request`/`push_pin_content`/`await_cross_device_pin_response`) + seção nova em `TruthIdPanel.tsx`. **Nota de rótulo**: o commit (`4e66ce4`) chamou isso de "Fase 8.4" no título, mas pelo desenho documentado aqui é claramente uma extensão da 8.6 (transporte cross-device) aplicada ao `/pin` — a 8.4 de verdade (criptografia via `/sign-message`, linha abaixo) continua não iniciada. Corrigido nesta entrada (Sessão 40, 2026-08-02) só na documentação, sem mexer no código/histórico do git. O mesmo transporte pro par `/sign-message` (Sessões 108-109 do TruthID) ainda não foi exercitado por nenhum app requisitante de verdade — só quando a 8.4 de fato consumir `/sign-message` isso fica provado igual

8.1 e 8.2 fechados na Sessão 31; 8.3 com código pronto e pin real validado na Sessão 32. **Novo bloqueio cross-repo descoberto e confirmado na Sessão 33 (ver nota em 8.3 acima)**: o canal `/sign-request` do TruthID executa toda UserOperation em Base Mainnet, hardcoded, sem jeito de pedir outra rede por chamada — então nenhuma escrita via esse canal contra um contrato fora de Mainnet (como o `SyncRegistry`, em Base Sepolia) funciona hoje. Diferente dos dois bloqueios cross-repo anteriores (`/sign-message`, `/pin`, resolvidos entre 2026-07-16 e 07-17), este ainda não tem solução do lado do TruthID — precisa ganhar um mecanismo de seleção de rede por chamada antes de 8.2/8.3 poderem ser considerados de fato funcionais. Decisão do dono do projeto na Sessão 33: por ora só documentar, sem desenhar a correção. **Atualização, Sessão 87 (2026-08-23)**: confirmado por leitura direta do código do TruthID que esse bloqueio de chain **continua idêntico, sem nenhuma mudança** — não foi tocado pela migração pro Arweave (são subsistemas ortogonais: chain/assinatura de UserOp vs. backend de armazenamento). Decisão nova do dono do projeto: em vez de continuar esperando o TruthID ganhar seleção de chain por chamada, o `SyncRegistry.sol` do Anchor vai ser **redeployado em Base Mainnet** — contrato próprio, custo real irrisório em L2, destrava 8.2/8.3/8.5 sem depender de nada cross-repo. **Executado ainda na Sessão 87 (continuação, mesmo dia)** — ver nota na fatia 8.1 acima: bloqueio de chain **resolvido**, 8.2/8.3 (na parte de escrita on-chain) deixam de estar bloqueados a partir de agora.

**Sessão 87 (2026-08-23) — replanejamento pós-migração do TruthID pro Arweave, só documentação, nenhum código tocado**: dono do projeto avisou que o TruthID mudou bastante desde a última vez que esta fase foi tocada, "agora utilizamos o arweave". Investigação de verdade feita com 3 agentes `Explore` em paralelo contra o repo do TruthID (`~/Documents/workspace/truthid`) — histórico/`project/` dele, docs públicos (`site/frontend/content/docs/`, Fumadocs — o antigo `docs/` Docusaurus está deprecado e não é mais buildado) + os 4 SDKs (TS/Python/Ruby/Dart), e o código real que fala com Arweave (`desktop/src-tauri/src/arweave/`, espelhado em Dart no Mobile) — cruzados com leitura direta do código atual do Anchor (`commands/truthid.rs`, `sync_registry.rs`). Achados que mudam o desenho desta fase, todos já refletidos inline acima:
1. **Só o backend de armazenamento permanente trocou** (IPFS pinning → Arweave, um cliente Arweave feito à mão em Rust/Dart, sem SDK de terceiros — `bundles-rs`, a única lib Rust mantida, só faz ANS-104/bundler, incompatível com a trilha self-sovereign do TruthID). Identidade/dispositivos/sessões/recovery/o *ponteiro* do Vault continuam 100% em contratos Base, inalterados em assinatura — só o *conteúdo* do campo `cid` mudou de forma (`ar://<tx_id>` em vez de CID IPFS), já que sempre foi uma string opaca (zero migration de contrato do lado deles).
2. `/truthid/v1/pin` continua com a mesma rota/wire-format, backend trocado — ver nota na fatia 8.3 acima.
3. TruthID **deletou o próprio subsistema de pinning IPFS** (`PinningProvider`/Kubo/Filebase/Pinata) — motivou descartar o fallback próprio que a fatia 5 do "Desenho revisado" tinha registrado (ver acima).
4. **Nenhum SDK Rust existe** no TruthID (só TS/Python/Ruby/Dart), e o único papel de "app terceiro pedindo algo" (`TruthIDRequester`) só existe no SDK Dart — confirma que a abordagem que o Anchor já usa (cliente HTTP à mão contra o `local_signer_server` loopback, `commands/truthid.rs`) continua sendo o jeito certo, não ficou obsoleta por causa de nenhum SDK novo.
5. Leitura de conteúdo Arweave é um `GET` público direto em `https://arweave.net/{tx_id}`, sem TruthID no meio — mais simples que o gateway IPFS que seria necessário antes.
6. **O bloqueio de chain (fatia 8.3) é anterior ao Arweave e não foi tocado** — ver nota na fatia 8.3 e no parágrafo acima.
7. **Achado de segurança novo**, elevando a prioridade da fatia 8.4 — ver nota na fatia 8.4 acima.
8. TruthID documenta o próprio armazenamento via Arweave como "code-complete" mas **nunca provado com uma wallet financiada de verdade em Mainnet** — risco operacional do lado deles, não bloqueia o Anchor, só vale saber.

Nenhuma fatia nova foi implementada na parte inicial desta sessão — o entregável ali foi só a atualização desta fase (`PHASE.md`) e do `ROADMAP.md`. **Continuação, mesmo dia**: fatias 1 (redeploy) e 2 (Fase 8.4) da fila abaixo foram implementadas de verdade — ver notas nas fatias 8.1 e 8.4. Fila de próximas fatias, pra sessões futuras, aos poucos: ~~(1) redeploy do `SyncRegistry.sol` em Base Mainnet~~ **feito**; ~~(2) Fase 8.4 (cifra real via `/sign-message`, HKDF)~~ **feito, os dois caminhos (loopback + cross-device)** — validado até onde o Anchor controla, falta só o TruthID ganhar uma wallet Arweave configurada (operacional, fora deste repo); ~~(3) branch de leitura `ar://` no lado do Anchor~~ **feito na Sessão 98 (2026-09-13)** — ver bloco "Sessão 98" no fim desta fase; (4) revalidação ponta a ponta real (pin cifrado → `updateRecord` em Mainnet → `getRecord` → fetch Arweave → decifrar → conferir integridade) — bloqueada até a wallet Arweave do TruthID existir; (5) só depois, Fase 8.5 (loop completo de sync).

**Sessão 98 (2026-09-13) — fatia 3 da fila implementada: branch de leitura `ar://`**. Dono do
projeto escolheu adiantar essa fatia mesmo com a 4 (revalidação ponta a ponta) ainda bloqueada
externamente, deixando o código pronto pro dia em que a wallet Arweave do TruthID existir. Novo
módulo `desktop/src-tauri/src/arweave.rs` (`fetch_content(tx_id) -> Result<Vec<u8>, AppError>`,
`GET https://arweave.net/{tx_id}` público, sem TruthID no meio — leitura Arweave não exige
assinatura). `sync_registry.rs` ganhou `parse_arweave_tx_id(cid)` (extrai o `tx_id` de
`ar://<tx_id>`, único formato possível já que o registro nunca teve escrita real antes da migração
pro Arweave). `sync_snapshot_cipher.rs` promoveu o `decrypt_for_test` privado (só usado em teste)
pra `pub fn decrypt` de verdade — a tag de autenticação do AES-256-GCM na decifra é a prova
criptográfica real de integridade (chave errada ou conteúdo adulterado falham aqui,
independentemente do hash on-chain bater). `commands::truthid::derive_sync_snapshot_key_loopback`
virou `pub(crate)` pra ser reaproveitado pelo novo comando
`commands::sync_registry::pull_and_verify_sync_snapshot(address)`, que: lê o `CidRecord` on-chain
→ busca o conteúdo Arweave → confere o hash on-chain do conteúdo bruto (`raw_content_hash_matches`,
informativo — detecta gateway servindo conteúdo errado/desatualizado) → decifra com a mesma chave
determinística da Fase 8.4 (erro real se falhar, já que é a prova criptográfica de verdade) →
devolve um resumo (`tx_id`, registro, os dois booleans/tamanho) **sem nunca expor os bytes
decifrados pro frontend** — decisão explícita de escopo: essa fatia é só busca+decifra+verificação,
não restaura/sobrescreve o banco SQLite local (isso é trabalho da Fase 8.5, merge por replay
causal, que ainda nem começou — aplicar um snapshot puxado de forma ingênua antes do merge existir
seria destrutivo e prematuro). UI: seção nova "Pull & verify snapshot" em
`TruthIdSettingsSection.tsx`, logo depois do bloco "Read sync record" (Fase 8.1), mesmo padrão
`useMutation`. 2 novos `AppError` (`Arweave`, `Decryption`). `cargo test --lib` **200/200** (5
testes novos — `parse_arweave_tx_id` aceita/rejeita, `decrypt` round-trip + rejeita chave
errada/ciphertext adulterado/blob curto demais), `tsc --noEmit` limpo. **Não dá pra validar o
caminho de sucesso real** (buscar+decifrar um snapshot de verdade) ainda — não existe nenhum
conteúdo pinado real (bloqueado pela wallet Arweave do TruthID, fatia 4); fica pendente pra quando
isso existir.

**Sessão 23 (2026-07-16) — Practice Valuation vira o app requisitante de referência do transporte cross-device do `/sign-request`, fecha parte da 8.6**: até aqui, o canal de assinatura delegada (`commands/truthid.rs`) só tinha sido provado com TruthID Desktop na mesma máquina (Sessão 18) — nenhum app terceiro real tinha gerado o QR nem consumido o transporte LAN/dead-drop que o TruthID lado Mobile já expunha (Sessões 108-111 de lá, pro par `/sign-message`+`/sign-request`). Três commits, mesmo dia:
- `2db1eab` — Practice Valuation passa a gerar o QR (par de chaves ECIES efêmero + `sessionId` de 128 bits) e varrer a LAN nas mesmas portas que `RemoteSignerLanServer` do TruthID Mobile usa. Novos `ecies.rs` (porta do decrypt ECIES já testado em Rust no TruthID Desktop) e `lan_sweep.rs` (porta da varredura já em TS na extensão do TruthID).
- `2e81c52` — segundo transporte em paralelo: dead-drop assíncrono via IPFS/IPNS (`ipns_key.rs` recalcula o nome IPNS determinístico a partir do `sessionId`, HKDF+Ed25519+protobuf+CID+base36 — mesmo algoritmo já usado em Dart/TS do lado do TruthID; `dead_drop.rs` consulta o gateway público `ipfs.io`). LAN tenta a cada 2s, dead-drop a cada ~20s (bater num gateway público a cada 2s seria agressivo, e a propagação de IPNS já leva 1-2min) — o primeiro que responder decide.
- `81d77ac` — bugfix achado testando com celular físico de verdade (Samsung SM_S731B, TruthID Sessão 114): `rename_all = "camelCase"` em `TruthIdSignResult` resolvia a leitura do JSON que chega de fora mas quebrava a serialização de volta pro frontend Tauri/JS (que espera snake_case, mesmo padrão de `TruthIdHandshakeResult`). Corrigido separando em `TruthIdWireResult` (Deserialize, camelCase, só parsing interno) convertido via `From` pra `TruthIdSignResult` (Serialize, sem rename_all). Sem teste automatizado cobrindo isso — os campos são `Option<T>`, então uma chave ausente nunca dá erro, só vira `None` em silêncio.

Isso fecha, do lado do TruthID (Sessão 114 de lá), **a pendência "nenhuma troca ponta a ponta real foi observada"** que estava aberta desde a Sessão 108 — a primeira troca real com um app terceiro de verdade e um celular físico aconteceu nesta sessão. **O que ainda não mudou neste repo**: nenhum contrato de CID foi deployado (8.1), a chave de criptografia derivada de `/sign-message` ainda não é consumida por nenhum fluxo real de sync (8.4), e não há pinning (8.3) — só a camada de transporte/canal foi validada. Vale notar que, **do lado do TruthID** (fora deste repo, sessões 108-121, todas em 2026-07-16/17), tanto `/sign-message` quanto `/sign-request` já têm os dois transportes (LAN + dead-drop) prontos, e `/truthid/v1/pin` também já foi implementado nas 3 fatias (núcleo Rust, rota HTTP + comandos Tauri, tela de Settings) — ou seja, **as duas pendências cross-repo que bloqueavam 8.3/8.4/8.6 não existem mais**; falta só este repo de fato começar a consumi-las (8.1 contrato próprio, 8.3 integração real com `/pin`, 8.4 integração real com `/sign-message`).

**Sessão 24 (2026-07-21) — fatia 8.1 implementada (contrato + leitura), planejada com `/plan` (3 agentes `Explore` em paralelo levantaram a arquitetura Rust existente, as crates web3/crypto já presentes — nenhuma — e o `VaultRegistry` do TruthID como referência)**:
- **Debate antes de codar**: o dono do projeto perguntou se fazia mais sentido o TruthID hospedar um contrato genérico pra qualquer app terceiro, em vez de cada um trazer o próprio. Argumento contra centralizar, aceito: `/sign-message`/`/pin` precisam ser genéricos no TruthID porque dependem de um recurso que só ele tem (chave de device, credenciais de pinning); um contrato de registro não depende de nenhum segredo do TruthID — qualquer smart account chama qualquer contrato deployado, não importa quem fez o deploy. Além disso, um registro único-por-endereço compartilhado por vários apps colidiria (o mesmo endereço usado em apps diferentes sobrescrevendo o CID um do outro) sem uma dimensão extra de `appId` — complexidade que contratos por-app não têm, já que cada contrato já é o próprio namespace. Mesmo raciocínio, no fundo, da rejeição de generalizar o `VaultRegistry` (Sessões 95/102 do TruthID). Mantido o desenho original: contrato próprio, simples.
- **`contracts/` novo na raiz do repo** (Foundry, `forge-std` instalado como arquivos normais via `forge install --no-git`, não submodule — mesmo padrão do TruthID): `src/SyncRegistry.sol` (adaptado do `VaultRegistry.sol` do TruthID, mas **sem indireção de identidade** — gated direto por `msg.sender`, já que este contrato não depende de um `IdentityRegistry` próprio nem do TruthID). Struct `CidRecord{cid, contentHash, updatedAt, version, exists}`, `updateRecord`/`getRecord`/`hasRecord`, evento `RecordUpdated`. **Decisão deliberada**: `updateRecord` (escrita) já entra no contrato nesta fatia, mesmo a 8.1 sendo "só leitura" do lado do app — contratos não são upgradeable aqui, e redeployar só pra acrescentar a escrita na 8.2 seria desperdício de endereço/histórico. `test/SyncRegistry.t.sol` (10 testes, mesmo estilo `vm.prank`/`vm.expectRevert`/`vm.expectEmit` do `VaultRegistry.t.sol`) — `forge test`: **10/10 passando**. `script/DeploySyncRegistry.s.sol` pronto, mas **deploy real ainda não rodado** — passo manual combinado com o dono do projeto, que quis olhar o contrato de novo antes de rodar o `--broadcast` (gasta ETH de testnet). Foundry já estava instalado no host (mesma versão do TruthID, v1.7.1) — nenhuma mudança no Docker do `desktop/` foi necessária.
- **Decisão de ABI encoding, perguntada ao dono do projeto**: adicionar `alloy-primitives`+`alloy-sol-types` (v1.6.1) em vez de codar o encode/decode ABI à mão (como `ipns_key.rs` fez pra CID/protobuf) — projeto ainda não tinha nenhuma lib Ethereum no Rust. Escolhido `alloy` (moderna, sucessora do `ethers-rs` que está em manutenção) porque a macro `sol!` gera o encode/decode a partir da própria assinatura Solidity, cobrindo também a Fase 8.2 (escrita) de graça.
- **`desktop/src-tauri/src/sync_registry.rs`** (novo módulo, mesmo nível de `ecies.rs`/`lan_sweep.rs`/`ipns_key.rs`): `sol! { struct CidRecord {...} function getRecord(address) ...; error RecordNotFound(address); }` gera o encode/decode; `get_record(who)` monta o `eth_call` via `reqwest` (já dependência do projeto) contra `RPC_URL = "https://sepolia.base.org"` (pública, sem chave — mesmo endpoint que o ecossistema TruthID já usa como fallback) e um endereço de contrato ainda placeholder (`0x000...0`, preenchido só depois do deploy real). Revert `RecordNotFound` vira `Ok(None)`, não erro — "ainda não tem registro" é resultado esperado. 4 testes novos (`cargo test`): calldata tem seletor+endereço no formato certo, round-trip de encode/decode de um `CidRecord` completo, `parse_address` aceita/rejeita o esperado — **68/68 no total, sem regressão**.
- **`commands/sync_registry.rs`**: comando fino `get_sync_record(address) -> Option<CidRecordResponse>`, seguindo o padrão de `commands/bazin.rs`. `CidRecordResponse` é um tipo próprio (não o `CidRecord` gerado pela macro, que não implementa `Serialize` e usa tipos ABI como `U256`/`FixedBytes<32>`) — mesma razão do split `TruthIdWireResult`/`TruthIdSignResult` já usado em `commands/truthid.rs`.
- **Frontend**: `TruthIdPanel.tsx` ganhou uma seção nova (campo de endereço + botão "Read sync record"), mesmo padrão `useMutation`/`invoke` do resto do painel — só o suficiente pra provar a leitura visualmente quando o contrato existir de verdade; uma aba "Sync" própria fica pra quando 8.2/8.5 também existirem.
- **Validado sem o deploy**: `cargo check`/`cargo test --lib` (68/68) e `npx tsc --noEmit` limpos; `curl` direto contra `https://sepolia.base.org` confirmou o RPC respondendo (chain id `0x14a34` = 84532, Base Sepolia) e que um `eth_call` pro endereço placeholder (`0x0`) retorna `"0x"` (sem contrato, sem revert) — exatamente o caminho que o código já trata como erro de decodificação (`AppError::Rpc`), não um crash.
- **Pendência pra próxima sessão**: dono do projeto vai revisar o `SyncRegistry.sol` de novo antes do deploy real em Base Sepolia (`forge script script/DeploySyncRegistry.s.sol --broadcast`); depois disso, atualizar `SYNC_REGISTRY_ADDRESS` em `sync_registry.rs` e validar a leitura ponta a ponta (escrever 1 registro via `cast send`, ler pelo botão novo no app rodando).

<details>
<summary>Brainstorm original (Sessões 11-96), preservado por histórico — suposições corrigidas acima, não usar como referência de desenho</summary>

- **P2P direto entre devices foi descartado** — usuário não gosta da abordagem (conexão ao vivo entre dispositivos, NAT, complexidade). Transporte vira **assíncrono via IPFS**: cada device publica quando muda algo, os outros puxam quando abrem — sem exigir dois devices online ao mesmo tempo. (Isso continua valendo no desenho revisado.)
- Ideia original considerava generalizar o `VaultRegistry` do TruthID (`identityId + vaultKind/appId → VaultRef`) — **rejeitada pelo TruthID, ver desenho revisado acima**: Practice Valuation traz o próprio contrato.
- Ideia original assumia session key escopada + Paymaster pra assinatura delegada — **substituída pelo canal de sign-request genérico já implementado**, ver desenho revisado acima.
- Login/pareamento de device: discutido reaproveitar o fluxo de QR do TruthID (servidor HTTP local na mesma rede vs. polling on-chain) — a questão em si continua válida (ver item 6 do desenho revisado acima), só o mecanismo concreto mudou: não é mais "login" propriamente, é o mesmo transporte LAN/dead-drop que a Fase 13.9 do TruthID já usa pra extensão de navegador.

</details>

**Nota de UI, Sessão 54 (2026-08-02) — sem mudança de funcionalidade**: a aba própria "TruthID Sync" no nav principal foi movida pra dentro de Configurações (pedido explícito do dono do projeto: "poderia migrar a parte de integração com o truthid pras configurações?"). `TruthIdPanel.tsx` foi renomeado/movido pra `desktop/src/truthid/TruthIdSettingsSection.tsx` (sem `Card` própria — a `Card` já vem de `SettingsPage.tsx`, mesmo padrão de `IaSettingsSection`), virando a segunda seção da sidebar de Configurações (a primeira é "IA"). `App.tsx` perdeu a entrada `truthid` do objeto `SECTIONS`/`TabsContent`. Nenhum comando Rust mudou — é reposicionamento de UI puro. As referências a `TruthIdPanel.tsx` no histórico de sessões acima (23-40) descrevem o nome do arquivo **como era na época**, não atualizadas retroativamente.

---

### Fase 9 — Tela de Pesquisa de Ação (Stock Lookup, ideia levantada na Sessão 20)

**Objetivo**: tela nova e separada (aba "Stock Lookup", ao lado de "Valuation" na navegação principal) pra pesquisar um ticker de ação e ver várias informações reunidas de uma vez — cotação, fundamentos (LPA/VPA/ROE/payout), dividendo médio 5 anos, DCF fundamentals, SMA 50/100/200 dias, CAGR de preço 5/10 anos, P/L, P/VP, a valuation já salva pra esse ticker (se existir) e um espaço de anotações livres. Só ações por enquanto — cripto (que já tem seu próprio dashboard, Fase 4.3) fica pra depois.

**Etapas**:
- [x] 9.1 — Concluída na Sessão 20:
  - `fetch_technicals` novo em `data-collector/sources/acoes_yahoo.py` (reusa o mesmo endpoint do Yahoo já usado por `fetch_quotes`/`fetch_dividends_avg`, `range=10y&interval=1d`) calcula SMA 50/100/200 dias e CAGR de preço 5/10 anos (`None` quando o ticker não tem histórico suficiente pra aquele cálculo — mesmo padrão de `payout`/`tax_rate`). Testado contra a API real (PETR4, MGLU3, WEGE3, RAIZ4, ticker inválido) antes de integrar no resto
  - Tabelas novas `stock_technicals` (SMA/CAGR, campos nullable) e `stock_notes` (anotação livre por ticker, upsert sem constraint de unicidade — app single-user local) via migration SeaORM. **Achado real**: o mesmo bug de conversão de nome de coluna da Sessão 5/6 (`avg_dividend5y` sem underscore, corrigido por `m20260710_220000_rename_avg_dividend5y_column`) se repetiu aqui — `DeriveIden` converteu `Sma50`/`Cagr5y` pra `sma50`/`cagr5y` (sem underscore antes do dígito). Corrigido **antes** de qualquer dado real ser gravado: `Alias::new("sma_50")` etc. explícito em vez das variantes do enum, `migrate down -n 2` + edição + `migrate up` de novo
  - `collect_stock_technicals` novo no `main.py`, chamado dentro do mesmo pipeline unificado que os formulários de valuation já usam (`run_stock_collector` continua servindo os dois — decisão consultada com o usuário: unificar a busca de histórico no Python, mas **não** disparar sempre — ver próximo item)
  - Aba nova `StockLookupPanel.tsx` (`desktop/src/stock-lookup/`), com busca "cache-aware": a primeira pesquisa de um ticker sem nada salvo no banco dispara o collector sozinha (`useEffect` com guarda por `useRef`, evita loop infinito se o ticker realmente não existir/não tiver dado); pesquisas seguintes só leem o banco; botão "Refresh data" força atualização a qualquer momento (mesmo padrão visual do botão "Run crypto collector" do Crypto Score)
  - P/L e P/VP calculados no frontend, não persistidos — derivados de `price`/`lpa`/`vpa` já existentes
  - Planejada com `/plan` (dois agentes `Explore` em paralelo levantaram os padrões existentes de navegação/painéis no frontend e de migrations/entities/commands/collector no backend antes de qualquer código) e testada de ponta a ponta: `cargo check` (app principal + crate `migration`) e `npx tsc --noEmit` limpos, `python main.py --ticker PETR4` rodado contra a API real depois da migration corrigida (linha real conferida em `stock_technicals`), app rodando via `dev.sh` — **testado pelo próprio usuário no app real, aprovado** ("mt boa, mt legal")

**Pendências levantadas pelo usuário na Sessão 20, pra uma sessão futura** (só registradas, nada planejado em detalhe ainda):
- [x] 9.2 — Mais indicadores na tela: dívida líquida/EBITDA, EV/EBIT, margem líquida. **Concluída na Sessão 20**: dos 3, 2 são conta pura no frontend (`StockLookupPanel.tsx`) em cima de campos que já existiam — `net_debt_ebitda = (total_debt − cash) / (ebit + depreciation_amortization)` e `ev_ebit = (preço × ações_em_circulação + total_debt − cash) / ebit`, tudo já em R$ milhões vindo da CVM, sem precisar converter unidade. O 3º (margem líquida) precisou de dado novo: `revenue` (receita líquida, conta "3.01" da DRE) adicionado em `cvm_dfp.py::fetch_dcf_fundamentals` — **verificado contra o zip real da CVM antes de codar**: conta "3.01" tem exatamente a mesma cobertura da "3.05" (EBIT), 436 de 436 empresas. Coluna nova `revenue` (nullable) em `stock_dcf_fundamentals` via migration (`m20260715_230504_add_revenue_to_stock_dcf_fundamentals`). Lucro líquido não ganhou extração própria — deriva de ROE × VPA × ações em circulação (dado que já existe), evitando puxar mais uma conta da CVM só pra isso. Testado ponta a ponta: extração isolada contra PETR4/WEGE3/MGLU3/ITUB4 reais (ITUB4/banco corretamente descartado, taxonomia diferente), `cargo check` nas duas crates, `npx tsc --noEmit`, `python main.py --ticker PETR4` real com a migration aplicada, conferência manual das 3 contas com os números reais da PETR4 (Net Debt/EBITDA ≈1,51x, EV/EBIT ≈6,0x, Net Margin ≈23,7%) — **confirmado pelo usuário no app real** ("deu boa, os 3 indicadores aparecendo certinho")
- [x] 9.3 — Gráfico de histórico de dividendos por mês e por ano. **Concluída na Sessão 20**: nova função `fetch_dividend_payments` em `acoes_yahoo.py` (`range=10y&interval=1d&events=div`, mesmo endpoint de `fetch_technicals`) casa cada pagamento com o fechamento do dia via `_closest_close` (generalizada com um parâmetro de tolerância — 5 dias aqui, 30 pro CAGR) pra calcular o yield daquele pagamento específico; verificado contra BBAS3 real (71 pagamentos, todos casados, `diff_days=0.0`). Tabela nova `stock_dividend_payments` (histórico real, diferente do resto do coletor: usa `INSERT OR IGNORE` num índice único `(ticker, payment_date)`, já que é fato histórico que não muda — rodar de novo/"Refresh data" não duplica). **Primeira biblioteca de gráfico do projeto**: escolhida `Recharts` com o usuário (opções apresentadas: Recharts, lightweight-charts, SVG sem dependência). Seguida a skill de dataviz: **nunca dois eixos Y** — em vez de um gráfico combinado, viraram 2 mini gráficos de barra empilhados (valor pago e yield%) com toggle mensal/anual, reusando a rampa de verde já definida em `index.css` (`--chart-1`.._5`). Testado ponta a ponta: extração real, `cargo check`/`tsc` limpos, dedupe confirmado rodando o coletor 2x pro mesmo ticker (71→71, "0 new" na segunda vez), conferido visualmente pelo usuário no app real — **"mt bonitinho, exatamente como imaginei"**
- [x] 9.4 — Ícone/logo da empresa na tela. **Concluída na Sessão 21**: verificado contra a API real antes de codar (`curl` direto, sem token) — `brapi.dev/api/quote/{ticker}` devolve um campo `logourl` apontando pra `https://icons.brapi.dev/icons/{TICKER}.svg`, e esse padrão de URL funciona sozinho, sem precisar chamar a API: CDN pública (GitHub Pages + Fastly, `access-control-allow-origin: *`, cache configurado), 200 com SVG real pra ticker existente (testado PETR4/VALE3/WEGE3/MGLU3/ITUB4/RAIZ4), 404 limpo pra ticker inválido. Como é só uma URL previsível, a implementação ficou **inteiramente no frontend** — nenhuma mudança em `data-collector`, nenhum comando Rust novo, nenhuma tabela/migration: `CompanyLogo` novo em `StockLookupPanel.tsx`, um `<img>` com `onError` escondendo o elemento (sem placeholder/ícone genérico) quando o logo não existe pra aquele ticker. Todos os SVGs do CDN já vêm com fundo colorido próprio (confirmado inspecionando o conteúdo), então não precisou de wrapper branco. `npx tsc --noEmit` limpo, testado no app real rodando via `./desktop/dev.sh` — **confirmado pelo usuário** ("deu boa dms")
- [x] 9.5 — Reorganizar a navegação: **concluída na Sessão 20** — "Saved Valuations" saiu de `SECTIONS` (não é mais aba própria); a aba "Valuation" ganhou um estado local (`valuationView: "form" | "saved"`) que troca entre o formulário de cálculo e o `SavedValuationsPanel`, com um botão "Saved Valuations" no canto (ao lado do seletor de modelo) e um botão "← Back" pra voltar. `SavedValuationsPanel` desmonta/remonta ao trocar de view (reseta pra lista sempre que volta), decisão simples aceita sem precisar de estado persistente. `npx tsc --noEmit` limpo, aplicado via HMR no app rodando e confirmado pelo usuário ("o botão do saved valuations funcionou")

---

### Fase 10 — Remodelagem para Workspaces Colaborativos & Multi-Ativos (ideia levantada na Sessão 29, decisão de stack confirmada — mesma stack do projeto, não iniciada)

**Objetivo**: evoluir o Practice Valuation de app desktop single-user pra uma plataforma colaborativa de Intelligence & Portfolio Management (estilo Status Invest / Investidor10 + Notion), com múltiplos usuários por Workspace, autenticação via TruthID, e suporte a mais classes de ativos além de ações BR.

**Decisão de arquitetura (Sessão 29, resolvida)**: o rascunho original mencionava "App Django" e PostgreSQL — o dono do projeto confirmou que isso veio de uma sugestão equivocada do Gemini ao ajudar a escrever o rascunho, não uma intenção real de trocar de stack. **Fica mantido o padrão já decidido no resto do projeto**: Tauri + Rust + React/TS, SQLite local, sem servidor próprio operado pelo Practice Valuation (ver "Decisões de Arquitetura em Aberto"). Os conceitos abaixo (workspaces, RBAC, multi-ativos, teses, watchlists) precisam ser adaptados pra rodar sobre esse modelo.

**⚠️ Ressalva honesta que continua em aberto**: isso não é só trocar "Postgres" por "SQLite" no papel. Um Workspace com Owner/Admin/Editor/Viewer de verdade — várias pessoas diferentes, em máquinas diferentes, escrevendo no mesmo Workspace — pressupõe **alguém arbitrando quem pode escrever o quê**. Num servidor central isso é trivial (checar `role` numa query antes de aceitar o `UPDATE`). De forma descentralizada, sem servidor do Practice Valuation, isso é um problema bem mais difícil: teria que apoiar em cima do sync da Fase 8 (TruthID + IPFS) e resolver permissão via algo tipo capability/assinatura em vez de uma checagem de banco — nada disso está desenhado ainda. Vale separar, quando a hora chegar, o que é "só mais schema no SQLite de sempre" (multi-ativos, watchlists, teses+anexos — não dependem de multiusuário nenhum) do que é de fato "colaboração entre pessoas diferentes" (Workspaces com convite/RBAC — esse sim exige resolver o problema acima antes de começar a codar).

**1. Arquitetura Multi-Tenant & Colaboração (Workspaces)**
- App passa a ser centrado em **Workspaces**: carteiras, teses, listas e anexos pertencem a um Workspace, não a um usuário direto.
- Fluxo: login via TruthID → tela inicial lista Workspaces próprios (Owner) e Workspaces em que o usuário foi convidado (Membro/Convidado) → botão "Criar Novo Workspace".
- RBAC granular por membro, definido pelo Owner/Admin: papéis padrão `Owner`/`Admin`/`Editor`/`Viewer`, mais flags específicas — `can_add_transactions`, `can_delete_transactions`, `can_create_theses`, `can_manage_members`.
- **Decidido (continuação da Sessão 29)**: pra destravar o início da Fase 10 sem depender do problema de permissão descentralizada (ressalva acima), o Workspace **nasce single-user** — convite/membro externo/RBAC ativo entre pessoas diferentes fica pra depois, ver "Ordem de Implementação" no fim desta fase.

**2. Consolidação de Carteiras & Multi-Ativos**
- Um Workspace pode conter múltiplas carteiras de investimento pra consolidação global.
- **Confirmado explicitamente pelo dono do projeto (continuação da Sessão 29)**: a carteira (`Portfolio`) pertence ao **Workspace**, não a um membro individual — mesmo modelo já refletido no rascunho de banco abaixo (`Portfolios(id, workspace_id, ...)`, sem `user_id`). Um Workspace com 3 membros compartilha as mesmas carteiras; não existe "minha carteira dentro do workspace" separada da carteira de outro membro — é a carteira do Workspace, e quem pode mexer nela é resolvido pelo RBAC (item 1), não por dono individual da carteira.
- Classes de ativos propostas: Ações (Brasil, já suportado hoje), Stocks internacionais (moeda base USD, novo), Criptomoedas (paridades principais, fração de ativos), Tesouro Direto detalhado (nome do título, taxa contratada, indexador, data de aplicação/vencimento), Renda Fixa geral (emissor, taxa % CDI/Pré/Pós, vencimento, liquidez). **Lista ampliada e detalhada na Sessão 30** — ver item 8 abaixo (FII, REIT, ETF, Metal, Imóvel, Empresa não listada, mais o modelo de listagem/exposição/classe).

**3. Pesquisa Centralizada & Tela de Análise**
- Barra de busca global (command bar / spotlight) centralizando a pesquisa de qualquer ativo (ações/stocks/cripto/renda fixa) ou tese.
- Tela unificada do ativo: dados de mercado (cotação, variação, gráficos, valuation), posição consolidada no Workspace (preço médio, total investido, P&L), e teses/notas do Workspace vinculadas àquele ativo.

**4. Teses de Investimento, Documentos e Anexos**
- `Thesis`: vinculável a um ativo específico (ex.: tese em WEGE3) ou global/macro (ex.: "Cenário de Juros 2027"), editor Markdown/Rich Text.
- `ThesisAttachment`: anexar PDFs (relatórios de RI, fatos relevantes, lâminas), planilhas e imagens — armazenamento em bucket (S3/MinIO/Cloudflare R2) com metadados no banco.

**5. Listas de Ativos (Watchlists) & Favoritos**
- Favoritos: marcação rápida de ativos pra destaque no dashboard do Workspace.
- Watchlists nomeadas (ex.: "Ações de Dividendos", "Turnarounds pra Acompanhar") com anotações e preço-alvo por ativo.

**6. Rascunho de modelagem de banco trazido junto** (formato ilustrativo do brainstorm original; ao planejar de verdade, vira migration SeaORM/SQLite como todo o resto do projeto — ver Fase 1):
```sql
-- Workspaces & Membros
Workspaces (id, name, owner_truthid, created_at)
WorkspaceMembers (id, workspace_id, user_truthid, role, permissions_json)

-- Carteiras e Transações
Portfolios (id, workspace_id, name, description)
Assets (id, ticker, name, asset_class, currency)
Transactions (id, portfolio_id, asset_id, type, quantity, price, date, fees, fixed_income_metadata_json)

-- Teses, Documentos e Watchlists
Theses (id, workspace_id, asset_id NULL, title, content_markdown, created_by)
ThesisAttachments (id, thesis_id, file_url, file_name, file_size)
Watchlists (id, workspace_id, name)
WatchlistItems (id, watchlist_id, asset_id, target_price, notes)
```

**7. Rentabilidade Histórica (Backfill) da Carteira** (ideia trazida num rascunho à parte na continuação da Sessão 29, `spec_rentabilidade_historica_carteira.md`, conteúdo transcrito aqui e o arquivo removido)

**Problema**: migrar uma carteira que já existe há 2-3 anos pro app sem perder o histórico de rentabilidade — lançar retroativamente as movimentações (compra/venda/provento/aporte/retirada) desde a origem da carteira e calcular a rentabilidade mês a mês, consolidando mês → ano → período total, com granularidade **mensal** (não diária).

**Por que não é `(valor final − valor inicial) / valor inicial`**: essa conta ingênua quebra na presença de aporte/retirada no meio do mês (um aporte de R$5.000 no meio do mês infla o "retorno" sem ser rendimento de verdade). Existem duas famílias de métrica:
- **Money-Weighted Return (TIR/XIRR)**: mede o retorno em R$ do dinheiro do investidor, mas não encadeia mês a mês de forma simples.
- **Time-Weighted Return (TWR)**: mede o retorno da carteira em si, neutralizando o efeito de quando o dinheiro entrou/saiu — é o padrão usado por fundos/CVM/B3, e **é geometricamente componível** (jan +1%, fev −2%, ... → total = produto de `(1+r)` − 1). É essa a métrica certa pro que foi pedido (rendimento mês a mês se somando geometricamente até o total).

**Como calcular TWR mensal sem dado diário — Método de Dietz Modificado** (aproxima o TWR "de livro", que exigiria reavaliar a carteira a cada fluxo de caixa, usando só o valor no início/fim do mês + os fluxos do mês):
```
R_mes = (EMV - BMV - CF_total) / (BMV + Σ(CF_i × W_i))
```
onde `BMV`/`EMV` são o valor da carteira no início/fim do mês, e `W_i = (dias_no_mes - dias_desde_inicio_ate_o_fluxo) / dias_no_mes` é o peso de cada fluxo (um aporte no dia 1 "trabalhou" o mês inteiro, peso ≈ 1; um aporte no dia 28 quase não influenciou o mês, peso ≈ 0). Proventos entram como fluxo de caixa positivo dentro do mês (mesma lógica de aporte) se sacados, ou só aumentam o `EMV` se reinvestidos/mantidos em caixa — decisão de modelagem em aberto, ver abaixo.

**Consolidação mês → ano → total** (TWR é geometricamente componível, sem mistério):
```
R_ano   = Π(1 + R_mes_i) - 1   para os N meses do ano
R_total = Π(1 + R_mes_i) - 1   para todos os meses do período filtrado
```
Filtro de período (ex.: "desde que comecei", "últimos 2 anos", "2024") é só decidir o intervalo de meses no produtório — nenhuma mudança de metodologia.

**O que precisa ser modelado**: pra calcular `BMV`/`EMV` de cada mês retroativamente, o sistema precisa, pra cada fim de mês no passado, saber (1) quais ativos estavam na carteira e em que quantidade — derivado do histórico de lançamentos até aquela data, (2) o preço de fechamento de cada ativo naquele fim de mês — precisa de série histórica de preços, não só o preço atual (pra ativos B3 o projeto já tem infra de dado de mercado; pra ativos vendidos antes de hoje, precisa do preço histórico até a data da venda), e (3) todos os fluxos de caixa do mês (aportes/retiradas/proventos em dinheiro) com data e valor. Rascunho de estrutura (ilustrativo, viraria migration SeaORM/SQLite como o resto do projeto):
```
Lancamento { data, tipo: compra|venda|aporte|retirada|provento, ativo (null p/ aporte/retirada puro), quantidade, preco_unitario, valor_total }
SnapshotMensal { ano_mes, valor_carteira_fim_mes (calculado), rentabilidade_mes (calculado via Dietz Modificado) }
```
No desenho da Fase 10, `Lancamento` mapeia naturalmente pra `Transactions` (já no rascunho de banco acima, escopada por `portfolio_id` — e por consequência por Workspace, ver item 2); `SnapshotMensal` seria tabela nova, por `portfolio_id` + `ano_mes`.

**Decisões em aberto (o dono do projeto ainda precisa bater o martelo)**:
- Proventos contam como retorno mesmo sem reinvestir, ou só quando reinvestidos? (Convenção de mercado: contam no mês do pagamento, entram no `EMV` via caixa, mesmo sem reinvestir.)
- Meses sem lançamento nenhum ainda têm `R_mes` (variação de preço dos ativos parados) — não dá pra simplesmente pular o mês no produtório.
- Carteira zerada em algum mês (vendeu tudo) quebra o encadeamento de TWR ali — tratar como "novo período" a partir da próxima entrada, ou explicitar na consolidação total.

**Exemplo numérico** (do rascunho original, confirma a fórmula): janeiro começa com R$10.000, aporte de R$2.000 no dia 10 (peso ≈ 21/31 ≈ 0,68), fecha com R$12.500 → `R_jan = (12.500 − 10.000 − 2.000) / (10.000 + 2.000×0,68) = 500/11.360 ≈ 4,4%`. Fevereiro começa com R$12.500, sem fluxo, fecha com R$12.250 → `R_fev = (12.250 − 12.500)/12.500 = −2,0%`. Consolidado jan+fev: `(1,044 × 0,98) − 1 ≈ 2,3%`.

**8. Modelo Detalhado de Multi-Ativos: Exposição, Novas Classes e Anexos** (rascunho à parte, `practice-valuation-carteira-multiativo (1).md`, conteúdo transcrito aqui e arquivo removido, Sessão 30). Detalha e substitui a lista simples de classes do item 2 acima — é o desenho de como a fatia 10.2 (multi-ativos) deve funcionar de fato.

**Classes de ativo suportadas**: Ação (B3 e bolsas americanas), FII, REIT (equivalente americano do FII — FII usa P/VP e dividend yield em BRL, REIT usa FFO/AFFO e cap rate em USD, mas convivem na mesma "família" de fundo imobiliário na UI), ETF (B3 e bolsa americana, expansível a outros países no futuro), Cripto, Metal (ouro, prata, etc.), Imóvel (ativo físico, cadastro manual) e Empresa não listada (equity privado, cadastro manual).

**Modelo central — listagem ≠ exposição ≠ classe**: hoje cada ativo tende a ter implicitamente um único país, herdado da fonte de dados (bolsa de negociação). O modelo separa três conceitos:

| Conceito | O que representa | Origem |
|---|---|---|
| Listagem/exchange | Onde o ativo é negociado (ex.: NASDAQ, B3) | Puxado automático da fonte de dados, editável |
| Exposição | A que país(es) ou categoria especial o ativo realmente expõe o investidor | Padrão = país da listagem, editável pelo usuário |
| Classe do ativo | Ação, FII, REIT, ETF, cripto, metal, imóvel, empresa não listada | Cadastro do ativo |

Exposição é **um ou mais países com peso % somando 100%, OU uma categoria especial** (cripto, metal_ouro, metal_prata, etc.) — mutuamente exclusivo por ativo. Campo sugerido `tipo_exposicao = pais | categoria_especial`, com tabela `ativo_exposicao_pais (ativo_id, pais, peso_%)` pro caso de país. Exemplos: Banco Inter negociado nos EUA → listagem EUA (auto), exposição 100% Brasil (editado manualmente), classe Ação; IVVB11 → listagem B3 (auto), exposição 100% EUA (editado), classe ETF; ETF brasileiro de cripto → listagem B3, exposição categoria_especial (cripto) — fora do cálculo de alocação por país; ETF de ouro → mesma lógica, categoria_especial (metal_ouro).

**Automatização da exposição**: puxar a exposição real de um ETF via fact sheet tem custo alto de implementação com pouco ganho no estágio atual — abordagem pro MVP é override manual por ativo (cadastra uma vez, fica salvo); evolução futura possível seria uma base curada própria de ETFs conhecidos, community-maintained.

**Metais como classe**: preço puxado automaticamente (cotação ao vivo, como cripto, sem valor manual); quantidade cadastrada em **gramas**, não "ações/cotas" — pede um campo `unidade_medida` no ativo (`acao`, `cota`, `grama`, `unidade`), generalizando o cálculo de posição pra `quantidade × preço_unitário` independente da classe. Fonte de cotação provavelmente é uma API diferente das já usadas — mais um provider seguindo o padrão de "cada classe pode ter sua fonte própria".

**Imóvel** (ativo físico, cadastro manual, sem fonte de dados externa): fotos/documentos anexos (escritura, ITBI, impostos pagos), endereço, valor de compra (fixo, histórico), histórico de avaliações `(data, valor, origem: manual | reajuste_automatico)`. Reajuste automático por % anual — mecanismo de cálculo (ao vivo vs. snapshot periódico) **ainda não decidido**, adiado pra implementação.

**Empresa não listada** (equity privado, mesmo padrão do imóvel): percentual de participação, quantidade de ações, valor de compra, valor atual (editável manualmente, mesmo mecanismo de histórico do imóvel), valuation total da empresa, fotos/foto de perfil da empresa. **Ponto em aberto**: percentual, quantidade de ações e valuation total são matematicamente relacionados (`percentual = suas_ações / total_de_ações_da_empresa`; `sua_participação_R$ = percentual × valuation_total`), mas isso só fecha se o total de ações emitidas pela empresa for conhecido — nem sempre disponível em empresa não listada. Decidir se o percentual é validado contra esses campos ou se é um valor digitado independente.

**Generalização `AtivoManual`**: Imóvel e Empresa não listada compartilham o mesmo esqueleto — cadastro manual, sem fonte de dados externa, histórico de valor, anexos. Faz sentido um tipo base `AtivoManual` (nome, valor de compra, histórico de valor, anexos), com cada subtipo adicionando só os campos específicos (endereço vs. participação societária).

**Anexos (sistema polimórfico único)**: em vez de um sistema de anexo por feature (imóvel, empresa, lançamento), um único subsistema reutilizável: `anexo (id, entidade_tipo, entidade_id, arquivo, tipo_documento, data)`, onde `entidade_tipo` pode ser `imovel`, `empresa` ou `lancamento` — todos passam pelo mesmo código de upload/storage, só muda o rótulo do tipo de documento. (Mesma necessidade de storage de arquivo já levantada em `ThesisAttachment`, item 4 acima — decidir na implementação se vira o mesmo mecanismo ou fica separado.)

**Busca unificada de ativos**: uma única busca cobrindo todas as classes negociadas em bolsa (não inclui Imóvel/Empresa não listada, que são cadastro manual direto, sem ticker). Arquitetura híbrida: catálogo local pré-indexado (`ativo_catalogo`: ticker, nome, classe, exchange, fonte_dados) populado por job de sync periódico, busca instantânea pro caso comum; atualização sob demanda — se a busca não encontra o ativo no catálogo local, cai pra fan-out ao vivo nas APIs externas, e se encontrar, indexa no catálogo pra próxima busca. UX: cada resultado exibe classe + exchange + exposição (ex.: "BIDI3 — Ação — B3 — 🇧🇷" vs. "INTR — Stock — NASDAQ — 🇧🇷"), pra resolver o caso de ativos do mesmo emissor negociados em mercados diferentes (ex.: Banco Inter).

**Decisões em aberto trazidas por este rascunho**: mecanismo de reajuste automático de imóvel (ao vivo vs. snapshot); validação cruzada percentual/quantidade/valuation de empresa não listada; automação de exposição de ETF via fact sheet (adiado — override manual no MVP).

**9. Gestão de Custódias/Contas** (rascunho à parte, `practice-valuation-mobile-custodia-nome.md`, seção 4, conteúdo transcrito aqui e arquivo removido, Sessão 30). Nova dimensão no modelo, ortogonal à classe do ativo: **onde** cada investimento está guardado (corretora, exchange, hardware wallet, banco), independente do que o ativo é.

Entidade `Custodia`: cadastro livre pelo usuário, sem nada pré-pronto (ex.: "Rico", "Nomad", "Bipa", "Ledger"). Campo `instituicao` (nome da corretora/exchange/wallet) e campo `titular` (CPF/CNPJ ou identificação do titular da conta) — dois campos separados na mesma entidade, sem precisar de hierarquia adicional. Permite agrupar por qualquer uma das duas dimensões: "tudo que está na Rico" (soma titulares) ou "tudo que é meu CNPJ" (soma instituições).

Posição quebrada por custódia: `posicao (ativo_id, custodia_id, quantidade)` — um mesmo ativo pode estar dividido entre múltiplas custódias (ex.: parte do BTC na Binance, parte na Ledger). Total do ativo na carteira = soma de todas as linhas de posição. Resumo consolidado da carteira soma tudo, ignorando titular/custódia por padrão — o detalhamento é uma visão adicional, não a visão default.

Tela de gerenciamento de contas: seção própria na navegação, separada do cadastro de ativo. Lista de custódias cadastradas, cada uma mostrando total consolidado ali dentro + breakdown por classe de ativo. Duas visões possíveis a partir da mesma tabela `posicao` — por custódia (o que está em cada lugar) e por ativo (onde cada ativo está distribuído) — não duplica dado, só agrupa diferente.

Lançamentos com custódia e taxa: vínculo à custódia acontece no momento do lançamento (compra/transferência já pede em qual custódia está entrando), não como passo manual separado. Novo tipo de lançamento **transferência** entre custódias, com `custodia_origem`, `custodia_destino`, `quantidade`. Campo `taxa` genérico, aplicável a qualquer tipo de lançamento (compra, venda, transferência) — reaproveita a estrutura de lançamento já existente (`Lancamento`/`Transactions`, ver item 7 acima), sem tabela separada.

**Decisão em aberto**: taxa de transferência sempre no próprio ativo transferido, ou pode ser em ativo/moeda diferente (ex.: taxa de rede em cripto vs. corretagem em BRL) — não decidido.

**10. Tela inicial do Workspace / Dashboard personalizável** (ideia trazida pelo dono do projeto na Sessão 68, puro registro até a Sessão 84 — ver item 10.8 abaixo pra fatia 1, já planejada e implementada). Hoje, depois do `WorkspaceGate` (Fase 10.6), o usuário cai direto numa aba do Portfolio (Custódias/Ativos/Lançamentos) — não existe uma tela-resumo. Pedido: uma tela inicial logo após entrar no Workspace, funcionando como painel/resumo de tudo — carteiras, valor total, rentabilidade — "bonitinha", tipo dashboard de verdade (não uma lista de números crus).

**Painel de mercado (parte mais específica do pedido)**: cadastrar índices e ativos pra acompanhar em tempo real dentro dessa tela, independente de fazer parte da carteira do usuário — exemplo dado: S&P 500. Índice não é um ativo comprável (não tem posição/quantidade), então é um conceito novo, distinto do catálogo de `Asset` atual — decisão em aberto: nasce como uma classe própria (`indice`, sem campos de posição/custódia) reaproveitando o mesmo pipeline de cotação Yahoo (que já serve tickers de índice, ex. `^GSPC`, `^BVSP`), ou como uma extensão do mecanismo de Watchlist (Fase 10.4, que já resolve "acompanhar algo sem possuir") em vez de crescer o catálogo de `Asset`. Nenhuma das duas decidida ainda.

**Personalização total, pedido explícito**: o usuário quer poder montar essa tela na mão — não um layout fixo pré-definido, um dashboard de widgets configuráveis (o que aparece, provavelmente também onde/tamanho). E pedido explicitamente **nos dois clientes**, desktop e mobile (Fase 11.2) — a persistência da configuração de layout por Workspace (e se compartilhada ou por-dispositivo, já que desktop e mobile não sincronizam ainda, ver Fase 11.2) é uma decisão em aberto.

**Sobreposição não resolvida**: painel de mercado (índices/ativos "pra acompanhar") se sobrepõe conceitualmente com Watchlists/Favoritos (Fase 10.4) — pode ser a mesma coisa com apresentação diferente (cards de dashboard em vez de tabela) ou um mecanismo separado. Fica pra decidir quando essa fatia for planejada de verdade.

**Ordem de implementação decidida com o dono do projeto (continuação da Sessão 29, via `AskUserQuestion`)**:

**Workspace nasce single-user** — um único Workspace implícito (o próprio dono), sem convite/membro externo, sem RBAC ativo ainda. Essa escolha destrava começar a Fase 10 sem precisar resolver a permissão descentralizada primeiro (ver ressalva acima) — convite/multiusuário real vira uma fatia à parte, bloqueada até esse problema estar resolvido (provavelmente amarrado à Fase 8). **Primeira fatia concreta escolhida pra quando começarmos: multi-ativos.**

- [x] 10.1 — Fundação: schema `Workspace`/`Portfolio` (SQLite/SeaORM) — Workspace single-user, sem `WorkspaceMember`/convite ainda. **Implementada na Sessão 33 (2026-07-29)**: migrations `workspace`/`portfolio` (Workspace semeado com exatamente 1 linha, mesmo espírito do único registro implícito que `ai_api_key` já assumia; uma `Portfolio` padrão "Carteira Principal" também semeada pra não deixar o usuário numa tela vazia). Comandos `get_workspace`/`list_portfolios`/`create_portfolio`/`rename_portfolio`.
- [x] 10.2 — **Primeira fatia, decidida com o dono do projeto — escopo Sessão 29, NÃO o expandido da Sessão 30**: Multi-ativos — Stocks internacionais (moeda USD), Tesouro Direto detalhado, Renda Fixa geral no schema de `Transactions`; Gestão de Custódias básica (ver item 9) incluída desde o início, por decisão explícita (Sessão 33). **Implementada na Sessão 33 (2026-07-29)**: tabelas `assets` (catálogo global, não escopado por Workspace/Portfolio; `asset_class` só `acao_br`/`acao_internacional`/`tesouro_direto`/`renda_fixa` por ora; modelo de exposição **simplificado** — um valor só por ativo, `pais` ou `categoria_especial`, não o split percentual multi-país do rascunho original, decisão explícita da Sessão 33), `custodia` (`instituicao`+`titular`, escopada por Workspace), `transactions` (compra/venda/aporte/retirada/provento/transferência entre custódias; campos `fi_*` — emissor/indexador/taxa contratada/vencimento/liquidez — modelados **por transação, não por ativo**, decisão da Sessão 33: a taxa de um título de renda fixa é fixada na compra, não é propriedade fixa do "mesmo" título nominal). Comandos: `list/create_asset`, `list/create/delete_custodia`, `create/delete/list_transaction` + `get_portfolio_positions` (quantidade líquida + preço médio de compra simples — sem FIFO/custo médio de verdade, isso fica pra 10.3 — + breakdown por custódia). Nova aba "Portfolio" no app (`desktop/src/portfolio/`), 3 sub-seções (Custódias/Ativos/Lançamentos & Posições). `cargo check`/`cargo test --lib` (86/86, sem teste novo — CRUD sem função pura pra testar, mesmo padrão de `alert_rule.rs`) e `npx tsc --noEmit` limpos. **As classes expandidas da Sessão 30 (FII/REIT/ETF/Cripto/Metal/Imóvel/Empresa não listada, modelo de exposição multi-país) ficam para uma fatia futura** — nenhuma das decisões em aberto delas foi resolvida ainda. **Testado no app real na Sessão 34 (2026-07-29)**: fluxo completo (cadastrar ativo → cadastrar custódia → lançar compra/aporte → conferir posições consolidadas) confirmado funcionando pelo dono do projeto, fechando a pendência deixada pela Sessão 33.

**Redesenho do cadastro de Ativos pra Ação BR, também na Sessão 34**: testando ao vivo, o dono do projeto achou o formulário de cadastro (ticker/nome/moeda/bolsa tudo digitado à mão) ruim pra classes que já têm fonte de dado automática — pediu que funcionasse como a tela "Stock Lookup" (Fase 9): busca por ticker, dado vem sozinho, só edita se quiser; cadastro manual de verdade ficaria reservado pra ativos sem fonte automática (imóvel, empresa não listada — ainda não implementados). Cobrir todas as classes citadas (ações, stocks, FII/REIT, Tesouro Direto, cripto) exigiria um coletor novo por classe — decidido com o usuário (via `AskUserQuestion`) escopar **só Ação BR** nesta fatia, já que é a única com infra de dado pronta (Yahoo Finance); as demais classes continuam manuais, migração classe a classe em sessões futuras. **Achado confirmado ao vivo contra a API real do Yahoo**: o bloco `meta` que `acoes_yahoo.py::fetch_quotes` já lia só pra `regularMarketPrice` também traz `longName` (nome), `fullExchangeName` (bolsa) e `currency` — sem precisar de endpoint novo. Migration `add_details_to_stock_quotes` (3 colunas nullable, 3 `alter_table` separados por causa do bug de ambiente já documentado — SQLite/sea-query não aceita múltiplos `add_column` numa `ALTER TABLE` só). `AssetSection.tsx`: quando a classe é "Ação (B3)", o form vira busca de ticker (mesmo padrão auto-fetch-once da Stock Lookup) que pré-preenche Nome/Moeda/Bolsa (editáveis) e Exposição (`pais`/`BR`, também editável); outras classes seguem com o formulário manual de sempre. `cargo check`/`cargo test --lib` (86/86) e `npx tsc --noEmit` limpos; coletor Python testado contra a API real (PETR4: nome/bolsa/moeda gravados certo); fluxo de busca confirmado pelo usuário no app real.
- [x] 10.3 — Rentabilidade histórica (TWR/Dietz Modificado, ver seção 7 acima). **Implementada e testada no app real na Sessão 35 (2026-07-30), fatia escopada só a Ação BR** — decisão explícita do dono do projeto: as outras 3 classes (Stocks internacionais/Tesouro Direto/Renda Fixa) não têm preço histórico automatizado ainda, e o projeto não vai pra produção nesse estado, então a lacuna fica documentada em vez de bloquear a fatia ("deixei registrado que precisamos resolver isso pra todos os ativos"). Nova tabela `stock_price_history` (série diária persistida do Yahoo, `range=10y&interval=1d` — antes só usada transitoriamente pra SMA/CAGR do Stock Lookup, agora também salva) alimentada por um backfill manual novo ("Atualizar histórico de preços"). `domain::twr::modified_dietz`/`chain` calcula mês a mês; `commands::profitability::get_portfolio_profitability` reconstrói holdings a partir de `transactions`, tratando compra(+)/venda(−)/provento(+) da fatia Ação BR como os fluxos externos do Dietz Modificado (o app não tem sub-ledger de caixa, então uma compra equivale a um aporte na sub-carteira Ação BR) — `aporte`/`retirada` (sem `asset_id`) ficam fora deste cálculo. Preço faltando pra um mês → erro nomeado, não um número silenciosamente errado. Nova aba "Rentabilidade" no Portfolio (gráfico de barra mensal + linha acumulada + tabela). `cargo test --lib` 90/90 (4 testes novos em `domain::twr`, batendo o exemplo numérico do item 7 acima), `npx tsc --noEmit` limpo, coletor Python testado contra a API real (PETR4/BBAS3, backfill idempotente confirmado). **Testado no app real**: dono do projeto abriu o app (`docker compose up`), abriu a aba Rentabilidade e confirmou a linha de julho/2026 batendo perto de +64% — o número esperado pro primeiro mês de uma carteira nova com BMV zero, calculado à mão antes de perguntar. Estender preço histórico/TWR pras outras classes de ativo fica pra uma fatia futura. **Correção na Sessão 101 (Fase 24)**: o provento entrava como `+CF` no Dietz, o que **derrubava** o retorno (BMV 100, EMV 100, provento 5 → ≈ −5% em vez de +5%), contrário à intenção registrada aqui ("provento soma ao retorno"). Sinal centralizado em `domain::twr::flow_amount` (compra +, venda −, provento −, imposto pago +) com teste; quem tinha provento lançado vai ver a rentabilidade histórica subir.
- [x] 10.4 — Watchlists + favoritos. **Implementada e testada no app real na Sessão 36 (2026-07-30)**. Decisão explícita do dono do projeto (via `AskUserQuestion`, antes de planejar): os dois mecanismos do rascunho original entram juntos, não só watchlists nomeadas — favoritos (estrela) é um mecanismo **separado**, não "crie uma watchlist chamada Favoritos". Schema novo (`m20260730_100000_create_watchlists.rs`, 3 tabelas num arquivo só, mesmo padrão de `workspace`+`portfolio`): `watchlists` (FK Cascade → `workspace`, mesma hierarquia de posse de `Portfolio`/`Custodia`), `watchlist_items` (FK Cascade → `watchlists`, FK sem `on_delete` → `assets` — mesmo raciocínio de `transactions`, apagar um ativo do catálogo não deveria arrastar a anotação), `asset_favorites` (FK Cascade → `workspace`, FK sem `on_delete` → `assets`, índice único `(workspace_id, asset_id)`). `commands/watchlist.rs` (8 comandos): CRUD fino de watchlist/item, mais `toggle_favorite` — um único comando pro botão de estrela (busca a linha por `(workspace_id, asset_id)`, deleta se existir/insere se não, devolve o novo estado bool) em vez de expor add/remove separados, já que a UI só precisa alternar. Frontend: nova aba "Watchlists" em `PortfolioPanel.tsx` (`WatchlistSection.tsx` — bloco "★ Favoritos" no topo + seletor/CRUD de watchlist nomeada + tabela de itens com preço-alvo/notas, preço atual derivado client-side de `list_stock_quotes`+`latestForTicker`, mesmo padrão já usado em `AssetSection.tsx`, sem lógica de comparação/veredito nesta fatia) e nova coluna "★" na tabela de `AssetSection.tsx` (favoritar de qualquer lugar do catálogo, não só dentro da tela de Watchlists). `cargo check`/`cargo test --lib` (90/90, sem teste novo — CRUD sem função pura, mesmo padrão de `custodia`/`asset`/`transaction`) e `npx tsc --noEmit` limpos. **Testado no app real**: dono do projeto confirmou favoritar pela estrela em Ativos aparecendo em Watchlists → Favoritos, e o fluxo completo de criar lista/adicionar item/editar/deletar ("funcionou sim").
- [x] 10.5 — Teses + anexos. **Implementada e testada no app real na Sessão 37 (2026-07-31)**. Decidido via `AskUserQuestion` antes de planejar: anexo em **disco local** (não bucket S3/MinIO/R2, que era a sugestão do rascunho original — contraria a decisão já fechada de "sem servidor próprio"), e visualização **sempre dentro do app** (pedido explícito do usuário: "queria que tivesse como ver pelo app... sem ter que abrir nada novo") — imagem/PDF renderizados nativamente pelo WebView (`convertFileSrc`+`<img>`/`<iframe>`, sem lib nova), planilha (xlsx/csv) via SheetJS (`xlsx`, MIT), aprovado explicitamente por não ter suporte nativo. Schema novo (`m20260731_100000_create_theses.rs`): `theses` (FK Cascade → `workspace`, `asset_id` nullable FK sem `on_delete` → `assets` — tese pode ser vinculada a um ativo ou global/macro) e `thesis_attachments` (FK Cascade → `theses`, guarda só metadados — nome original, caminho relativo a `app_data_dir()`, tamanho, content-type — o arquivo em si vive fora do banco). `commands/thesis.rs` (8 comandos): CRUD de tese + upload/list/delete de anexo + `get_thesis_attachment_path` (caminho absoluto único que serve os 3 tipos de preview, imagem/PDF direto via `convertFileSrc`, planilha via `fetch` sobre a mesma URL + SheetJS — evita um segundo comando devolvendo bytes por IPC). Primeira feature do projeto a resolver `app_data_dir()` (Tauri v2, sem plugin) e a copiar um arquivo picado pro storage do próprio app (diferente do fluxo de extração de PDF da Fase 2.4, que só lê o arquivo no lugar). **Dois achados de ambiente reais nesta sessão**: (1) o `sea-orm-cli` da imagem Docker tinha driftado pra 2.0.0 (Dockerfile nunca fixava versão, `cargo install sea-orm-cli` sem `--version`) — `generate entity` com essa versão reescreveria `id: i32` → `i64` em **toda** entity do projeto, silenciosamente; revertido a tempo (`git checkout` antes de commitar) e `Dockerfile` corrigido pra `cargo install sea-orm-cli --version 1.1.20 --locked`, batendo com o `sea-orm = "1.1.20"` do `Cargo.toml` — imagem reconstruída, confirmado `sea-orm-cli 1.1.20` antes de regerar (diff limpo: só `theses.rs`/`thesis_attachments.rs` novos + `assets.rs`/`workspace.rs` ganhando `Related<...>`). (2) o volume novo `app-data:/home/node/.local/share` (necessário pros anexos sobreviverem a um `docker compose down` — sem bind mount nenhum cobria esse caminho antes) nasceu com dono `root`, quebrando o setup do WebKitGTK (`Failed to setup app: Permission denied`) na primeira subida — corrigido com `docker compose run --rm --user root desktop chown -R 1000:1000 /home/node/.local/share`, sem precisar editar o compose de novo. `tauri.conf.json` ganhou `security.assetProtocol` (`scope: ["$APPDATA/thesis_attachments/**"]`) e `Cargo.toml` a feature `protocol-asset` do crate `tauri` (build falha sem ela, mensagem clara). Frontend: nova aba "Teses" em `PortfolioPanel.tsx` (`ThesisSection.tsx` — tabela de teses + form título/ativo-vinculado/markdown com toggle Editar/Preview via `react-markdown`+`remark-gfm`, nenhum editor rico — só textarea, mesmo espírito plain-input do resto do projeto — mais bloco de anexos com upload multi-arquivo, delete com confirmação, e preview inline por tipo). `cargo check`/`cargo test --lib` (**94/94**, 4 novos — sanitização de nome de arquivo e adivinhação de content-type por extensão) e `npx tsc --noEmit` limpos. **Testado no app real** pelo dono do projeto: criar tese vinculada/global, editar/preview markdown, anexar e visualizar imagem/PDF/planilha sem sair do app, apagar anexo e tese — "funciona perfeitamente". **Feedback pra próxima sessão, explícito**: a UX atual (form solto numa aba, sem o dashboard/tela unificada de ativo que o rascunho original imaginava) não é o UX final desejado — "dps temos que dar uma melhorada geral, não é assim que eu imaginei o UX mas importante é funcionar por enquanto". Revisitar quando a Fase 10 tiver mais fatias prontas.
- [x] 10.6 — Seletor de múltiplos Workspaces na tela inicial. **Implementada e testada no app real na Sessão 38 (2026-07-31)**. Pedido original do usuário era um fluxo "login TruthID → aparecem os workspaces disponíveis → escolho um → entro", mas isso pressupõe workspace com dono/convite — exatamente o que a 10.7 descreve como bloqueada. Decidido via `AskUserQuestion`: só o **seletor local** por ora (múltiplos workspaces seus, sem TruthID/dono), como uma **tela de entrada antes do app** (não um dropdown escondido numa aba) — combina com a ideia de "entrar dentro dele" sem precisar do login real. Backend: `commands/workspace.rs` ganhou `list_workspaces`/`create_workspace` (mirror exato de `list_portfolios`/`create_portfolio`, um nível acima, sem filtro de pai) — sem migration nova (tabela `workspace` já existia da 10.1), sem `rename_workspace`/`delete_workspace` (não pedido, escopo morto). Frontend: novo `desktop/src/workspace/WorkspaceGate.tsx` (lista os workspaces existentes como botões "entrar" + form de criar-e-entrar), `App.tsx` ganhou state `workspaceId` que gateia toda a UI principal (se `null`, mostra só o `WorkspaceGate`) e um botão "Trocar Workspace" no cabeçalho (ícone `LogOutIcon`, mesmo estilo dos botões de chat/settings) que volta pra tela de entrada sem fechar o app. `PortfolioPanel.tsx` deixou de buscar `get_workspace()` internamente — passou a receber `workspaceId` como prop resolvida pelo `WorkspaceGate`, mudança contida (os 5 componentes filhos não mudaram, já recebiam `workspaceId` só como prop). `cargo check`/`cargo test --lib` (94/94, sem teste novo) e `npx tsc --noEmit` limpos. **Testado no app real** pelo dono do projeto: "testei deu boa".
- [ ] 10.7 — **Desenhada na Sessão 93 (2026-08-30), ainda não implementada**: convite + RBAC ativo entre pessoas diferentes + login TruthID multiusuário (diferente do uso atual do TruthID no projeto, que é só assinatura delegada pra sync — Fase 8). Até aqui a ressalva registrada na Sessão 29 (topo desta fase) ficava sem caminho concreto — "sem servidor, quem arbitra quem pode escrever o quê" — porque a comparação óbvia era com um servidor central checando `role` numa query antes do `UPDATE`. **Achado que destrava o desenho**: o TruthID já resolve um problema isomorfo — `DeviceRegistry.sol` (`~/Documents/workspace/truthid/contracts/src/DeviceRegistry.sol`) mapeia `identityId → address[]` de devices autorizados a agir por uma identidade, checado via `eth_call` antes de confiar numa ação assinada. O mesmo padrão generaliza pra "quais endereços podem escrever no Workspace Y, e com que papel" — a checagem de permissão vira um `eth_call` em vez de uma query Postgres, sem precisar de servidor nenhum.

  **1. `WorkspaceRegistry.sol`** (contrato novo do Anchor, mesmo padrão do `SyncRegistry.sol` já deployado em Base Mainnet — Fase 8.1 — reaproveitando `alloy`/`sol!` já usado lá): **um contrato só, não um deploy por Workspace** (`workspaceId` como namespace — `uint256` incremental, mesmo padrão de `_nextId` do `IdentityRegistry.sol` do TruthID; diferente da decisão da Sessão 24 de "cada app terceiro traz o próprio contrato", que fazia sentido pra um contrato deployado uma vez por desenvolvedor — aqui é o usuário final criando Workspaces repetidamente, deploy por Workspace sairia caro à toa).
    - `createWorkspace() → workspaceId` (caller vira Owner)
    - `addMember(workspaceId, address, permissions)` / `updateRole(...)` / `removeMember(...)` — gated: só quem já tem o bit `CAN_MANAGE_MEMBERS` pode chamar
    - `getPermissions(workspaceId, address) → uint256` (bitmask) — leitura pública, sem assinatura, mesmo espírito do `getRecord` do SyncRegistry
    - Eventos `MemberAdded`/`RoleChanged`/`MemberRemoved` — sinalizam pros outros devices quando invalidar o cache local de papéis

  **2. Modelo de papel — decidido com o dono do projeto (`AskUserQuestion`): bitmask de permissões**, não enum fixo. Reaproveita as flags já especificadas desde a Sessão 29 (`can_add_transactions`/`can_delete_transactions`/`can_create_theses`/`can_manage_members`) como bits de um `uint256`; Owner/Admin/Editor/Viewer viram presets convenientes (combinações fixas de bits) montados no frontend, não tipos novos no contrato — evita ter que re-deployar ou migrar o contrato toda vez que uma permissão nova aparecer.

  **3. Replicação de dado — depende de fechar a Fase 8.5** ("merge por replay causal", `valuation`/`alert_event` já são append-only, formato natural pra isso — ver Fase 8 acima). O mecanismo generaliza do jeito single-user (mesmo usuário, múltiplos devices) pro multi-user (usuários diferentes) sem mudar de forma: cada device publica um log append-only de eventos assinados (não snapshot de tabela inteira); antes de aceitar/repetir um evento assinado pelo endereço A, cada device confere — via cache local do `WorkspaceRegistry`, invalidado pelos eventos `RoleChanged`/`MemberRemoved` — se A tinha permissão suficiente pra aquela operação **no momento** do evento. Conflito: entidades já append-only (`transactions`/`theses`/`watchlist_items`) nunca colidem, só acumulam; entidades mutáveis (rename, edição de campo) resolvem por last-write-wins com timestamp assinado — sem desenhar CRDT do zero, mesma decisão já assumida pra 8.5.

  **4. Convite — decidido com o dono do projeto: os dois fluxos ficam registrados, mas só o mais simples entra nesta leva**. **Fatia a implementar primeiro**: colar o endereço (smart account) do convidado manualmente — Owner chama `addMember` com um endereço que já tem em mãos, zero protocolo novo, mesmo padrão de qualquer app cripto (compartilhar endereço fora de banda). **Fica registrado como pendência futura, não bloqueando nada**: QR/pareamento reaproveitando o padrão LAN/dead-drop que a Fase 8.6 já construiu pro TruthID — melhora a UX (sem copiar/colar endereço longo) mas não é necessidade técnica, só conveniência.

  **Escopo desta sessão — decidido com o dono do projeto: só o desenho acima, nenhum código novo.** Justificativa: a 10.7 só vira útil de ponta a ponta depois que a 8.5 (loop de sync completo, hoje `[ ]` não iniciada) existir — um contrato de ACL sozinho, sem o loop de replicação por trás, não libera nenhuma feature usável ainda. Diferente da 8.1 (que valeu a pena escrever+testar via `forge test` antes do broadcast, porque a leitura pública já tinha uso imediato), aqui não há uso imediato possível sem a 8.5 primeiro. **Ordem de implementação sugerida pra quando a hora chegar**: (a) Fase 8.5 primeiro (loop de sync completo, ainda single-user — pré-requisito técnico de qualquer jeito); (b) `WorkspaceRegistry.sol` escrito+testado via `forge test`, sem deploy real, mesmo padrão da 8.1; (c) deploy real em Base Mainnet + `commands/workspace_registry.rs` (leitura/escrita, espelhando `sync_registry.rs`); (d) generalizar o log de eventos da 8.5 pra checar permissão via `WorkspaceRegistry` antes de aceitar um evento de terceiro; (e) UI de convite (colar endereço) + gerenciamento de membros em Configurações. QR/pareamento (item 4 acima) fica pra depois disso, se o dono do projeto quiser a UX melhor.
- [x] 10.8 — **Home do Workspace, fatia 1 (layout fixo)**. **Implementada e testada no app real na Sessão 84 (2026-08-22)**. 4 decisões de escopo fechadas via `AskUserQuestion` antes de planejar (o item 10 acima era "puro registro" até aqui): (1) Home fica **separada** do "Resumo" de cada Carteira (Fase 13.1, `SummarySection.tsx`) — agrega o Workspace inteiro, Resumo continua intocado; (2) **layout fixo** nesta fatia, sem drag/resize/configuração de widgets (personalização fica pra depois); (3) painel de mercado = **extensão do mecanismo de Watchlist/Favoritos** (Fase 10.4), não uma classe de ativo nova — resolve a "sobreposição não resolvida" registrada acima: um ativo cadastrado no catálogo sem nenhuma transação (ex. um índice) já aparece com cotação ao vivo se favoritado, zero mudança de backend necessária; (4) **só desktop** nesta fatia (mobile não sincroniza ainda, Fase 11.2). Recorte adicional decidido durante o planejamento: sem "vs CDI" no card de retorno (dependeria de estender `get_profitability_comparison`, escopado a 1 portfolio, pra múltiplos — fora do pedido original, fica pendência); widget "Carteiras" é só leitura (nome + valor, sem navegação pra dentro da carteira).
  - **O que foi implementado**: novo `commands::workspace_summary` (`get_workspace_summary`/`get_workspace_profitability`) — mesmo precedente de agregação cross-portfolio que `commands::net_worth::get_net_worth_summary` (Fase 12) já tinha estabelecido (`portfolio::Entity::find().filter(WorkspaceId)` + loop `compute_positions`); `get_workspace_profitability` só resolve os `portfolio_ids` do workspace e chama `compute_profitability(db, &portfolio_ids, None)` direto — a mesma função já aceitava múltiplos portfolios desde a Fase 12, zero lógica nova. `ClassAllocation` (`portfolio_summary.rs`) reaproveitado como está. Nenhuma migration. Frontend: novo módulo `desktop/src/home/` (`HomeSection.tsx` — KPIs/donut/tabela de carteiras/★ Market panel, mesmos helpers de `SummarySection.tsx` duplicados localmente, mesma convenção do resto do projeto; painel de mercado copia a tabela "★ Favorites" de `WatchlistSection.tsx` sem a coluna de ações). `App.tsx`: `SECTIONS` ganhou `home` como primeira entrada, estado inicial de `section` virou `"home"` — Home é a aba que aparece primeiro ao entrar no Workspace, sem mexer no `WorkspaceGate`.
  - **Verificado de verdade**: `cargo check`/`cargo test --lib` **170/170** (zero teste novo, mesmo padrão de `get_net_worth_summary` — agregação sem função pura), `tsc --noEmit` limpo. Testado no app real (`docker compose up`, screenshot via `spectacle`+`xdotool`): Home carrega como aba inicial com KPIs batendo exatamente com os números já conhecidos da carteira (18,04 total/-1,96 profit/+72.90% return), donut de alocação e tabela "Portfolios" corretos, card "★ Market panel" renderizando; as outras abas (testado "General Finances") continuam navegáveis normalmente.
- [x] 10.8 — **Home do Workspace, fatia 2 (personalização de layout: drag/resize/esconder)**. **Implementada e testada no app real na Sessão 85 (2026-08-22)**. 3 decisões de escopo fechadas via `AskUserQuestion` antes de planejar: (1) os 7 widgets (4 KPIs individuais + Allocation donut + Portfolios + ★ Market panel) viram unidades independentes, não um bloco só; (2) persistência em **tabela nova no banco** (`home_widget_layout`, escopada por `workspace_id`), não `localStorage` — consistente com o resto do app (zero uso de `localStorage` em qualquer lugar) e já compatível com o modelo multi-usuário futuro (Fase 10); (3) **drag + resize + esconder/mostrar**, não só reordenar.
  - **Achado que mudou a arquitetura no meio da sessão**: a primeira implementação usou `react-grid-layout` (posicionamento livre `x`/`y` com `WidthProvider`+CSS transform) — `cargo test`/`tsc` limpos, mas **travava a Home em tela preta no WebKitGTK 2.50.6** deste ambiente Docker ao entrar na aba. Isolado com testes limpos (restart completo do container antes de cada tentativa, pra não carregar estado corrompido de HMR): código original (fatia 1) renderiza bem; a mesma implementação nova só sem `<GridLayout>` (grid CSS simples) também renderiza bem; com `<GridLayout>` de volta, tela preta — mesmo sem os CSS do pacote, então não era conflito de CSS. O menu de contexto do WebKitGTK abria normal por cima da área preta (processo não travava, só o paint da lib falhava). Trocado por **`@dnd-kit`** (drag/reorder) + resize customizado (pointer events na mão, sem lib), decisão via `AskUserQuestion` com o dono do projeto.
  - **O que foi implementado**: `commands::home_layout` novo (`get_home_layout`/`save_home_layout`) e tabela `home_widget_layout` (migration `m20260822_100000`, FK Cascade → `workspace`, índice único `(workspace_id, widget_key)`, colunas `widget_key`/`position`/`w`/`h`/`visible`) — `save_home_layout` substitui o conjunto inteiro (delete + insert) em vez de fazer diff, mesmo raciocínio de `liability.rs` pras parcelas. Schema usa `position` (índice na lista reordenável) em vez de `x`/`y` livres, redesenhado depois da troca pra `dnd-kit` (lib de lista ordenável, não posicionamento 2D livre) — migration editada no lugar e reaplicada (`down`+`up`) no banco de dev, sem custo por ainda não ter sido commitada. Frontend: `HomeSection.tsx` reescrito — cada widget é um item de uma `SortableContext` do `@dnd-kit/sortable` dentro de um CSS grid comum (`grid-template-columns: repeat(12, 1fr)`, sem `position: absolute`/transform simultâneo em todos os itens ao mesmo tempo como o RGL fazia), reordena por um "grip" específico no header (não o card inteiro, pra não capturar clique do botão de esconder), redimensiona por um handle customizado no canto inferior direito (`pointerdown`/`pointermove`/`pointerup` calculando delta em unidades de coluna/linha a partir do próprio retângulo do widget). Modo "Customize layout" com estado de rascunho local (`draftLayout`) e botões Save/Cancel — só persiste no banco ao salvar; `mergeWithDefaults` cobre tanto workspace nunca customizado (lista vazia) quanto widget novo de uma fatia futura ainda não salvo.
  - **Verificado de verdade**: `cargo check`/`cargo test --lib` **170/170** (zero teste novo), `tsc --noEmit` limpo em cada etapa (RGL e depois dnd-kit), migration reaplicada e schema conferido via `sqlite3` no banco real de dev. **Testado ao vivo pelo dono do projeto** no app real (`docker compose up`) depois da troca pra `dnd-kit` — confirmado funcionando.

**Continuação do item 8 (classes de ativo expandidas), Sessão 41 (2026-08-02) — FII adicionado**: dono do projeto pediu pra continuar em português ("vamos continuar no Brasil") escolhendo FIIs como a próxima classe, entre as opções levantadas (estender busca automática, ampliar classes de ativo, ou revisão de UX). **Achado confirmado ao vivo contra a API real do Yahoo antes de codar**: `HGLG11.SA` devolve exatamente o mesmo formato de `meta` que uma ação (`instrumentType: "EQUITY"`, `longName`, `fullExchangeName`, `currency`, `regularMarketPrice`) — FII é só mais um ticker `.SA` da B3, sem nenhuma diferença de fonte de dado em relação a Ação BR. Por isso a fatia não precisou de coletor Python novo nem de migration nova (nenhuma classe adicional exige campo de schema diferente) — só passar `fii` a ser tratada onde `acao_br` já era tratada como "classe com cotação automática": `commands/asset.rs::ASSET_CLASSES` ganhou `"fii"`; `commands/profitability.rs` teve o filtro `ACAO_BR` generalizado pra uma lista `ASSET_CLASSES_WITH_AUTO_QUOTE = ["acao_br", "fii"]` (TWR/Dietz passa a cobrir FII também, já que `stock_price_history` também é alimentada pelo mesmo endpoint `.SA`); `desktop/src/portfolio/types.ts` ganhou `fii` na union `AssetClass` + o mesmo array `ASSET_CLASSES_WITH_AUTO_QUOTE` do lado TS; `AssetSection.tsx` generalizou a flag `isAcaoBr` pra `isAutoQuoteClass` (form vira busca de ticker pra qualquer classe da lista, não só Ação BR); `ProfitabilitySection.tsx` idem (`acaoBrTickers` → `autoQuoteTickers`), textos ajustados. `WatchlistSection.tsx`/`TransactionSection.tsx` não precisaram de mudança — já eram agnósticos a classe (o primeiro só busca preço por ticker, o segundo só distingue `FIXED_INCOME_CLASSES`, e FII não é renda fixa). `cargo check`/`cargo test --lib` **102/102** (sem teste novo — troca de constante/filtro, mesmo padrão de mudanças de validação anteriores) e `npx tsc --noEmit` limpos. **Ainda não testado no app real** — próxima sessão começa confirmando o fluxo (cadastrar FII por ticker em Ativos, lançar compra, ver posição/rentabilidade) com o dono do projeto rodando `docker compose up`. Restam as demais classes do item 8 (REIT/ETF/Cripto/Metal/Imóvel/Empresa não listada) pra fatias futuras — nenhuma delas compartilha a mesma vantagem de "mesmo endpoint que já existe" que FII teve.

**Continuação, Sessão 42 (2026-08-02, mesmo dia)** — indicadores fundamentalistas de FII direto da CVM: dono do projeto perguntou se dava pra fazer "pela CVM igual pra ações" (desfeita a premissa de que CVM é fonte de cotação — não é, nem pra ações, só de fundamentos) e pediu indicadores específicos de FII (vacância, inadimplência, proventos, imóveis), priorizando a CVM sobre agregador de terceiro. Pesquisa real contra `dados.cvm.gov.br/dados/FII/DOC/` achou o informe trimestral (`INF_TRIMESTRAL`, arquivo `imovel`) com **vacância e inadimplência por imóvel** exatas, mais o mensal (`INF_MENSAL`) com patrimônio líquido/VP da cota/DY mensal/nº cotistas — mas nenhum arquivo da CVM (incl. o cadastro geral de fundos, conferido) tem o ticker B3, só CNPJ. Resolvido cruzando o `openapi.json` real da bolsai (achou endpoints de FII não documentados publicamente, `/fiis/{ticker}`) pra pegar `administrator_cnpj`+nome oficial do fundo, casado **exato** (nunca fuzzy) contra `Nome_Fundo_Classe` do cadastro público da CVM — zero ou mais de um match vira `None`, nunca chuta. **CNPJ resolvido uma única vez no cadastro** (`assets.cnpj`, migration `m20260802_100000_add_cnpj_and_fii_cvm_tables` + tabelas `fii_cvm_monthly`/`fii_cvm_properties`); depois disso a bolsai nunca mais é chamada, todo indicador recorrente vem só da CVM. Proventos continuam do Yahoo (CVM não tem; testado que `events=div` já funciona pra FII). Novo `cvm_fii.py` (mesmo estilo de `cvm_dfp.py`), `commands/fii.rs` (`resolve_fii_cnpj`/`run_fii_cvm_collector`/`list_fii_cvm_monthly`/`list_fii_cvm_properties`), `commands/asset.rs::update_asset_cnpj` (comando estreito pra colar CNPJ manualmente se a sugestão falhar, já que `assets` não tem edição geral). Frontend: `AssetSection.tsx` sugere o CNPJ junto com o prefill de cotação (editável antes do submit); botão "Dados CVM" por linha de FII expande `FiiCvmDetails.tsx` com o indicador mensal mais recente + tabela de imóveis do trimestre mais recente. Tudo testado contra dados/API reais via `docker compose run` isolado (`resolve_cnpj`/`fetch_monthly_indicators`/`fetch_property_data` batendo cruzado com a bolsai — patrimônio líquido e nº de cotistas idênticos — e gravação idempotente confirmada). `cargo check`/`cargo test --lib` **102/102** e `npx tsc --noEmit` limpos. **Ainda não testado no app real** (nenhum `tauri dev` rodando durante as Sessões 41/42) — próxima sessão começa por aí.

**Continuação, Sessão 43 (2026-08-02, mesmo dia) — Fase 9 unificada pra "Pesquisa"**: dono do projeto quer o app "completo mas enxuto" — rejeitou tela separada por classe (minha sugestão inicial) e detecção automática por tentativa (ação→FII em cascata), pediu um seletor de classe explícito ao lado do ticker. "Stock Lookup" virou "Pesquisa": `StockLookupPanel.tsx` é só a moldura (busca + `Select` de classe), despachando pra `StockAnalysisSection.tsx` (Ação BR, extraída sem mudança de comportamento) ou `FiiLookupSection.tsx` (novo — cotação/CNPJ automático/indicadores CVM/imóveis/proventos/notas). Achado de arquitetura: pesquisar um FII sem cadastrá-lo como Ativo quebraria a promessa "bolsai só uma vez" da Sessão 42 (cada busca repetida chamaria de novo) — resolvido com tabela `fii_cnpj_cache` (ticker único, migration `m20260802_120000_create_fii_cnpj_cache`), consultada antes de qualquer chamada à bolsai. `AssetSection.tsx`/`FiiCvmDetails.tsx` da Sessão 42 não foram tocados (painel "Dados CVM" ali agora duplica parte do que Pesquisa mostra — decisão consciente de não mexer sem pedido). Testado no app real (dono do projeto pediu pra subir o Docker; `cargo test --lib` 102/102, `tsc --noEmit` limpo).

**Continuação, Sessão 44 (2026-08-02, mesmo dia) — bugfix: proventos de FII vinham vazios.** Achado testando a Sessão 43 no app real: `main.py::main()` sempre dava `return` cedo se a bolsai não achasse fundamentos de ação pro ticker (`/fundamentals/{ticker}`, 404 garantido pra qualquer FII, que usa `/fiis/{ticker}` à parte) — isso cortava os 3 coletores só-Yahoo (dividendo médio, técnicos, histórico de proventos) mesmo eles não dependendo da bolsai em nada. Reordenado: os 3 coletores Yahoo agora rodam incondicionalmente antes do bloco da bolsai; só o DCF (que precisa de `cvm_code` vindo da bolsai) continua atrás do early-return. Testado ao vivo: HGLG11 passou a coletar 54 pagamentos de provento; PETR4 reconfirmado sem regressão (fundamentos+DCF ainda populam). `FiiLookupSection.tsx` ganhou o tile "Provento médio/cota (5y)", par do "Avg dividend (5y)" que Ação já tinha.

**Continuação, Sessão 45 (2026-08-02, mesmo dia) — Pesquisa e Ativos convergentes.** Dois pedidos: cadastrar ativo direto pela Pesquisa, e edições em Ativos aparecerem na Pesquisa. Achado antes de codar: CNPJ de FII tinha virado 2 fontes de verdade (`assets.cnpj` vs. `fii_cnpj_cache`, esta só escrita por `resolve_fii_cnpj`) — corrigir o CNPJ em Ativos não propagava pra Pesquisa. Corrigido com `upsert_fii_cnpj_cache` em `commands/asset.rs`, chamado por `create_asset` e `update_asset_cnpj`. Novo `AddToAssetsButton` (`shared.tsx`, reaproveitado por ambas as seções) — checa `["assets"]` antes de oferecer cadastro duplicado, senão chama `create_asset` com os defaults já usados em `AssetSection.tsx`. `cargo check`/`cargo test --lib` **102/102** e `npx tsc --noEmit` limpos, testado no app real via watch do `tauri dev`.

**Continuação, Sessão 46 (2026-08-02, mesmo dia) — performance + auto-fetch dos indicadores CVM.** Dono do projeto reportou busca lenta e "VGHF11 não puxa"/"não puxa yield mensal/rentabilidade/PVP". Achado real (instrumentado com timing, não chutado): `cvm_dfp.py::fetch_roe`/`fetch_payout` sempre processavam o zip inteiro da CVM antes de checar se havia algum ticker pra buscar — pra FII, `ticker_cvm_codes` vem sempre vazio (bolsai não tem `/fundamentals` pra fundo), então era ~15s desperdiçados por busca, à toa. Guard `if not ticker_cvm_codes: return []` adicionado em `fetch_roe`/`fetch_payout`/`fetch_dcf_fundamentals` — não é bug específico de FII, o mesmo desperdício valeria pra qualquer ação se a bolsai falhasse. Testado ao vivo: VGHF11 caiu de ~20s pra ~4,9s; PETR4 sem regressão. VGHF11 em si funciona (fundo de papel, zero imóveis é esperado — não é bug). Segunda causa: `fii_cvm_monthly` (yield/rentabilidade/PVP) nunca teve auto-fetch, só botão manual — corrigido com `useEffect` em `FiiLookupSection.tsx` disparando `run_fii_cvm_collector` sozinho assim que o CNPJ resolve, mesmo padrão da cotação. `cargo check`/`cargo test --lib` **102/102** (sem mudança Rust) e `npx tsc --noEmit` limpos.

**Continuação, Sessão 49 (2026-08-02) — ETF brasileiro, fatia de cotação automática.** Dono do projeto pediu ETF como próxima classe, confirmando via `AskUserQuestion` que queria o mesmo padrão do FII (busca automática + indicador de fundo da CVM). Achado real que mudou o escopo: cotação funciona sem problema (mesmo Yahoo `.SA`, confirmado ao vivo com BOVA11), mas o cadastro geral de fundos da CVM (`cad_fi.csv`, 17MB, baixado de verdade) **não tem "ISHARES" nem uma vez** — Brasil não parece registrar ETF do mesmo jeito que FII/FIDC/FIP (que têm categoria própria em `dados.cvm.gov.br`); a bolsai também não tem CNPJ/fundamentos de ETF, só cotação (`/companies`/`/fundamentals` dão 404, `/stocks/.../quote` funciona). Achado levado ao dono do projeto antes de decidir — escolhido implementar **só a fatia de cotação automática** por ora, sem indicador de fundo (mesma decisão inicial, sem mudança de plano). `etf_br` virou classe nova em `ASSET_CLASSES` + `ASSET_CLASSES_WITH_AUTO_QUOTE` (Rust e TS) — mesmo mecanismo que `fii` já usava, sem coletor novo nem migration; TWR/Dietz (10.3) cobre ETF de graça. Testado ao vivo: `--ticker BOVA11` coletou cotação/nome/bolsa/moeda/técnicos em ~5s. `cargo check`/`cargo test --lib` **102/102** e `npx tsc --noEmit` limpos. Indicador de fundo específico pra ETF fica em aberto pra quando (se) uma fonte melhor aparecer.

**Continuação, Sessão 50 (2026-08-02) — tela de análise própria pro ETF.** Dono do projeto testou a Sessão 49 e pediu pra tirar a seção de "preço teto" (não existe pra fundo de índice) e montar algo próprio: cotação, médias de período, proventos (quando existir), e — se desse — composição de carteira/índice replicado/site do fundo. Pesquisado antes de codar: confirmado que nem Yahoo nem bolsai têm esses 3 últimos itens de graça pra ETF (mesmo achado da Sessão 49, reconfirmado) — ficam de fora, sem fonte automática conhecida. Novo `EtfLookupSection.tsx` (irmã de `StockAnalysisSection.tsx`/`FiiLookupSection.tsx`): cotação, SMA 50/100/200 + CAGR 5y/10y (`list_stock_technicals`, já coletado desde a Sessão 49, só sem UI própria), histórico de proventos (só aparece se houver pagamento), Notes — **sem** fundamentos/DCF/"Saved valuation", removidos de propósito, não é herança de `StockAnalysisSection`. `StockLookupPanel.tsx` ganhou o terceiro branch de despacho. Testado ao vivo: BOVA11 e SMAL11 com cotação/SMA/CAGR corretos, 0 proventos nos dois (plausível, ETF de índice costuma reinvestir). `npx tsc --noEmit` limpo, testado no app real via HMR.

**Continuação, Sessão 51 (2026-08-02) — Cripto vira classe de ativo, unificada em Research.** Dono do projeto pediu explicitamente pra "unificar a tela de análise de cripto na tela de research" — a antiga aba solta "Crypto Score" (9 indicadores de topo de ciclo, Fase 3, calibrados só pra ETH) era separada da Pesquisa unificada (Ação/FII/ETF). Perguntado o escopo via `AskUserQuestion` (só mover a UI vs. também virar classe de ativo no Portfolio) — escolhido o escopo maior: cripto também cadastrável em Ativos/Transações/Rentabilidade, não só a UI reorganizada. **Achado que definiu a arquitetura**: cripto não é listada na B3, então não podia reusar o endpoint Yahoo `.SA` que `acao_br`/`fii`/`etf_br` compartilham — precisou de fonte própria (CoinGecko, público sem chave, confirmado ao vivo: `/search` resolve símbolo→coin id por match exato + desempate por `market_cap_rank`, `/coins/{id}/market_chart?days=365&interval=daily` traz preço atual + histórico numa chamada só). Novo `collect_crypto_ticker` em `data-collector/main.py` (`--crypto-ticker SYMBOL`) grava nas mesmas tabelas genéricas `stock_quotes`/`stock_price_history` que os coletores Yahoo já usam — o resto do app (TWR/Dietz, `AssetSection.tsx`, Research) não precisa saber a diferença. `cripto` entrou em `ASSET_CLASSES` (Rust) e `ASSET_CLASSES_WITH_AUTO_QUOTE` (Rust + TS), exposição default `categoria_especial`/`crypto` (não `pais`, diferente das classes B3). `run_stock_collector` ganhou `asset_class: Option<String>` pra rotear `--crypto-ticker` vs `--ticker` sem duplicar comando. Como cripto se auto-backfila (preço + histórico na mesma chamada), o botão manual "Update price history" do Portfolio (Yahoo-only) passou a filtrar por uma lista própria (`YAHOO_BACKFILL_CLASSES`), não `ASSET_CLASSES_WITH_AUTO_QUOTE` inteiro. Novo `CryptoLookupSection.tsx` em Research (4ª opção no seletor de classe) absorve o antigo `CryptoScorePanel.tsx` (removido, junto com a aba "Crypto Score" do nav principal) — o coletor automatizado dos 9 indicadores continua hardcoded pra ETH no backend (não mudou), então só aparece buscando ETH; registro manual continua livre pra qualquer moeda, como já era. `cargo test --lib` **102/102**, `npx tsc --noEmit` limpo. **Testado no app real** pelo dono do projeto via HMR do `tauri dev`: "funcionou sim".

**Mesma sessão, pedido seguinte**: testando ao vivo, pediu SMA 50/100/200 (mesmo padrão que Ação/ETF já mostram) e um **Fear & Greed Index global** aparecendo "em toda tela que for cripto". SMA não precisou de coletor novo — computada client-side em cima do mesmo histórico de 365 dias que `collect_crypto_ticker` já grava. Fear & Greed: nova fonte `alternative.me` (`GET /fng/`, pública sem chave, confirmada ao vivo — índice de mercado, não por moeda), nova tabela `crypto_fear_greed` (migration `m20260802_140000_create_crypto_fear_greed`, índice único em `reading_date`), atualizada de graça junto de qualquer refresh de ticker cripto (mesma chamada de `collect_crypto_ticker`). Novo componente `CryptoFearGreedGauge` em `shared.tsx`, self-contained (própria query, cacheada por `["crypto-fear-greed"]`) — pensado pra "toda tela de cripto futura" só precisar renderizar `<CryptoFearGreedGauge />` sem prop nenhuma. `cargo test --lib` 102/102, `npx tsc --noEmit` limpo, migration aplicada e entity regerada contra o banco real (`sea-orm-cli` 1.1.20 confirmado antes de gerar), testado ao vivo (`ETH (Ethereum): US$ 1877.99`, `Fear & Greed: 27 (Fear)`).

**Marco**: Fase 10, item 8 ganha Cripto como quarta classe expandida (depois de FII/ETF) — primeira a não compartilhar a fonte Yahoo `.SA` das outras três, confirmando que o desenho "cada classe pode ter sua própria fonte" (rascunho original do item 8) funciona na prática sem quebrar o resto do app. A antiga aba "Crypto Score" deixa de existir como tela própria; seu conteúdo (indicadores de ciclo ETH) agora mora dentro de Research.

**Continuação, Sessão 52 (2026-08-02) — gráfico de preço, unificado em toda classe.** Dono do projeto pediu um "gráfico completo e simples" de preço pra todos os ativos, "inclusive daqui pra frente" (qualquer classe futura) — explicitamente **não** querendo algo no estilo TradingView (zoom livre, candlestick, indicadores sobrepostos). Achado antes de codar: `stock_price_history` só era populada pro Yahoo (`acao_br`/`fii`/`etf_br`) pelo botão manual de backfill do Portfolio, nunca automaticamente numa busca da Pesquisa — diferente de Cripto, que desde a Sessão 51 já grava histórico de graça em toda busca (`collect_crypto_ticker`). Corrigido generalizando esse mesmo comportamento: `main()` (fluxo `--ticker`, usado por Ação/FII/ETF) ganhou uma chamada incondicional a `collect_price_history([ticker])`, no mesmo bloco dos outros coletores só-Yahoo que já rodam sempre (dividendos/técnicos/proventos) — testado ao vivo (PETR4 com histórico já backfillado antes, 3 pontos novos do dia; XPML11, nunca buscado, populou 1534 pontos na primeira busca). Novo `PriceHistoryChart.tsx` (Recharts `LineChart`, mesma paleta/`Tooltip` customizado já usados em `ProfitabilitySection.tsx`/`DividendHistoryChart.tsx` — nenhuma lib nova) — uma linha só, sem eixo duplo, sem candlestick. Reaproveitado idêntico em `StockAnalysisSection.tsx`/`FiiLookupSection.tsx`/`EtfLookupSection.tsx`/`CryptoLookupSection.tsx` (cada seção só passa `history` + `currencyPrefix`, já buscando via o comando genérico `list_stock_price_history`).
**Pedido seguinte, mesma sessão**: testando ao vivo, pediu seletor de período — lista exata "ultimos 30 dias, ultimos 12 meses, este ano, 5y, 10y, alltime". Implementado dentro do próprio `PriceHistoryChart.tsx` como recorte client-side (`filterByPeriod`) em cima do `history` já buscado, sem requisição nova por período — mesmo padrão de toggle (`Button` default/outline) que `DividendHistoryChart.tsx` já usa pro Monthly/Yearly. Um período maior que o histórico disponível (ex.: "10Y" numa cripto, limitada a 365 dias pela CoinGecko) simplesmente devolve tudo que existe, sem erro. Default "12M". `npx tsc --noEmit` limpo nas duas rodadas (sem mudança Rust). Testado ao vivo pelo dono do projeto via HMR do `tauri dev`: "mt bom".

**Marco**: gráfico de preço, com seletor de período, agora é parte padrão de qualquer classe com cotação automática — próxima classe que ganhar auto-quote (Fase 10, item 8) herda o gráfico de graça, só precisa passar `history`/`currencyPrefix` pro mesmo componente.

**Continuação, Sessão 53 (2026-08-02) — BDR como quinta classe expandida.** Dono do projeto pediu BDR (recibo negociado na B3 representando empresa estrangeira, ticker `XXXX34`, ex.: `AAPL34` = Apple) como próxima classe. **Achado que definiu o escopo, confirmado ao vivo antes de codar**: BDR bate no mesmo endpoint Yahoo `.SA` que `acao_br`/`fii`/`etf_br` já usam — `docker compose run` do coletor Python existente, **sem nenhuma mudança de código**, contra `--ticker AAPL34` populou cotação (R$77,82), dividendo médio (R$0,2145/5y), técnicos (SMA200 R$72,34, CAGR10y 33,6%), 40 pagamentos de provento e 2492 dias de histórico de preço em ~5s (`fullExchangeName: "São Paulo"`, `currency: "BRL"`, `longName: "Apple Inc."`). A bolsai (`/fundamentals/AAPL34`) devolve 404 (confirmado ao vivo) e já é pulada de forma barata pelo guard genérico que a Sessão 46 adicionou em `cvm_dfp.py` (não é código novo, BDR só se beneficia dele de graça). **Conclusão: zero coletor Python novo, zero migration** — só declarar a classe + montar a tela de Research, mesmo molde de ETF (Sessões 49-50).

**Decisão de exposição, perguntada ao dono do projeto via `AskUserQuestion`**: diferente de Ação/FII/ETF (que default exposição "BR"), BDR representa empresa estrangeira — o Yahoo não devolve o país real por trás do ticker (só o nome, ex.: "Apple Inc."), então o padrão escolhido foi **exposição "US"**, editável (a maioria esmagadora dos BDRs na B3 é de empresa americana). **Ideia trazida pelo próprio dono do projeto, registrada pra depois, não implementada agora**: um sistema de "tipo de ativo subjacente" mais completo (metal/cripto/ação-stock/REIT-FII/ETF), já que um BDR pode representar qualquer um desses, não só ação — extensão futura da distinção "listagem ≠ exposição ≠ classe" do rascunho original do item 8 (ver `ROADMAP.md`).

**Código**: `bdr` entrou em `ASSET_CLASSES` (`commands/asset.rs`, 7→8) e `ASSET_CLASSES_WITH_AUTO_QUOTE` (`commands/profitability.rs`, 4→5, TWR/Dietz cobre BDR de graça) — `commands/collector.rs` não mudou, `run_stock_collector` só especializa `asset_class == Some("cripto")`, BDR cai no fluxo Yahoo já existente passando `asset_class: null`. TS: `AssetClass`/`ASSET_CLASSES`/`ASSET_CLASS_LABELS` (`bdr: "BDR (B3)"`)/`ASSET_CLASSES_WITH_AUTO_QUOTE` em `portfolio/types.ts`; `YAHOO_BACKFILL_CLASSES` em `ProfitabilitySection.tsx` ganhou `"bdr"` (é Yahoo-backed, diferente de cripto que se auto-backfila). Novo `BdrLookupSection.tsx` — cópia adaptada de `EtfLookupSection.tsx` (mesma ausência de fundamentos/DCF/"preço teto", mesmo motivo: bolsai não tem pra nenhum dos dois), com `AddToAssetsButton` passando `exposureType="pais"`/`exposureValue="US"`. **Decisão de estrutura**: arquivo novo, não componente compartilhado com ETF — mantém o precedente do projeto de não abstrair sobreposição entre seções de classe (FII/ETF/Ação/Cripto já são 4 arquivos separados apesar de overlap real); se uma terceira classe idêntica aparecer, aí sim vale extrair. `AssetSection.tsx`/`StockLookupPanel.tsx` ganharam o branch `bdr`/placeholder `AAPL34`, mesmo padrão de toda classe anterior.

**Verificação**: `cargo check`/`cargo test --lib` **102/102** (sem teste novo, mesmo padrão de toda adição de classe) e `npx tsc --noEmit` limpos. Reconfirmado ao vivo pós-mudança: `--ticker AAPL34` idempotente (0 pontos novos, já tinha rodado antes). Rust recompilou e relançou sozinho (watch do `tauri dev`), frontend via HMR do Vite, sem reiniciar o container.

**Marco**: Fase 10, item 8 ganha BDR como quinta classe expandida — a mais barata de implementar até agora (zero coletor, zero migration), puro reaproveitamento da infra Yahoo `.SA` + do gráfico de preço genérico da Sessão 52. Fica registrada a ideia de "tipo de ativo subjacente" pra quando (se) o dono do projeto quiser essa fatia.

**Continuação, Sessão 55 (2026-08-02) — Metal (ouro) como sexta classe expandida.** Perguntei ao dono do projeto "pra onde vamos agora" (via `AskUserQuestion`, opções: mais uma classe de ativo, revisão de UX da Fase 10, continuar Fase 8) — escolheu mais uma classe, especificamente Metal ("bem mais fácil"). **Achado que definiu a arquitetura, confirmado ao vivo antes de codar**: metal não é negociado na B3, então não bate no endpoint Yahoo `.SA` que Ação/FII/ETF/BDR compartilham — mas o Yahoo tem o contrato futuro de ouro do COMEX (`GC=F`, sem `.SA`), com o mesmo shape de cotação+histórico do resto do projeto (`meta.regularMarketPrice` + `range=10y&interval=1d`, 2517 pontos testados ao vivo). Perguntado ao dono do projeto se começava só com ouro ou já com prata também — escolheu **só ouro (`XAU`) por ora**, prata fica fácil de adicionar depois (só mais uma entrada num dicionário ticker→símbolo Yahoo).

**Segunda decisão de arquitetura**: o contrato do COMEX cota em USD por **onça troy**, mas o cadastro deste app usa **grama** pra metal (pedido original do rascunho da Fase 10, Sessão 30) — convertido **na fonte** (`sources/metais_yahoo.py::fetch_quote_and_history`, uma constante `TROY_OUNCE_GRAMS = 31.1034768`), então o resto do app (`stock_quotes`/`stock_price_history`, TWR, `PriceHistoryChart.tsx`) nunca precisa saber disso — grava sempre preço/grama, igual qualquer outro `price`/`close_price` do projeto. **Nenhuma migration nova precisou existir**: `transactions.quantity` já é `f64` (mesmo mecanismo que cripto/BDR usam), então "gramas" é só uma unidade que o usuário escolhe mentalmente na hora de lançar a compra — confirma que a ideia original de um campo `unidade_medida` (rascunho da Sessão 30) não era necessária pro MVP, só pra rotular a UI, o que não foi feito ainda (quantidade aparece como número puro, sem sufixo "g").

**Código**: `data-collector/sources/metais_yahoo.py` (novo) — `TICKER_TO_YAHOO_SYMBOL = {"XAU": "GC=F"}` é o único lugar a mudar pra adicionar outro metal; `fetch_quote_and_history(ticker)` faz uma chamada só (quote + 10y de histórico, já convertidos onça→grama), levanta `RuntimeError` claro pra ticker não suportado (nunca chuta). `main.py::collect_metal_ticker`/`main_metal_ticker` (mesmo papel de `collect_crypto_ticker` da Sessão 51), novo flag `--metal-ticker`. Testado ao vivo: `XAU` → US$132,88/g, 2513 pontos de histórico na primeira busca, idempotente num segundo run. `commands/collector.rs::run_stock_collector` virou um `match` de 3 braços (`cripto`→CoinGecko, `metal`→COMEX via Yahoo sem `.SA`, resto→Yahoo `.SA`). `metal` entrou em `ASSET_CLASSES` (Rust) e `ASSET_CLASSES_WITH_AUTO_QUOTE` (Rust+TS) — TWR/Dietz cobre metal de graça, mesma tabela genérica. **Não** entrou em `YAHOO_BACKFILL_CLASSES` (`ProfitabilitySection.tsx`) — mesmo motivo de cripto, se auto-backfila na mesma chamada que traz a cotação. Exposição default `categoria_especial`/`gold_metal` (`AssetSection.tsx`, nem `pais`/"BR" nem "US" fazem sentido pra metal — reusa o valor de exemplo que a própria UI já sugeria desde a Sessão 29: "e.g.: crypto, gold_metal"). Novo `MetalLookupSection.tsx` (irmã de `CryptoLookupSection.tsx`/`BdrLookupSection.tsx`): preço/grama, variação 7/30/90/365d e SMA 50/100/200 (mesmos helpers já usados em Cripto, duplicados aqui sem abstração — mesmo precedente do projeto), `PriceHistoryChart` reaproveitado sem mudança, Notes — **sem** fundamentos/DCF/dividendos (metal não paga provento) nem painel de indicadores de ciclo (não se aplica).

**Verificação**: `cargo check`/`cargo test --lib` **102/102** (sem teste novo) e `npx tsc --noEmit` limpos. Coletor testado ao vivo contra a API real duas vezes (idempotência confirmada). Rust recompilou e relançou sozinho (watch do `tauri dev`), frontend via HMR do Vite, sem reiniciar o container.

**Marco**: Fase 10, item 8 ganha Metal como sexta classe expandida — segunda (depois de Cripto) a não compartilhar a fonte Yahoo `.SA` das classes B3, e a primeira a fazer conversão de unidade na fonte (onça→grama) em vez de gravar o dado cru. Prata (e outros metais) fica pronta pra entrar numa fatia futura só editando `TICKER_TO_YAHOO_SYMBOL`.

**Continuação, Sessão 57 (2026-08-02) — unidade revertida pra onça troy + Prata/Platina/Paládio.** Dono do projeto testou a Sessão 55 e pediu o oposto da conversão original: "queria que fosse sempre em onça, que tivesse apenas um indicador simples de dol/g de ouro" — a unidade real do contrato COMEX/NYMEX é onça troy, não grama; grama vira só um indicador auxiliar de referência na tela, não a base gravada. Pediu também mais metais — confirmado ao vivo antes de codar, via `AskUserQuestion`: **Prata (`SI=F`), Platina (`PL=F`) e Paládio (`PA=F`)** seguem exatamente o mesmo contrato COMEX/NYMEX cotado em USD/onça troy que o ouro (testado ao vivo os 4 — `XAU` US$4114,80/oz, `XAG` US$58,21/oz, `XPT` US$1662,20/oz, `XPD` US$1292,00/oz). Cobre (`HG=F`) existe mas cota em USD/libra-peso — decisão explícita de deixar de fora (unidade diferente dos outros 4).

**Achado de segurança de dado, antes de mexer no código**: nenhum `asset`/`transaction` de classe `metal` existia no banco de dev (checado direto via sqlite) — só ~2513 linhas de `stock_quotes`/`stock_price_history` do teste da Sessão 55, já em grama, sob o ticker `XAU`. Como `stock_price_history` usa `INSERT OR IGNORE` num índice único `(ticker, price_date)`, só trocar a fonte pra gravar onça deixaria o histórico **misturado** (dias antigos em grama sobrevivendo, silenciosamente, por trás do índice único) — confirmado na prática (`0 new` ao rodar `--metal-ticker XAU` de novo antes da limpeza). Linhas antigas apagadas manualmente via sqlite antes de re-testar; sem risco, dado de dev descartável.

**Código**: `sources/metais_yahoo.py` perdeu a divisão por `TROY_OUNCE_GRAMS` — grava o preço cru que o COMEX/NYMEX já devolve. `TICKER_TO_YAHOO_SYMBOL`/`METAL_NAMES` ganharam `XAG`/`XPT`/`XPD` (códigos ISO 4217 de metal precioso, mesma convenção de `XAU`) — zero mudança em Rust (`run_stock_collector`'s branch `"metal"` já roteia qualquer ticker genericamente). `MetalLookupSection.tsx`: tile principal virou "Price (oz)", novo tile "Price (g)" (convertido só na exibição, `TROY_OUNCE_GRAMS` agora client-side e só decorativo), SMA 50/100/200 relabeladas "(oz)". **Achado à parte, corrigido no caminho**: `exposureValue` do `AddToAssetsButton` estava hardcoded `"gold_metal"` desde a Sessão 55 (quando só ouro existia) — generalizado pra um mapa por ticker (`silver_metal`/`platinum_metal`/`palladium_metal`), senão prata/platina/paládio cadastrados via Research herdariam a exposição errada de ouro.

**Verificação**: coletor testado ao vivo pros 4 metais (preços plausíveis, nomes corretos). `npx tsc --noEmit` limpo (sem mudança Rust nesta fatia). Frontend via HMR do Vite, sem reiniciar o container, sem erro.

**Marco**: Metal agora cobre os 4 metais preciosos "clássicos" (ouro/prata/platina/paládio), todos na unidade real do contrato (onça troy), com um indicador simples de grama só de referência — cobre fica registrado como candidato futuro se o dono do projeto quiser lidar com a unidade em libra-peso depois.

**Continuação, Sessão 58 (2026-08-03) — Ação americana (`acao_internacional`), Fatia 1: cotação/técnicos/dividendos.** Dono do projeto pediu pra estudar como rastrear ações americanas (NYSE/NASDAQ) com fundamentos vindos da SEC (equivalente americano da CVM), Yahoo como fallback. Pesquisa em paralelo (2 agentes) antes de planejar: um releu o pipeline BR inteiro (bolsai + `cvm_dfp.py`), outro pesquisou ao vivo a API XBRL da SEC (`data.sec.gov/api/xbrl/companyfacts`, gratuita, sem chave, só exige `User-Agent` com e-mail e respeita 10 req/s; ticker→CIK via `company_tickers.json`) — confirmou que o mapeamento de tags XBRL (`Revenues` vs `RevenueFromContractWithCustomerExcludingAssessedTax`, ano fiscal não-calendário tipo Apple, reformulações) é mais inconsistente entre empresas do que os códigos de conta padronizados da CVM, risco real de mapear errado sem testar contra várias empresas. Decisão confirmada com o dono do projeto: dividir em 2 fatias — **Fatia 1** (esta sessão) só cotação/técnicos/dividendos/histórico via Yahoo, sem fundamentos, molde do BDR; **Fatia 2** (futura) SEC EDGAR + DCF.

**Achado que corrigiu a suposição inicial**: `acoes_yahoo.py` (caminho genérico `--ticker`, usado por acao_br/fii/etf_br/bdr) tinha `.SA` **hardcoded** nas 5 funções — não era reaproveitável pra ticker americano sem mudança (diferente de `metais_yahoo.py`, que já busca sem sufixo mas é um módulo à parte). Corrigido com um parâmetro `suffix: str = ".SA"` nas 5 funções (`fetch_quotes`/`fetch_price_history`/`fetch_dividends_avg`/`fetch_technicals`/`fetch_dividend_payments`), chamado com `suffix=""` pra ação americana — os 4 chamadores BR existentes continuam sem mudança de comportamento (default preserva `.SA`). `acao_internacional` já existia como valor placeholder em `ASSET_CLASSES`/`AssetClass` desde a Fase 10 original (Sessão 33), mas sem nenhuma fiação — ausente de `ASSET_CLASSES_WITH_AUTO_QUOTE` (Rust e TS), sem branch no coletor, sem UI.

**Código**: `main.py` ganhou `collect_us_stock_quotes`/`collect_us_stock_dividends_avg`/`collect_us_stock_technicals`/`collect_us_stock_dividend_payments`/`collect_us_price_history` (espelham as 5 funções BR equivalentes, `suffix=""`) e `main_us_stock(ticker)` (mesmo desenho de `main()`, só os 4 passos Yahoo, sem bloco de fundamentos/CVM), novo flag `--us-ticker`. `commands/collector.rs::run_stock_collector` ganhou o braço `Some("acao_internacional") => ... "--us-ticker" ...`. `commands/profitability.rs::ASSET_CLASSES_WITH_AUTO_QUOTE` (6→7) e `portfolio/types.ts::ASSET_CLASSES_WITH_AUTO_QUOTE` ganharam `acao_internacional` (TWR/Dietz cobre ação americana de graça). Novo `UsStockLookupSection.tsx` — cópia adaptada de `BdrLookupSection.tsx` (mesma ausência de fundamentos/DCF nesta fatia, mas por motivo diferente do BDR: não é "a fonte não tem", é "a fonte certa — SEC EDGAR — ainda não foi construída"), formatador de moeda local `formatUsd` (mesmo padrão de `MetalLookupSection.tsx::formatUsdPerOz`, já que `shared.tsx::formatCurrency` é fixo em R$), `AddToAssetsButton` com `asset_class: "acao_internacional"` explícito (diferente de BDR/ETF que usam `asset_class: null`, pois `acao_internacional` precisa do roteamento explícito pro `--us-ticker`) e exposição default `pais`/`"US"`, mesmo padrão do BDR. `StockLookupPanel.tsx`/`AssetSection.tsx` ganharam o branch + placeholder `AAPL`, mesmo padrão de toda classe anterior. **Decisão confirmada com o dono do projeto**: o botão de backfill manual (`run_price_history_backfill`/`--price-history`, `YAHOO_BACKFILL_CLASSES`) fica de fora por ora — não recebe classe do ativo, não dá pra saber o sufixo certo; a busca automática da tela Research já cobre preço/histórico.

**Verificação**: `cargo check`/`cargo test --lib` **102/102** (sem teste novo) e `npx tsc --noEmit` limpos. Coletor testado ao vivo contra a API real duas vezes com tickers diferentes (`AAPL` US$308,91, `MSFT` US$464,72, ambos `currency: "USD"`/`exchange: "NasdaqGS"` confirmando que o sufixo `.SA` não vazou), idempotência confirmada rodando `AAPL` de novo (`0 new`). Rodado dentro do container via `docker compose run --rm` (sem subir a GUI) — `cargo check`/`test`/`tsc` também rodados assim.

**Marco**: Fase 10, item 8 ganha Ação americana como sétima classe expandida (fatia de cotação) — reaproveita a infra Yahoo genérica parametrizada em vez de duplicar módulo, mesmo precedente que motivou a correção do `suffix`. Fundamentos via SEC EDGAR ficam documentados como Fatia 2, ainda não iniciada — o risco de mapeamento de tags XBRL foi isolado de propósito numa sessão futura dedicada, com pelo menos um ticker de ano fiscal não-calendário (Apple) e um com reformulação (10-K/A) no plano de teste.

**Continuação, Sessão 59 (2026-08-03) — Ação americana, Fatia 2: fundamentos + DCF via SEC EDGAR.** Dono do projeto pediu pra continuar ("bora continuar pra fatia dois"). Planejado via `/plan`: pesquisa ao vivo confirmou que a SEC EDGAR (`data.sec.gov/api/xbrl/companyconcept/CIK{cik}/us-gaap/{tag}.json`) cobre numa fonte só tanto os campos "tipo bolsai" (LPA/VPA/ROE/nº de ações) quanto os 9 campos do DCF — diferente do Brasil, que precisa de bolsai + CVM separados —, então a fatia virou **paridade completa** com Ação BR (não só DCF), confirmado com o dono do projeto via `AskUserQuestion`.

**Dois achados críticos só descobertos testando ao vivo (não estavam no plano original)**: (1) filtrar só `form=="10-K" and fp=="FY"` não basta pra tags de fluxo (duration) — a Apple usa a mesma tag `Revenues`/`PaymentsOfDividendsCommonStock` nas notas de rodapé "dados trimestrais selecionados" dentro do próprio 10-K, então a maioria das linhas vinha trimestral mesmo com `form`/`fp` corretos; corrigido exigindo `350 <= (end-start).days <= 380`. (2) a tag `Revenues` da Apple só tem dado real até o exercício de 2018 (a empresa migrou pra `RevenueFromContractWithCustomerExcludingAssessedTax` depois da ASC 606) — a estratégia inicial "primeira tag da lista com qualquer dado vence" pegava 2018 em vez do ano corrente disponível na tag mais nova; corrigido pra "entre todas as tags candidatas, pega a linha mais recente" (`_try_tags`), não a primeira que resolver. Sem esse segundo fix, a receita da Apple aparecia como US$265,6bi (2018) em vez de US$416,2bi (2025) — confirmado ao vivo antes/depois da correção.

**Gap de taxonomia de banco, confirmado ao vivo (espelha o achado da CVM com COSIF)**: testando a JPMorgan real, `OperatingIncomeLoss`/`InventoryNet`/`AccountsReceivableNetCurrent`/`AccountsPayableCurrent`/`PaymentsToAcquirePropertyPlantAndEquipment`/`LongTermDebtNoncurrent`/`RevenueFromContractWithCustomerExcludingAssessedTax` devolvem 404 — banco não reporta EBIT/estoque/contas a receber-pagar/capex/receita-de-contrato do jeito que empresa não-financeira reporta. Tratado como esperado (mesmo espírito do `LookupError`/skip da CVM): `fetch_dcf_fundamentals` descarta o ticker inteiro, `fetch_fundamentals` (LPA/VPA/ROE) continua funcionando — confirmado ao vivo (JPM ganhou linha em `stock_fundamentals`, zero em `stock_dcf_fundamentals`).

**Código**: novo `data-collector/sources/sec_edgar.py` — resolução CIK via `company_tickers.json` (cache com TTL de 7 dias, diferente do cache eterno do zip da CVM, já que o arquivo não tem versão no nome), rate limit de 0,11s/requisição (~9 req/s, sob o teto de 10), `_required`/`_optional` espelhando `_find_exact`/`_find_by_keyword` da CVM, `_effective_tax_rate` com o mesmo teto de sanidade de 100%. `fetch_fundamentals`/`fetch_dcf_fundamentals`/`fetch_payout` com o mesmo shape dos pares bolsai/CVM. `total_debt = LongTermDebtNoncurrent (obrigatório) + LongTermDebtCurrent (opcional, default 0 — "sem parcela circulante" é fato de negócio legítimo)`. D&A tentou 3 tags combinadas + descobriu uma 4ª (`Depreciation`) testando a Microsoft, que reporta só depreciação de imobilizado separada da amortização de intangíveis. `main.py` ganhou `collect_us_stock_fundamentals`/`collect_us_stock_dcf_fundamentals` (mesmas tabelas/colunas de `stock_fundamentals`/`stock_dcf_fundamentals`, `source="sec_edgar"`) e `main_us_stock` passou a chamá-las depois do bloco Yahoo, mesmo padrão try/except-skip-barato de `main()`. Corrigido também um bug real achado nesta sessão: `useTickerCollector.ts` sempre chamava `run_stock_collector` com `asset_class: null`, então o botão "Fetch" de qualquer form de valuation buscaria um ticker americano pelo caminho `.SA` errado — parametrizado com `assetClass`, propagado pelos 8 forms/`registry.ts`/`NewValuationDialog.tsx`. `UsStockLookupSection.tsx` ganhou os blocos "Fundamentals"/"DCF fundamentals"/"Saved valuation" (New Valuation/All Saved Valuations), espelhando `StockAnalysisSection.tsx` mas mantendo `formatUsd` local em vez de virar componente compartilhado (mesmo precedente de BDR/Metal).

**Verificação**: testado ao vivo contra a API real da SEC (dentro do container via `docker compose run --rm`) — `AAPL` (EBIT US$133,05bi, receita US$416,16bi, payout 15,08%, ano fiscal 2025 corretamente não-calendário), `MSFT` (D&A US$34,3bi via a 4ª tag, capex US$115,95bi, ano fiscal 2026), `JPM` (fundamentos ok, DCF vazio como esperado). Idempotência confirmada (rodar `AAPL` de novo não duplica/não quebra). Conferido direto no SQLite. `cargo check`/`cargo test --lib` **102/102** e `npx tsc --noEmit` limpos. App real (`docker compose up`) subiu sem erro (ruído esperado de MESA/libGL/dbind sem GPU) — fluxo de clique (buscar AAPL/JPM em Research, abrir New Valuation, clicar Fetch, Calculate, Save) fica pro dono do projeto testar.

**Marco**: Fase 10, item 8 fecha a Ação americana com paridade completa à Ação BR — os 8 modelos de valuation agora funcionam pra qualquer ticker americano com cobertura XBRL suficiente, com degradação graciosa documentada e testada pra bancos.

**Continuação, Sessão 60 (2026-08-03) — "Sobre o ativo": resumo gerado por IA em Research, pra todas as 7 classes de ativo.** Dono do projeto pediu uma seção de texto qualitativo (o que a empresa/fundo/ativo faz, história, contexto) na tela de análise, mas recusando preenchimento manual (não escala, fica desatualizado) — proposta e aceita: gera via IA na primeira visita, fica cacheado no banco por `(ticker, asset_class)`, com botão "Regenerate" que abre um popup pedindo o motivo (o motivo entra no prompt junto com o texto anterior, pra IA corrigir). Usa a API de IA já configurada pelo usuário em Configurações (Claude/OpenAI/Gemini, infra da Fase 7) — sem key cadastrada, a seção simplesmente não aparece, sem erro nem aviso.

**Achado de planejamento**: nenhuma infra nova precisou ser criada — toda a base de IA (tabela `ai_api_key` + segredo no keyring do SO, os 3 clients HTTP em `chat.rs`, o padrão de auto-seleção de key/modelo do `ChatPanel.tsx`) e o padrão de cache por ticker (`stock_notes`) já existiam prontos, só precisaram ser recombinados.

**Código**: nova tabela `company_ai_info` (migration `m20260803_120000_create_company_ai_info`) — `(ticker, asset_class)` como chave lógica (não `ticker` sozinho, pra não ambiguar entre classes e porque o enquadramento do prompt muda por classe), `content`/`provider`/`model`/`generated_at`/`regenerate_reason`. `chat.rs` ganhou `generate_completion` — uma variante de `generate_reply` sem o contexto de valuations/alertas nem a tool `propose_valuation`, reaproveitando os mesmos clients privados `ask_gemini_api`/`ask_claude_api`/`ask_openai_api`. Novo `commands/company_ai_info.rs`: `get_company_ai_info` (leitura) e `generate_company_ai_info` (monta o prompt por `asset_class` — empresa listada / fundo / protocolo cripto / commodity —, chama `generate_completion`, faz upsert find-then-write igual `stock_notes.rs`). Novo componente compartilhado `AboutCompanySection.tsx` — esconde a seção inteira se `list_api_keys` vier vazia, gera sozinho na primeira visita (sem popup, nada a corrigir ainda), Dialog de motivo só no botão Regenerate — plugado nas 7 seções de Research (`StockAnalysisSection`/`UsStockLookupSection`/`FiiLookupSection`/`EtfLookupSection`/`CryptoLookupSection`/`BdrLookupSection`/`MetalLookupSection`).

**Verificação**: `cargo check`/`cargo test --lib` **102/102** e `npx tsc --noEmit` limpos, tanto no crate principal quanto no de migration. Migration aplicada contra o banco real de dev (`sea-orm-cli migrate up` via `docker compose run`, `DATABASE_URL` apontado pro mesmo arquivo que `db.rs` usa — achado de sessão: o app não roda migration sozinho no startup, sempre manual), tabela conferida direto no SQLite. App real (`docker compose up`, já rodando) recompilou e reiniciou sozinho sem erro (hot reload do Tauri dev pegou os 7 arquivos de frontend mudados).

**Bug da Sessão 60, resolvido na Sessão 61 (2026-08-03)**: a causa real não foi a hipótese líder registrada (erro invisível de `infoQuery`) — era `AboutCompanySection.tsx` chamando `invoke("get_company_ai_info", { ticker, asset_class: assetClass })`. O Tauri converte automaticamente nomes de parâmetro de comando pra camelCase no lado JS (confirmado comparando com `ChatPanel.tsx`, que já chama `ask_ai` com `keyId` pro parâmetro Rust `key_id`), então a chave `asset_class` nunca batia com o `assetClass` esperado — a leitura falhava, e o efeito de auto-geração (que depende de `infoQuery.isSuccess`) nunca disparava. `generate_company_ai_info` não tinha esse problema porque seu único parâmetro de nível superior é o objeto `request` inteiro; os campos snake_case *dentro* dele passam pelo serde normal do struct, sem a conversão de camelCase do Tauri (que só se aplica a nomes de parâmetro de nível superior). Fix de uma linha; testado ao vivo — `company_ai_info` foi de 0 pra 1 linha (BBAS3) depois do teste do dono do projeto.

**Refinamento de formatação, mesma sessão**: dono do projeto pediu texto mais legível/"bonitinho". `SYSTEM_INSTRUCTION` em `company_ai_info.rs` passou a pedir Markdown (subtítulos `###`, negrito pontual, listas com `-`) e `AboutCompanySection.tsx` ganhou `react-markdown`+`remark-gfm` com componentes estilizados próprios (não reusa o mapa de `markdown-text.tsx`, acoplado ao `MarkdownTextPrimitive` do assistant-ui). Primeira iteração ainda saiu densa (1 parágrafo de 3-4 frases por seção) — prompt reforçado pra tornar lista o formato padrão (no máx. 1 frase de abertura + lista de 2 a 4 itens de 1 frase cada, proibindo parágrafos encadeados), mais respiro visual nas margens dos componentes. Confirmado pelo dono do projeto ("ficou mt legal") depois da segunda iteração.

**Continuação, mesma sessão — "About the asset" reposicionado + criar Teses direto de Research.** Dois ajustes pedidos testando ao vivo: (1) mover `AboutCompanySection` pra **depois** do bloco de Valuation nas telas de Ação BR/americana (antes vinha logo após os `StatTile`s, acima de Valuation); (2) criar/editar Teses (Fase 10.5) **direto da tela de análise**, sem precisar ir em Portfolio → Teses. Novo `AssetThesesSidebar.tsx` reaproveita `list_theses`/`create_thesis`/`update_thesis` já existentes da Fase 10.5, sem tabela nem comando novo no backend — `list_theses` só filtra por `workspace_id`, o filtro por ticker é client-side (`thesis.asset_id` comparado ao `id` resolvido em `assets` pelo ticker). Se o ativo ainda não estiver cadastrado em `assets`, criar a primeira tese registra ele primeiro, silenciosamente (mesmo espírito "sem BO" do `AboutCompanySection`). Plugada só em `StockAnalysisSection.tsx`/`UsStockLookupSection.tsx` (Ação BR/americana) — FII/ETF/Cripto/BDR/Metal não ganharam a sidebar de teses nesta sessão. `StockLookupPanel.tsx`/`App.tsx` passaram a repassar `workspaceId` até essas duas seções. `npx tsc --noEmit` limpo (sem mudança Rust). **Sem confirmação registrada de teste ao vivo no app rodando** — pendência pra próxima sessão.

**Marco**: Fase 10, item 8 ("Sobre o ativo" via IA) fecha de ponta a ponta — implementação, bug de auto-geração e formatação, todos confirmados funcionando pelo dono do projeto no app real. Teses (Fase 10.5) ganham um segundo ponto de entrada — direto da tela de análise de Ação BR/americana, sem sair de Research — ainda sem confirmação de teste ao vivo.

**Continuação do item 8, Sessão 62 (2026-08-03) — REIT adicionado.** Dono do projeto pediu pra implementar REIT "seguindo a mesma ideia de buscar informações e indicadores iguais aos dos fiis". Planejado via `/plan`, 2 agentes em paralelo antes de desenhar: um mapeou FII/Ação americana ponta a ponta, outro verificou ao vivo contra a SEC/Yahoo se FFO/AFFO/ocupação dão pra automatizar. **Achado crítico**: confirmado contra 4 REITs reais (Realty Income, Simon Property, Prologis, AvalonBay) que FFO/AFFO e taxa de ocupação — os indicadores "de verdade" de REIT — **não existem como tag XBRL na SEC**, só non-GAAP em texto do 10-K. Automatizável de graça: cotação/dividendo mensal/histórico/DY via Yahoo (mesmo pipeline de Ação americana) e Receita/valor de imóveis/PL/LPA via SEC XBRL (lucro líquido com fallback `NetIncomeLoss`→`ProfitLoss`, achado ao vivo que Simon Property, um UPREIT, só reporta a segunda). Duas decisões perguntadas via `AskUserQuestion`: (1) escopo — painel de indicadores imobiliários, **sem** os 8 modelos de valuation/DCF (não encaixam em imobiliário, mesmo espírito de FII sem LPA/VPA/ROE); (2) FFO/AFFO/ocupação — campo manual editável, mesmo espírito do landbank do RNAV.

Código: `sources/sec_edgar.py::fetch_reit_fundamentals` (reaproveita `_try_tags`/`_required`/`_optional` já existentes, zero helper novo). Nova migration `m20260803_140000_create_reit_tables` — `reit_fundamentals` (time series via SEC) + `reit_manual_indicators` (FFO/AFFO/ocupação, upsert por ticker) — tabela própria, não `stock_fundamentals`/`stock_dcf_fundamentals`. Novo `commands/reit.rs` (find-then-upsert, mesmo padrão de `generate_company_ai_info`). `"reit"` entrou em `ASSET_CLASSES`/`ASSET_CLASSES_WITH_AUTO_QUOTE` e no roteamento do `run_stock_collector` (`--reit-ticker`, novo flag CLI espelhando `--us-ticker`). `company_ai_info.rs::framing_for_asset_class` ganhou `"reit"` no enquadramento de fundo (`fii`/`etf_br`). Frontend: novo `ReitLookupSection.tsx` (mistura o esqueleto de `FiiLookupSection.tsx` com `UsStockLookupSection.tsx`), seção "Real estate indicators" + "Manual indicators", `AboutCompanySection` posicionado **depois** dos indicadores (padrão novo da Sessão 61), sem `AssetThesesSidebar` (não pedido, FII também não tem).

Verificação: coletor testado ao vivo duas vezes (`O`: Revenue 5749,4/Equity 39438,7/EPS 1,17/Net income 1058,6 US$mi FY2025; `SPG`: confirmando o fallback `ProfitLoss` funcionando de verdade), conferido no SQLite. `cargo check`/`cargo test --lib` **102/102** e `npx tsc --noEmit` limpos. Rust recompilou e frontend fez HMR sem erro via `tauri dev` já rodando. **Não testado clicando na GUI** (sem ferramenta de automação de UI no projeto) — fica pro dono do projeto confirmar o fluxo completo.

**Marco**: Fase 10, item 8 ganha REIT como oitava classe expandida — primeira a documentar um gap de fonte de dado (FFO/AFFO/ocupação) resolvido com campo manual em vez de forçar automação inexistente. Restam Imóvel e Empresa não listada da lista original da Sessão 30.

**Continuação do item 8, Sessão 63 (2026-08-04) — ETF americano adicionado.** Dono do projeto pediu explicitamente uma frente "mais tranquila" depois de confirmar o REIT funcionando no app real — lembrou que faltava ETF listado nos EUA (SPY/VOO/QQQ). Planejado via `/plan`: confirmado ao vivo (`curl` direto no Yahoo) que `SPY` sem sufixo `.SA` devolve o mesmo shape de `meta` que ação americana (`instrumentType: "ETF"` em vez de `"EQUITY"`, resto idêntico) e que o endpoint de dividendos funciona igual; leitura completa de `acoes_yahoo.py` confirmou que nenhuma das 5 funções lê `instrumentType` — zero mudança necessária nesse módulo. Nome da classe decidido com o usuário via `AskUserQuestion`: `etf_us` (paralelo a `bdr`/`reit`/`metal`, não `etf_internacional`).

Código: nova `main_etf_us(ticker)` em `data-collector/main.py` — só o bloco Yahoo (`collect_us_stock_quotes`/`dividends_avg`/`technicals`/`dividend_payments`/`collect_us_price_history`, todos com `suffix=""`), **sem nenhum fundamento** — mais simples que REIT, mesmo espírito de `etf_br` (fundo não tem demonstração financeira própria). Novo flag `--etf-us-ticker`. `"etf_us"` entrou em `ASSET_CLASSES` (`commands/asset.rs`, 10→11) e `ASSET_CLASSES_WITH_AUTO_QUOTE` (`commands/profitability.rs`, 8→9, TWR cobre de graça) + roteamento no `run_stock_collector` (`collector.rs`). `company_ai_info.rs::framing_for_asset_class` ganhou `"etf_us"` no enquadramento de fundo (`fii`/`etf_br`/`reit`). Frontend: `portfolio/types.ts` (`AssetClass`/`ASSET_CLASSES`/`ASSET_CLASS_LABELS`/`ASSET_CLASSES_WITH_AUTO_QUOTE`), novo `EtfUsLookupSection.tsx` (esqueleto de `EtfLookupSection.tsx` — sem fundamentos/DCF/valuation/teses — com os ajustes americanos de `UsStockLookupSection.tsx`: `formatUsd` local, `asset_class: "etf_us"` explícito no coletor, exposição default `pais`/`US`), branch de dispatch + placeholder `SPY` em `StockLookupPanel.tsx`/`AssetSection.tsx` (novo booleano `isEtfUs`, plugado nos mesmos 4 lugares que `isUsStock`/`isReit` já usam). `ProfitabilitySection.tsx::YAHOO_BACKFILL_CLASSES` deliberadamente **não** ganhou `etf_us` — mesma exclusão de `acao_internacional`/`reit` (preço histórico já vem de graça a cada busca em Research; o botão de backfill manual só sabe `.SA`). Aproveitado pra corrigir um texto desatualizado na mesma seção: a frase "Covers Stock (B3), FII, ETF, Crypto, BDR and Metal" já estava incorreta havia duas sessões (REIT/International stock tinham entrado em `ASSET_CLASSES_WITH_AUTO_QUOTE` sem atualizar essa prose) — corrigida pra listar a cobertura real.

Verificação: coletor testado ao vivo duas vezes contra `SPY` dentro do container (`docker compose run --rm desktop /data-collector/.venv/bin/python3 /data-collector/main.py --etf-us-ticker SPY`) — cotação (US$771,33), técnicos (SMA200 US$701,39/CAGR10y 13,5%), 40 pagamentos de dividendo, 2512 pontos de histórico, tudo conferido direto no SQLite; segunda rodada confirmou idempotência (0 novos). `cargo check`/`cargo test --lib` **102/102** e `npx tsc --noEmit` limpos.

**Bug real crítico achado testando no app de verdade (a primeira vez que `run_stock_collector` foi exercitado, na GUI, com um ticker nunca antes cacheado, pra qualquer classe não-B3)**: dono do projeto reportou "No data found" pra `IVV`/`VOO`/`VZ` mesmo com a classe certa selecionada. Coletor Python testado direto via CLI pros 3 tickers funcionou perfeitamente — apontando pra um bug na camada de invocação, não no coletor. Confirmado lendo o código-fonte de `tauri-macros` dentro do container (`ArgumentCase::Camel` é o default de `#[tauri::command]`, sem `rename_all` declarado em `run_stock_collector`): **o Tauri converte automaticamente o nome de parâmetro de comando de nível superior pra camelCase no lado JS** — mesmo bug já achado e corrigido na Sessão 61 (`get_company_ai_info`), agora reincidindo em `run_stock_collector`, só que **em todo o projeto**: toda chamada usava `asset_class` (snake_case) em vez de `assetClass`, então o parâmetro sempre bindava `None`, silenciosamente caindo no `--ticker`/`.SA` genérico errado. Nunca tinha aparecido porque todo teste "confirmado no app real" anterior (Cripto/Metal/Ação americana/REIT) usou um ticker já pré-cacheado via CLI na mesma sessão de verificação — a busca automática nunca chegava a disparar de verdade. Corrigido em **todos** os 11 pontos de chamada de `run_stock_collector` (não só o ETF novo): `useTickerCollector.ts` (usado pelos 8 forms de valuation via "Fetch"), `CryptoLookupSection.tsx`, `MetalLookupSection.tsx`, `UsStockLookupSection.tsx`, `ReitLookupSection.tsx`, `EtfUsLookupSection.tsx`, `AssetSection.tsx`, e os 4 que já passavam `null` por consistência (`FiiLookupSection.tsx`/`BdrLookupSection.tsx`/`EtfLookupSection.tsx`/`StockAnalysisSection.tsx` — inofensivos hoje, já que `null` cai no mesmo lugar de qualquer forma, mas a mesma armadilha de nome errado). Confirmado que `create_asset`/`generate_company_ai_info` (chamados com um `request: {...}` objeto único) não têm esse problema — só nomes de parâmetro de nível superior sofrem a conversão, campos dentro de um struct usam o serde normal. `npx tsc --noEmit` limpo depois da correção; Vite HMR confirmado pegando os 11 arquivos. **Confirmado ao vivo pelo dono do projeto** com um ticker genuinamente novo — "funcionou".

**Nota pra próxima sessão**: vale uma auditoria dedicada nos demais comandos Tauri do projeto procurando o mesmo padrão (parâmetro de nível superior multi-palavra chamado em snake_case) — não feita agora por escopo, mas o precedente (2 ocorrências reais já achadas em sessões diferentes) sugere que pode haver mais.

**Marco**: Fase 10, item 8 ganha ETF americano como nona classe expandida — a mais simples até agora, puro reaproveitamento da infra Yahoo sem `.SA` já criada pra Ação americana/REIT, zero fundamento. Restam Imóvel e Empresa não listada da lista original da Sessão 30.

**Continuação do item 8, Sessão 64 (2026-08-08) — Imóvel adicionado, escopo completo do rascunho.** Dono do projeto escolheu Imóvel entre as opções levantadas e, perguntado via `AskUserQuestion`, pediu o escopo cheio do rascunho original desta seção (endereço, valor de compra, histórico de avaliações, anexos) em vez de só cadastro básico. **Decisão de arquitetura**: em vez do "anexo polimórfico único" (`entidade_tipo`/`entidade_id`) sugerido no rascunho, `asset_attachments` nasceu como tabela dedicada, mesmo molde de `thesis_attachments` (Fase 10.5) — reafirma o precedente de tabela por dono em vez de polimorfismo. Sem coluna `address` nova: o campo "Name" já genérico do cadastro manual serve de rótulo/endereço, evitando migration extra pra um texto que não entra em cálculo nenhum; "valor de compra fixo, histórico" continua vindo de `Transactions` (tipo `compra`), mecanismo que toda classe manual já usa.

**Schema**: `m20260808_100000_create_asset_valuations_and_attachments` — `asset_valuations` (`asset_id` FK **Cascade** → `assets`, `valuation_date`/`value`/`origin` — textual, só grava `"manual"` por ora, reajuste automático por % segue não decidido — /`notes`) e `asset_attachments` (`asset_id` FK **Cascade** → `assets`, campos de `thesis_attachments` + `document_type` livre). Cascade aqui é deliberado, diferente do padrão sem `on_delete` de `transactions`/`theses`/`watchlist_items` pra `assets` — histórico/anexo de um imóvel só faz sentido preso àquele imóvel. `commands/property.rs` (7 comandos, cópia do molde de `commands/thesis.rs`, `app_data_dir()/asset_attachments/{asset_id}/`). `"imovel"` entrou em `ASSET_CLASSES` (11→12), fora de `ASSET_CLASSES_WITH_AUTO_QUOTE` — cadastro manual, mesmo caminho de `tesouro_direto`/`renda_fixa`. `tauri.conf.json::security.assetProtocol.scope` ganhou `$APPDATA/asset_attachments/**`.

**Frontend**: novo `PropertyDetails.tsx`. Nasceu como linha expandida em `AssetSection.tsx` (mesmo padrão do "CVM data" do FII), mas **testando ao vivo o dono do projeto pediu popup** — trocado pra `Dialog` (shadcn/radix, mesmo componente de `NewValuationDialog.tsx`), estado próprio `propertyDialogAssetId` (FII manteve linha expandida, não pedido). Histórico de avaliações (tabela + form) e anexos reaproveitam **exatamente** o mecanismo de preview em disco de `ThesisSection.tsx` (imagem/PDF via `convertFileSrc`, planilha via SheetJS), só trocando `thesis_id` por `asset_id`. Todos os comandos novos com parâmetro de nível superior invocados em camelCase (`assetId`/`valuationId`/`attachmentId`) — conferido de propósito contra o bug achado na Sessão 63.

**Verificação**: `cargo check`/`cargo test --lib` **105/105** (2 novos) e `npx tsc --noEmit` limpos. Migration aplicada e entities regeneradas contra o banco real de dev, diff conferido. **Testado no app real, mesma sessão** — `docker compose up`, cadastro do imóvel e do popup de detalhes confirmados funcionando ("deu boa"). **Feedback explícito pra depois**: revisar a UX desta área (avaliações + anexos) numa sessão futura — sem detalhe do que mudaria ainda, mesmo espírito da pendência de UX que Teses (Fase 10.5) já tem em aberto desde a Sessão 37.

**Marco**: Fase 10, item 8 ganha Imóvel como décima classe expandida — primeira totalmente manual a ganhar histórico próprio (avaliações + anexos), fechando `thesis_attachments` como o molde definitivo de "anexo em disco" do projeto e descartando a ideia de anexo polimórfico único do rascunho da Sessão 30. Falta só Empresa não listada pra fechar a lista original da Sessão 30.

**Continuação, mesma Sessão 64 — Empresa não listada fecha o item 8.** O rascunho original tinha uma decisão em aberto (percentual/quantidade de ações/valuation total são matematicamente relacionados, mas só fecham se o total de ações for conhecido). Perguntado ao dono do projeto via `AskUserQuestion`: escolheu o escopo completo (campos de participação societária) com **percentual sempre calculado** (`equity_shares_owned / equity_total_shares`, nunca digitado — nenhuma coluna própria, calculado sempre no frontend). Migration `m20260808_150000_add_equity_fields_to_assets` — 3 `alter_table` separados (limite do SQLite) adicionando `equity_shares_owned`/`equity_total_shares`/`equity_company_valuation` nullable em `assets`, mesmo padrão do `cnpj` do FII. `empresa_nao_listada` entrou em `ASSET_CLASSES` (12→13) reaproveitando **sem nenhuma mudança** `asset_valuations`/`asset_attachments` — confirma a "Generalização AtivoManual" do rascunho: Imóvel e Empresa não listada realmente compartilham o mesmo esqueleto. Novo comando `update_asset_equity` (mesmo padrão estreito de `update_asset_cnpj`, já que participação muda com rodada de investimento/diluição). `PropertyDetails.tsx` renomeado pra `ManualAssetDetails.tsx` (agora serve as duas classes), ganhou um bloco "Equity stake" no topo (só pra `empresa_nao_listada`) com os 3 campos editáveis + percentual/valor calculados ao vivo.

**Dois popups a mais, pedidos ao vivo testando**: o popup de detalhes (que nasceu só pra Imóvel na primeira parte da sessão) foi generalizado pra cobrir as duas classes manuais ("Property details"/"Company details", mesmo `Dialog`). Depois, testando Empresa não listada funcionando, o dono do projeto pediu que o form de "Add asset" inteiro (classe + busca de ticker + campos manuais, até então inline no card) também virasse popup — migrado pra um `Dialog` próprio disparado por um botão no topo do card, sem mudar nenhuma mutation/estado, só onde o JSX mora.

**Achado de ambiente**: depois das mudanças de backend, o watcher automático do `tauri dev` não pegou sozinho as edições feitas pelo host — resolvido com `docker compose restart desktop`, que reaproveitou o binário já compilado por um `cargo check`/`cargo test` anterior (mesmo volume `cargo-target` compartilhado entre os containers do `docker compose run` e o `desktop-1` de `docker compose up`), religando em under 1s. Vale lembrar esse atalho (rodar `cargo check` isolado antes de restart) se o watcher falhar de novo.

**Verificação**: `cargo check`/`cargo test --lib` **105/105** e `npx tsc --noEmit` limpos a cada etapa. Migration aplicada e entity regenerada contra o banco real de dev. **Testado no app real, mesma sessão** — Empresa não listada, popup de detalhes generalizado e popup de Add asset todos confirmados funcionando pelo dono do projeto.

**Marco**: Fase 10, item 8 **fecha por completo** a lista original da Sessão 30 — 10 classes expandidas de listagem em bolsa/cotação automática ou fonte manual/histórico (Ação BR, FII, ETF BR, Cripto, BDR, Metal, Ação americana, REIT, ETF US, Imóvel) mais Empresa não listada. Restam só as pendências de UX já registradas (Teses desde a Sessão 37, Property/Company details desde mais cedo nesta sessão) e o item 9 (Custódias) pra fechar o restante da Fase 10.

- [x] **8.1 — BDR: "tipo de ativo subjacente".** Pendência registrada na Sessão 53 (ver acima), **implementada na Sessão 86 (2026-08-23)**. Escolha via `AskUserQuestion`: campo reaproveita valores de `AssetClass` (não enum próprio, não texto livre), restrito a um subconjunto de 5 que faz sentido como subjacente de BDR (`acao_internacional`/`etf_us`/`reit`/`metal`/`cripto`, `commands::asset::BDR_UNDERLYING_CLASSES`); opcional/nullable, BDRs cadastrados antes ficam `null`, editáveis depois; nesta fatia é só metadado exibido — Alocação/gráficos/pricing continuam sem considerar o subjacente. Coluna nova `underlying_asset_class` em `assets` (migration `m20260823_100000_add_underlying_asset_class_to_assets`), sem guard exigindo `asset_class == "bdr"` (mesmo espírito solto do `cnpj`/`equity_*`). Novo comando `update_asset_underlying_class` (mesmo molde de `update_asset_cnpj`) pra editar depois de cadastrado — na UI vira um Select inline direto na tabela de `AssetSection.tsx` (sem popup, é 1 campo só). Testado ao vivo e confirmado pelo dono do projeto.

---

### Fase 11 — Rebranding (Anchor), App Mobile & Empacotamento Desktop (ideia levantada na Sessão 30; 11.1 concluído na Sessão 65, 11.3 concluído na Sessão 67, 11.2 iniciado na Sessão 68, portfolio local na Sessão 69, mais 3 classes de ativo na Sessão 70, anexos de ativos manuais na Sessão 71)

**Objetivo**: renomear o app, ganhar um cliente mobile completo e empacotar o desktop como instalador standalone (sem exigir Python instalado na máquina do usuário) — três decisões trazidas juntas num segundo rascunho à parte (`practice-valuation-mobile-custodia-nome.md`, conteúdo transcrito aqui e arquivo removido; a 4ª ideia do mesmo rascunho, Gestão de Custódias, foi pra Fase 10 item 9 por pertencer ao modelo de carteira, não a essa fase).

**1. Nome do app → Anchor**: critérios usados na escolha — em inglês, remetendo a segurança/controle do próprio patrimônio (mesma linha conceitual do TruthID — self-custody, sem intermediário), curto e fácil de pronunciar em qualquer idioma. Como é software open-source, não há preocupação de conflito de marca/domínio. **Decisão de nome registrada — execução do rename (repo, pastas, README, referências internas) ainda não feita.**

**2. App mobile**: completo, replicando todas as funções que o desktop tem e terá. Framework escolhido: **Flutter** — base de código separada do desktop, resolve de saída o problema de o sidecar Python não rodar em mobile. (Atualiza o item "Companion mobile" do Roadmap, que antes era só uma ideia solta sem framework decidido.)

**3. Empacotamento do desktop (Tauri + Python)**: sidecar Python compilado pra binário standalone via **PyInstaller** ou **Nuitka**, sem exigir Python instalado na máquina do usuário. Binário registrado em `tauri.conf.json` (`bundle.externalBin`), empacotado junto no instalador final do Tauri (NSIS/MSI, `.dmg`, `.deb`/AppImage). CI: **GitHub Actions**, com matrix build (`windows-latest`, `macos-latest`, `ubuntu-latest`) compilando o sidecar nativamente em cada runner antes do step do `tauri-action`, que gera os instaladores e publica na release. Atenção: nome do binário do sidecar precisa incluir o target triple da plataforma (ex.: `sidecar-x86_64-pc-windows-msvc.exe`) pro Tauri localizar corretamente.

**Etapas**:
- [x] 11.1 — Executar o rename pra Anchor. **Implementado na Sessão 65 (2026-08-08)**: não foi um find-and-replace simples — mapeado antes de codar (agente `Explore`) e confirmado ao vivo que 3 pontos tinham risco operacional real, todos resolvidos com segurança: (1) `app-data` (volume Docker, chaveado pelo `identifier` do Tauri) estava **vazio de dado real** (só cache WebKit, ~37MB) — trocar `identifier`/nome do projeto Docker foi seguro, volumes antigos (`practice-valuation_*`) ficaram órfãos mas intactos, não apagados; (2) chaves de API no keyring do SO — escrita uma migração automática análoga à `migrate_legacy_key_if_needed` já existente (Fase 7.9.2), roda uma vez no próximo `list_api_keys`, sem o usuário precisar recolar nenhuma chave; (3) o banco SQLite real (`practice_valuation.db`, 5.3MB, dado de portfólio de verdade) foi renomeado fisicamente pra `anchor.db` com o app parado (`docker compose down`), backup feito antes (`practice_valuation.db.bak`, mantido), e dado confirmado intacto ao vivo depois do restart. Repositório no GitHub renomeado também (`gh repo rename anchor`, remote `origin` local atualizado sozinho). **Achado de ambiente**: o volume Docker novo (`anchor_cargo-registry`) nasceu com dono `root` (mesmo padrão já visto na Sessão 37 com `app-data`) — corrigido com `chown -R 1000:1000` nos 3 caminhos (`/root/.cargo`, `/app/src-tauri/target`, `/home/node/.local/share`) antes do primeiro `cargo check` no volume novo funcionar. **Pasta local renomeada também, mesma sessão**: dono do projeto perguntou por que eu tinha deixado esse item de fora — confirmou não ter editor/terminal apontando pro caminho antigo, então renomeei (`mv .../practice-valuation .../anchor`) com o app parado (`docker compose down` antes). O bind mount `.` do Compose é relativo, então não precisou de nenhuma mudança de config; o Linux segue o inode, não o caminho, então o próprio shell desta sessão se ajustou sozinho. `docker compose up` confirmado subindo limpo do caminho novo. **Deixado explicitamente pro dono do projeto, não executado por mim**: renomear o remote `gitea` (sem `tea` CLI disponível aqui, precisa do painel web). `cargo check`/`cargo test --lib` **105/105** e `npx tsc --noEmit` limpos em cada etapa (Tier 1 cosmético, Tier 2 identidade de pacote/crate, Tier 3 identifier+DB+keyring). Testado no app real: janela abre com título "Anchor", processo roda como binário `anchor`, dados de Portfolio/Ativos confirmados intactos pelo dono do projeto.
- [ ] 11.2 — App mobile em Flutter, replicando as funções do desktop. **Iniciado na Sessão 68 (2026-08-08), não fechado**: tinha uma pergunta de arquitetura em aberto desde a Sessão 67 (sem sidecar Python, que não roda em mobile, e sem servidor — decisão já rejeitada no Roadmap — como o mobile acessaria carteira/dados de mercado?). Levantamento com 2 agentes `Explore`: das 10 fontes do `data-collector`, 5 são APIs JSON públicas sem chave chamáveis direto do mobile (Yahoo ações/metais, CoinGecko, DefiLlama, Fear&Greed, ultrasound.money); as outras 5 (bolsai, CVM DFP/FII, SEC EDGAR) exigem chave secreta ou processamento pesado, inviáveis direto do celular. ~1.300 linhas de lógica de valuation (`desktop/src-tauri/src/domain/`) hoje só existem em Rust, sem equivalente Dart. A Fase 8 (TruthID/IPFS) seria o sync natural mas está incompleta/bloqueada (8.3+ travada desde a Sessão 33) — só provada desktop-para-desktop, embora o TruthID já tenha um app Flutter mobile real usando esse padrão (precedente). Perguntado ao dono do projeto via `AskUserQuestion`: escolheu **"App standalone agora, sync depois"** — mobile nasce com armazenamento local próprio, busca direto as 5 fontes fáceis, funcionalidades dependentes de bolsai/CVM/SEC ficam de fora por ora; sync de verdade com o desktop (Fase 8 ou outro mecanismo) vira decisão de fase futura, não bloqueia o início do mobile. Escopo da sessão (também via `AskUserQuestion`, dado que é projeto novo do zero): **scaffold + 1 fatia vertical**, não o app completo. **O que foi entregue**: novo diretório `mobile/` (Flutter puro), toolchain Docker reaproveitado do TruthID mobile (`~/Documents/workspace/truthid/mobile`, mesmo `Dockerfile`/`docker-compose.yml` com Android SDK + volumes nomeados pra cache/keystore de debug persistente), projeto criado via `flutter create` (`com.masterlxz.anchor_mobile`), tema dark alinhado ao desktop (`#4ade80` sobre `#0b0f14`), e uma tela real ponta a ponta — busca de ticker B3 + cotação ao vivo via `lib/services/yahoo_quote_service.dart`, mesmo endpoint não-oficial do Yahoo já confirmado contra a API real em `data-collector/sources/acoes_yahoo.py::fetch_quotes`. `AndroidManifest.xml` ganhou `INTERNET` permission (não vem por padrão do `flutter create`). Sem gerenciamento de estado global — uma tela só não justificava. **Verificado de verdade**: `docker compose build`/`flutter analyze` limpos, `flutter build apk --debug` gerou um `.apk` real (146MB) — prova o toolchain completo (Android SDK/NDK/Gradle) funcionando dentro do Docker. Teste funcional ao vivo (abrir o app, buscar um ticker) fica pro dono do projeto — nenhum device/emulador Android disponível neste ambiente. **Continuação, Sessão 69 (2026-08-08) — portfolio local**: dono do projeto pediu pra continuar 11.2 com portfolio local de verdade (cadastrar ativos, lançar compra/venda, ver posição com valor/P&L ao vivo). Levantamento com 1 agente `Explore` do schema exato do desktop (`assets`/`transactions`/`get_portfolio_positions`) confirmou que o algoritmo de posição **não é FIFO**: quantidade líquida = compra − venda; preço médio = média ponderada só das compras (vendas não reduzem a base de custo, simplificação deliberada já documentada no próprio desktop) — só `compra`/`venda` afetam posição, `aporte`/`retirada`/`provento`/`transferencia` são fluxo de caixa/custódia, nenhum dos dois conceitos existe no mobile ainda. Duas decisões via `AskUserQuestion`: storage **`sqflite` + SQL puro** (não `drift`, sem trazer codegen a um projeto que não tem nenhum) e escopo de classe **só B3 (`.SA`)** — Ação BR/FII/ETF BR/BDR, as 4 que reaproveitam 100% o `YahooQuoteService` já existente (confirmado nas Sessões 41/63 do desktop que essas classes compartilham o mesmo formato de `meta`). **Entregue**: schema simplificado (`mobile/lib/data/db.dart`, 2 tabelas — `assets` e `portfolio_transactions`, bem mais enxutas que as do desktop, sem custódia/exposição/renda-fixa/campos manuais), modelos (`AssetClass`/`Asset`/`TransactionType`/`PortfolioTransaction`/`Position`), `portfolio_repository.dart` (CRUD + `computePositions()` — função pura replicando fielmente o algoritmo do desktop), 3 telas novas (`portfolio_screen.dart` com preço/valor/P&L ao vivo, `add_asset_screen.dart`, `add_transaction_screen.dart`) e `NavigationBar` de 2 abas em `main.dart`. **Achado de teste**: `flutter test` roda na VM Dart pura, sem os plugins de plataforma do `sqflite` normal — `widget_test.dart` (que agora pumpa `PortfolioScreen` dentro do `IndexedStack`) quebrava com `databaseFactory not initialized`; resolvido com `sqflite_common_ffi` + `test/flutter_test_config.dart` (SQLite real via FFI, sem mockar nada) — pegadinha de import (`package:sqflite_common_ffi/sqflite_ffi.dart`, não `sqflite_common_ffi.dart`, nome do pacote ≠ nome do arquivo) pega pelo próprio `flutter analyze`. **Verificado de verdade**: `flutter analyze` limpo, `flutter test` 5/5 (4 novos cobrindo o cálculo de posição — sem transação, compra única, venda parcial não afeta preço médio, média ponderada de duas compras), `flutter build apk --debug` gerou `.apk` real (185MB). Teste funcional ao vivo fica pro dono do projeto (sem device/emulador Android neste ambiente). **Fora de escopo, ainda não feito**: mais classes de ativo (EUA sem sufixo `.SA`, Cripto, Metal), custódia, tipos de transação além de compra/venda, sync com o desktop, e a tela inicial/dashboard personalizável (Fase 10, item 10, registrada na mesma Sessão 68/69). **Continuação, Sessão 70 (2026-08-08) — 3 classes de ativo novas**: dono do projeto escolheu, entre as opções restantes de 11.2, ampliar as classes suportadas — especificamente Cripto (CoinGecko), Metal e Ação internacional (as 3 que já têm fonte gratuita confirmada no desktop, ver Fase 2.2/2.3 e `metais_yahoo.py`/`cripto_coingecko.py`), deixando `reit`/`etf_us`/renda fixa/imóvel de fora. Planejado via `/plan`. **Entregue**: `AssetClass` ganhou `acao_internacional`/`cripto`/`metal` (mesmos valores string do desktop) mais uma extensão `AssetClassMeta` (`tickerHint`/`defaultCurrency` por classe, evita duplicar switch em cada tela); `Asset` ganhou `external_id` (guarda o `coin_id` do CoinGecko resolvido no cadastro, evita rebuscar por texto a cada refresh — migração de schema v1→v2 com `onUpgrade`); `YahooQuoteService.fetchQuote` ganhou parâmetro `suffix` (default `.SA`, ação internacional chama com `''`) e `fetchMetalQuote` (mapeia `XAU`/`XAG`/`XPT`/`XPD` pros contratos futuros `GC=F`/`SI=F`/`PL=F`/`PA=F`, mesmo mapa do `metais_yahoo.py`); novo `CoinGeckoQuoteService` (resolve símbolo→`coin_id` via `/search`, mesma regra de match exato + menor `market_cap_rank` do Python, preço via `/simple/price` — mais leve que o `market_chart` usado no desktop, já que o mobile não precisa de histórico aqui); novo `QuoteDispatcher` centralizando o roteamento por classe (usado pelas 3 telas — busca livre, cadastro, posição ao vivo — em vez de cada uma decidir a fonte por conta própria). `add_asset_screen.dart`/`quote_search_screen.dart` ganharam hint/label/erro dinâmicos por classe (`AssetClassMeta`); `quote_search_screen.dart` ganhou o dropdown de classe que só existia no cadastro. **Verificado de verdade**: `flutter analyze` limpo, `flutter test` 5/5 (`widget_test.dart` ajustado — o label do campo ticker mudou de fixo "Ticker (B3)" pra dinâmico "Ticker (Ação BR)" default), `flutter build apk --debug` gerou `.apk` real (185MB). Nenhuma fonte nova validada nesta sessão — as 3 já são usadas de verdade no desktop desde as Sessões 5/21/51/55/57. Teste funcional ao vivo (cadastrar um ticker de cada classe nova, ex. `AAPL`/`ETH`/`XAU`, e ver a posição com preço ao vivo) fica pro dono do projeto, mesma limitação de ambiente das sessões anteriores (sem device/emulador Android aqui). **Fora de escopo, ainda não feito**: `reit`/`etf_us`/renda fixa/imóvel, custódia, tipos de transação além de compra/venda, sync com o desktop, dashboard personalizável. **Mesma Sessão 70, continuação — mais tipos de transação**: dono do projeto escolheu ampliar os tipos suportados (`aporte`/`retirada`/`provento`), deixando `transferencia` de fora por depender de custódia (Fase 10 item 9, ainda não iniciada no mobile). Planejado via `/plan`, com uma decisão extra confirmada via `AskUserQuestion`: como o mobile não tinha nenhuma tela de histórico/lista de transações (só o formulário de lançar), e `aporte`/`retirada`/`provento` não mexem em posição nenhuma (ficariam invisíveis depois de salvos), a sessão também entregou uma tela de histórico simples, somente leitura. **Entregue**: `TransactionType` ganhou os 3 valores novos (mesmos strings do desktop) + extensão `TransactionTypeMeta` (`needsAsset`/`needsQuantity`/`needsUnitPrice`, espelhando `TransactionSection.tsx:65-79` do desktop sem a parte de `transferencia`) — `aporte`/`retirada` são fluxo de caixa puro (sem ativo), `provento` tem ativo mas não quantidade/preço unitário, só um valor total digitado direto. `PortfolioTransaction.assetId`/`quantity`/`unitPrice` passaram a nullable — migração de schema v2→v3 (`db.dart`) recriando a tabela `portfolio_transactions` (SQLite não altera constraint `NOT NULL` via `ALTER TABLE`). `computePositions` ganhou os 3 tipos novos como no-op (não alteram quantidade/preço médio, mesma regra do `get_portfolio_positions` do desktop). `add_transaction_screen.dart` reescrita: seletor de tipo virou dropdown (5 opções não cabiam num `SegmentedButton`), campos condicionais via `TransactionTypeMeta`, e a tela não trava mais inteira quando não há ativo cadastrado (só quando o tipo escolhido exige um). Nova `transaction_history_screen.dart` (lista somente leitura, data/tipo/ticker/quantidade/valor, `listAllTransactions` ganhou `orderBy` por data), acessível por um ícone novo no `AppBar` do Portfolio. **Verificado de verdade**: `flutter analyze` limpo, `flutter test` 7/7 (2 testes novos — aporte/retirada não aparecem em posição nenhuma, provento não altera quantidade/preço médio), `flutter build apk --debug` gerou `.apk` real (185MB). Teste funcional ao vivo fica pro dono do projeto, mesma limitação de ambiente. **Fora de escopo, ainda não feito**: `transferencia` (depende de custódia), edição/exclusão na tela de histórico (só leitura), saldo de caixa do portfolio a partir de aporte/retirada (o próprio desktop também não calcula isso ainda). **Mesma Sessão 70, continuação — Custódia + Transferência**: dono do projeto escolheu fechar o conjunto completo de 6 tipos de transação do desktop, trazendo custódia (conta/corretora) e `transferencia` juntos — confirmado via `AskUserQuestion` (incluir `transferencia` já que a dependência estava sendo resolvida, e mostrar o detalhamento de posição por custódia — `by_custodia` — na tela de Portfolio; **sim** pras duas). Levantamento direto de `desktop/src-tauri/src/entity/custodia.rs` (`{id, workspace_id, instituicao, titular, created_at}`, CRUD só create/list/delete) e `commands/transaction.rs:341-373`/`159-162` (`get_portfolio_positions`: `compra`/`venda` somam na custódia da transação, `transferencia` subtrai da origem e soma no destino, `aporte`/`retirada`/`provento` não entram nesse cálculo; rótulo "—" quando `custodia_id` é nulo). **Entregue**: `Custodia` novo (sem `workspace_id` — o mobile não tem Workspace) com CRUD simples (`custodias_screen.dart`, criar/listar/excluir, sem edição); `TransactionType` ganha `transferencia` (6º e último tipo do desktop) — `TransactionTypeMeta` ganha `needsTransferDestination` e `needsQuantity` passa a incluir `transferencia` (que tem quantidade mas não preço unitário — primeira vez que os dois campos divergem no mobile, forçando `add_transaction_screen.dart` a tratar quantidade/preço/total como 3 condições independentes em vez de "tudo ou nada"); `PortfolioTransaction` ganha `custodiaId`/`transferToCustodiaId` (ambos opcionais, migração v3→v4 com `ALTER TABLE ADD COLUMN` simples — dessa vez sem precisar recriar tabela, já que as colunas novas nascem nullable). `computePositions` ganha `byCustodia` (nova classe `CustodiaBreakdown`, sem rótulo embutido — resolvido na tela a partir da lista de custódias já carregada, mantendo a função pura/sem I/O) — `transferencia` redistribui quantidade entre origem/destino sem alterar `netQuantity`/preço médio, mesma regra do desktop. `portfolio_screen.dart` mostra o detalhamento por custódia no card de posição (só quando há mais de uma entrada ou a única tem custódia definida, pra não poluir quem nunca usou); `transaction_history_screen.dart` ganha o rótulo de custódia por linha (e "origem → destino" pra transferências). Novo ícone "Custódias" no `AppBar` do Portfolio. **Verificado de verdade**: `flutter analyze` limpo, `flutter test` 9/9 (os 7 já existentes + 2 novos — compras em custódias diferentes aparecem separadas em `byCustodia`, `transferencia` move quantidade sem alterar posição), `flutter build apk --debug` gerou `.apk` real. Teste funcional ao vivo fica pro dono do projeto, mesma limitação de ambiente. **Fora de escopo, ainda não feito**: edição de custódia (só criar/listar/excluir, igual ao desktop), edição/exclusão de transação, saldo de caixa a partir de aporte/retirada, sync com o desktop, dashboard personalizável — com isso, os 6 tipos de transação e o conceito de custódia do desktop agora têm equivalente completo no mobile. **Mesma Sessão 70, continuação — REIT e ETF US**: das 6 classes de ativo que ainda faltavam, dono do projeto escolheu trazer só as 2 "de graça" — REIT e ETF US, que reaproveitam 100% a mesma fonte Yahoo sem `.SA` que `acao_internacional` já usa (`desktop/src/portfolio/types.ts:43-48`: REIT é o equivalente americano do FII, ETF US é ETF listado nos EUA tipo SPY; REIT também tem fundamentos imobiliários via SEC EDGAR no desktop, mas isso é só de Research, não afeta cotação/posição) — confirmado via `AskUserQuestion` contra a alternativa de fechar as 6 de uma vez, que incluiria Tesouro Direto/Renda Fixa (sem cotação automática, campos próprios de indexador/taxa/vencimento) e Imóvel/Empresa não listada (cadastro 100% manual), modelo de dados fundamentalmente diferente, deixado pra uma sessão à parte. **Entregue**: `AssetClass` ganhou `reit`/`etfUs` (mesmos valores string do desktop) + entradas em `AssetClassMeta` (`tickerHint` `O`/`SPY`, `defaultCurrency` USD); `QuoteDispatcher.fetchQuoteForTicker` ganhou os 2 casos roteando pro mesmo `_yahoo.fetchQuote(ticker, suffix: '')` de `acaoInternacional` (switch exaustivo do Dart forçou listar os casos, mesmo sendo idêntico ao existente). Nenhuma mudança em `db.dart`/telas — tudo já dirigido por `AssetClass.values`/`QuoteDispatcher` sem hardcode de classe específica, as 2 novas apareceram automaticamente nos dropdowns e no fluxo de posição/preço ao vivo. **Verificado de verdade**: `flutter analyze` limpo, `flutter test` 9/9 (nenhum teste novo — mudança é só roteamento, mesmo padrão de "cotação/rede verificada manualmente" já estabelecido), `flutter build apk --debug` gerou `.apk` real. Teste funcional ao vivo fica pro dono do projeto. **Fora de escopo, ainda não feito**: Tesouro Direto/Renda Fixa, Imóvel/Empresa não listada (as 4 classes com modelo de dados diferente), custódia editável, sync com o desktop, dashboard personalizável — mobile agora com 9 das 13 classes de ativo do desktop. **Mesma Sessão 70, continuação — Renda Fixa/Tesouro Direto + Imóvel/Empresa não listada**: dono do projeto pediu as 4 classes restantes juntas. Levantamento antes de planejar (sem agente, lendo direto `desktop/src-tauri/src/commands/{transaction,property}.rs`, `desktop/src/portfolio/{TransactionSection,ManualAssetDetails}.tsx`, `desktop/src-tauri/src/entity/assets.rs`) confirmou que nenhuma das 4 tem cotação automática (`ASSET_CLASSES_WITH_AUTO_QUOTE` já não as lista) e revelou um achado importante: a própria tabela "Consolidated positions" do desktop nunca mostra preço atual nem P&L pra nenhuma classe, só quantidade/preço médio de compra — o preço ao vivo/P&L que o mobile já tem é melhoria só-mobile (Sessão 69+). Isso definiu o desenho: **Tesouro Direto/Renda Fixa** usam o mecanismo normal de compra/venda (cabe 100% no `computePositions` já existente), só ganham 5 campos extras na transação de compra (emissor/indexador/taxa/vencimento/liquidez, `showFixedIncomeFields` do desktop) — sem cotação, posição mostra só quantidade/preço médio, igual ao desktop. **Imóvel** ganha histórico de avaliação manual (`asset_valuations`, `origin` sempre "manual") — como quantidade é sempre 1, a última avaliação vira "preço atual" no mesmo card de P&L já existente, sem inventar matemática (1×valor=valor). **Empresa não listada** ganha isso mais 3 campos na própria `Asset` (`equity_shares_owned`/`equity_total_shares`/`equity_company_valuation`, percentual/valor sempre calculados, nunca digitados) — como o valor de participação já é o total da posição (não um preço por unidade), fica fora da conta quantidade×preço (continua "—", igual ao desktop) e ganha uma linha própria "Participação: X% — valor". **Anexos (escritura/ITBI/cap table) ficaram de fora** — exigem file-picker + storage em disco, que o mobile não tem, e não foram pedidos. **Entregue**: migração de schema v4→v5 (3 colunas equity em `assets`, nova tabela `asset_valuations`, 5 colunas `fi_*` em `portfolio_transactions`, tudo `ALTER TABLE ADD COLUMN` simples); `AssetClass` ganha os 4 valores + `AssetClassMeta.hasAutoQuote`/`isFixedIncome`/`isManualAsset`; `Asset` ganha os 3 campos de equity + getters `equityPercentual`/`equityParticipationValue`; novo `AssetValuation` model; `PortfolioTransaction` ganha os 5 campos `fi_*` + constantes `fiIndexadores`/`fiLiquidezOptions`; `portfolio_repository.dart` ganha CRUD de equity/avaliação (`updateAssetEquity`, `insertAssetValuation` fixando `origin: 'manual'` internamente, `listAssetValuations`, `getLatestValuation`, `deleteAssetValuation`); `QuoteDispatcher` lança `QuoteNotFoundException` pras 4 classes novas (sem fonte, tratado graciosamente pelos call-sites já existentes); `add_asset_screen.dart` esconde o botão "Buscar" quando `!hasAutoQuote` e mostra os 3 campos de equity pra `empresaNaoListada`; `add_transaction_screen.dart` mostra o bloco `fi_*` numa compra de ativo de renda fixa; nova `manual_asset_details_screen.dart` (mirror de `ManualAssetDetails.tsx` sem anexos — equity stake + histórico de avaliação criar/listar/excluir); `portfolio_screen.dart` busca `getLatestValuation` pra `imovel` em vez do dispatcher, card vira tocável (`InkWell`) pras 2 classes manuais abrindo a nova tela, e `empresaNaoListada` ganha a linha de participação; `quote_search_screen.dart` filtra o dropdown só pras classes com `hasAutoQuote` (busca de cotação não faz sentido pras 4 novas). **Verificado de verdade**: `flutter analyze` limpo, `dart format` rodado (código formatado automaticamente), `flutter test` **12/12** (9 já existentes + 3 novos cobrindo `Asset.equityPercentual`/`equityParticipationValue` — cálculo correto, `null` sem os 3 campos, `null` com total de cotas zero), `flutter build apk --debug` gerou `.apk` real. Teste funcional ao vivo (Tesouro Direto com campos de renda fixa, Imóvel com avaliações mostrando "preço atual" seguindo a última, Empresa não listada com participação calculada) fica pro dono do projeto, mesma limitação de ambiente. **Marco**: mobile fecha as 13 classes de ativo do desktop por completo. **Fora de escopo, ainda não feito**: anexos, edição/exclusão de transação, custódia editável, sync com o desktop, dashboard personalizável. **Mesma Sessão 70, continuação — Custódia editável**: dono do projeto pediu pra fechar esse pendente. Confirmado que o próprio desktop também não tem edição de custódia (só create/list/delete) — melhoria só-mobile, mesmo padrão do preço ao vivo/P&L. **Entregue**: `PortfolioRepository.updateCustodia`; `custodias_screen.dart` ganhou `_edit` (toque no card ou ícone de lápis abre `AlertDialog` com instituição/titular pré-preenchidos, salva e recarrega). Sem mudança de schema. **Verificado de verdade**: `flutter analyze` limpo, `flutter test` 12/12 (sem teste novo — CRUD/UI puro), `flutter build apk --debug` gerou `.apk` real. Teste funcional ao vivo fica pro dono do projeto. **Fora de escopo, ainda não feito**: anexos, edição/exclusão de transação, sync com o desktop, dashboard personalizável. **Mesma Sessão 70, continuação — Edição e exclusão de transação**: dono do projeto pediu pra fechar esse pendente também. Confirmado que o desktop também só tem `create_transaction`/`delete_transaction` (nem lá existe update). Decisões: editar reaproveita `add_transaction_screen.dart` inteiro (evita duplicar os campos condicionais por tipo já centralizados em `TransactionTypeMeta`), exclusão sem confirmação (mesmo padrão já usado em `custodias_screen.dart`/avaliação de imóvel). **Entregue**: `PortfolioRepository.updateTransaction`; `AddTransactionScreen` ganhou parâmetro opcional `transaction` (pré-popula todo o formulário via `_findById` genérico contra ativos/custódias já carregados, título vira "Editar transação", `_save` preserva id/createdAt/notes e chama `updateTransaction`); `transaction_history_screen.dart` ganhou `_edit`/`_delete` e ícones de lápis/lixeira no `_TransactionTile`; `portfolio_screen.dart::_openHistory` passou a recarregar a posição ao voltar. **Verificado de verdade**: `flutter analyze` limpo, `flutter test` 12/12 (sem teste novo — CRUD simples), `flutter build apk --debug` gerou `.apk` real. Teste funcional ao vivo fica pro dono do projeto. **Marco**: fecha as pendências de CRUD do Portfolio no mobile (custódia editável + transação editável/excluível) — restam anexos de ativos manuais, sync com o desktop e dashboard personalizável. **Continuação, Sessão 71 (2026-08-09) — Anexos de ativos manuais**: das 3 frentes restantes (anexos, sync com o desktop, dashboard personalizável), dono do projeto escolheu anexos via `AskUserQuestion` — única sem decisão de arquitetura pendente. Precedente do TruthID mobile confirmou `file_picker`+`path_provider` já usados em produção; `open_filex` (novo, "abrir com app padrão do sistema") confirmado via pub.dev/GitHub que auto-registra seu próprio `FileProvider` no manifest merge, sem exigir editar `AndroidManifest.xml`/criar `filepaths.xml` na mão (só necessário "quando há conflito com outro plugin de FileProvider", não é o caso). Decisão de design: sem preview inline (imagem/PDF/planilha) como o desktop — abre com o app padrão do sistema via `open_filex`; exclusão sem confirmação, mesmo padrão já estabelecido (Sessão 70). **Entregue**: `pubspec.yaml` ganha as 3 deps; migração de schema v5→v6 — tabela `asset_attachments` (mirror de `asset_attachments::Model` do desktop); novo model `AssetAttachment`; `portfolio_repository.dart` ganha `listAssetAttachments`/`addAssetAttachment` (copia o arquivo escolhido pro diretório de documentos do app via `path_provider`, sob `asset_attachments/{assetId}/`, nome prefixado com timestamp — mobile não guarda referência ao caminho original)/`resolveAssetAttachmentPath`/`deleteAssetAttachment`; `manual_asset_details_screen.dart` ganha seção "Anexos" (lista com nome/tipo/tamanho, toque abre via `OpenFilex.open`, lixeira exclui, campo de tipo de documento opcional + botão que aciona `FilePicker.platform.pickFiles()`). **Verificado de verdade**: `docker compose build`/`flutter pub get` (resolveu `file_picker 10.3.10`/`path_provider 2.1.6`/`open_filex 4.7.0`, sem conflito), `flutter analyze` limpo, `flutter test` 12/12 (sem teste novo — CRUD/I/O de arquivo), `flutter build apk --debug` gerou `.apk` real (185MB, único aviso não-bloqueante sobre KGP do `file_picker`). Teste funcional ao vivo fica pro dono do projeto, mesma limitação de ambiente. **Fora de escopo, ainda não feito**: sync com o desktop, dashboard personalizável — restam essas 2 frentes de 11.2/Fase 10.
- [x] 11.3 — Empacotamento do sidecar Python (PyInstaller) + CI GitHub Actions (matrix build) gerando instaladores standalone. **Implementado na Sessão 67 (2026-08-08)**: escolha de PyInstaller (não Nuitka — script leve, sem dependência nativa pesada) e reaproveitamento direto do `.github/workflows/build.yml` do TruthID (outro projeto do mesmo dono, mesma stack Tauri+Rust+`keyring`), matrix `ubuntu-22.04`/`windows-latest`/`macos-latest` via `tauri-apps/tauri-action`, só acrescentando um passo de compilar o sidecar Python antes. **Achado que ampliou o escopo, antes de codar**: compilar o sidecar sozinho não bastava — `db.rs` conectava no banco com um caminho hardcoded de dev, *antes* até do `tauri::Builder` existir (sem `AppHandle` disponível pra resolver `app_data_dir()`). Perguntado ao dono do projeto via `AskUserQuestion` — escolheu resolver a unificação completa agora, não só provar o mecanismo com o caminho de dev. `db.rs::resolve_database_path(app)` (dev vs. release, mesmo mecanismo de `app_data_dir()` que `thesis.rs`/`property.rs` já usam) + `lib.rs` movendo a conexão de banco e `alert_checker::spawn_periodic_check` pra dentro de um `.setup()` novo (só ali existe `AppHandle`) fecham de vez a pendência "caminho de produção deferido pra Fase 6" que existia desde a Fase 1. `commands/collector.rs::run_collector` ganhou branch `cfg!(debug_assertions)` — dev continua no `.venv` de sempre, release usa `tauri-plugin-shell`'s `sidecar("anchor-collector")` passando `--db-path` explícito (novo argumento em `main.py`, parseado antes do resto do dispatch, sem quebrar o uso via `.venv`). **Achado de ambiente**: declarar `externalBin` no `tauri.conf.json` já quebra `cargo check`/`tauri dev` até o binário do sidecar existir de verdade pro target-triple do host — não é só uma checagem do build final. **Testado de ponta a ponta, de verdade**: sidecar compilado (`pyinstaller --onefile`) e testado standalone contra a API real (PETR4) antes de integrar; `npm run tauri build` gerou `.deb`/`.rpm` reais (AppImage travou só por falta de `xdg-open` na imagem Docker mínima — ferramenta que o GitHub Actions já tem, não deve reproduzir lá); `.deb` **instalado de verdade via `dpkg -i`**, banco novo nascendo no `app_data_dir()` certo, migrado manualmente (app não migra sozinho no startup, achado antigo reconfirmado) e o app relançado — **confirmado ao vivo pelo dono do projeto**: janela abre, busca de cotação funciona pelo sidecar compilado. `cargo check`/`cargo test --lib` **105/105** e `npx tsc --noEmit` limpos; dev (`docker compose up`) também confirmado funcionando sem regressão depois do bootstrap reestruturado. **Fora de escopo, não resolvido**: distribuição de segredos/`.env` (BOLSAI_API_KEY, contato SEC) pro binário empacotado — funcionalidades que dependem deles ficam degradadas num instalador real distribuído, pendência separada registrada. **CI escrita mas não disparada** — nenhuma tag/push nesta sessão, fica pro dono do projeto rodar quando quiser testar Windows/macOS de verdade (só o caminho Linux foi verificável daqui).

---

### Fase 12 — Módulo de Finanças Gerais (Balanço Patrimonial + Fluxo de Caixa, ideia levantada na Sessão 65, núcleo + Category + Liability na Sessão 71, limite de orçamento mensal em Category na Sessão 75)

**Objetivo**: dono do projeto trouxe um rascunho à parte (`features-financas-anchor.md`, conteúdo transcrito aqui e arquivo removido) — brainstorm de arquitetura pra expandir o app de "portfolio tracker" pra "balanço patrimonial + fluxo de caixa + investimentos", cobrindo pessoa física e jurídica (PF/PJ).

**Núcleo — `BankAccount` + `GeneralTransaction` manual, Sessão 71 (2026-08-09)**: dono do projeto pediu pra começar a Fase 12. Duas decisões confirmadas via `AskUserQuestion` antes de planejar: (1) **arquitetura** — o `Transaction` de fluxo de caixa geral nasce como **entidade paralela** (`general_transactions`), não estende a `transactions` do Portfolio (Fase 10.2) — evita acoplar semânticas diferentes numa tabela que já alimenta `get_portfolio_positions`; (2) **escopo da primeira fatia** — só o núcleo (`BankAccount` + `GeneralTransaction` manual, receita/despesa, saldo derivado), sem `Liability`/dívida, `Category` com limite de orçamento, `Attachment`+Vault, IA de extração ou reconciliação (a ordem sugerida no rascunho abaixo). Plataforma: só desktop, mesmo padrão de toda fase anterior (mobile réplica depois). Planejado via `EnterPlanMode` depois de um agente `Explore` mapear os padrões exatos de `Custodia`/`Transaction` (Fase 10.2) a reaproveitar — migrations em `migration/src/`, entities SeaORM com `belongs_to`, comandos Tauri simples filtrados por `workspace_id` (sem verificação de posse/sessão — app single-user local, mesmo padrão observado em todo o resto do backend), e o idiom de `list_transactions`/`get_portfolio_positions` (agregação em memória via `HashMap`, não view SQL).

**Modelo de dados**: `bank_accounts` (`id`, `workspace_id` FK cascade, `nome` — apelido livre, `titular` — CPF/CNPJ texto livre, `created_at`; mesmo molde de `Custodia`, sem os campos de agência/conta detalhados do rascunho, não usados em cálculo nenhum). `general_transactions` (`id`, `workspace_id` FK cascade, `bank_account_id` FK cascade **obrigatório**, `transaction_type` — só `receita`/`despesa` nesta fatia, `categoria` texto livre opcional — sem a entidade `Category`/limite de orçamento ainda, `valor` sempre positivo — sinal implícito no tipo, `transaction_date`, `notes`, `created_at`). Saldo de conta é **derivado**, nunca gravado — soma de `valor` das `receita` menos `despesa` daquela conta, calculado em memória (`compute_balances`, função pura extraída e testada, mesmo padrão de `sanitize_file_name`/`guess_content_type` em `commands/property.rs`).

**Entregue**: migrations `m20260809_150000_create_bank_accounts`/`m20260809_150100_create_general_transactions`; entities `entity/bank_account.rs`/`entity/general_transaction.rs`; comandos `commands/bank_account.rs` (`list_bank_accounts` retorna `BankAccountView` com `balance` calculado, `create`/`update`/`delete_bank_account`) e `commands/general_transaction.rs` (`list_general_transactions` ordenado por data desc, `create`/`update`/`delete_general_transaction`, guarda `validate_type` rejeitando qualquer `transaction_type` fora de `receita`/`despesa`); frontend novo `desktop/src/finances/` (módulo irmão de `portfolio/`, não sub-aba dele — `BankAccountSection.tsx` com edição inline mesmo idiom de `WatchlistSection.tsx`, `GeneralTransactionSection.tsx` mirror simplificado de `TransactionSection.tsx`, `FinancesPanel.tsx` com sub-abas Accounts/Transactions); `App.tsx` ganha a aba top-level "General Finances", ao lado de Research/Portfolio/Alerts. **Verificado de verdade**: `cargo check` limpo (achado à parte: cache de build do Rust em `target/` tinha caminhos absolutos presos da época em que o repo se chamava `practice-valuation`, antes do rename da Sessão 65 — limpo antes de compilar, não é um problema introduzido por esta sessão), `cargo test --lib` **110/110** (5 novos — `compute_balances` vazio/soma receita−despesa/mantém contas separadas, `validate_type` aceita os 2 tipos válidos e rejeita tipo desconhecido), `npx tsc --noEmit` limpo, e as 2 migrations novas **aplicadas de verdade** contra um SQLite descartável do zero (`cargo run -- up` na crate `migration`, todas as ~30 migrations do projeto rodaram em sequência sem erro), schema final conferido via `sqlite3 .schema` batendo exatamente com o desenho. **Teste funcional ao vivo rodado de verdade nesta sessão** (achado incomum — ambiente com `DISPLAY` real disponível, diferente de todas as sessões mobile): app subido via `docker compose up` (`desktop/`), janela real do Tauri capturada e dirigida via `spectacle`/`xdotool` (screenshot + clique/digitação simulados). **Achado no caminho**: o banco de dev real (`data-collector/anchor.db`, bind-mount usado pelo app em `cfg!(debug_assertions)`) não tinha as 2 migrations novas aplicadas — só o SQLite descartável de teste tinha; rodado `cargo run -- up` (crate `migration`) apontando pro arquivo real antes do teste (backup feito antes, por segurança, mudança é só `CREATE TABLE`). Fluxo completo confirmado no app rodando: criar conta bancária ("Nubank PJ"), editar nome inline ("Nubank PJ (main)", `update_bank_account`), lançar receita (5000,00, categoria "Salary") e despesa (1800,00, categoria "Rent"), saldo derivado na aba Accounts mostrando **3200,00** — exatamente `5000 − 1800`, confirmando `compute_balances` correto contra o banco real, não só os testes unitários.

**Fora de escopo, ainda não feito** (ordem sugerida pelo rascunho, não confirmada): `Liability`/dívida com geração automática de parcelas (Price/SAC), `Category` com limite de orçamento mensal, `Attachment`+Vault (depende de Fase 8.5, sync completo, não pronta), IA de extração/categorização de extrato (reaproveitaria a Fase 2.4, `document_extraction.rs`, mas ainda não é consumida aqui), reconciliação/dedupe de extrato importado, réplica mobile.

**Continuação — `Category` implementada + forms viram popup, mesma Sessão 71**: a ideia registrada acima (categoria selecionável, não texto livre) foi implementada na sequência, ainda na Sessão 71. **Decisão de modelo**: `general_transaction_categories` nasce como tabela própria — `id`/`workspace_id`/`nome`/`created_at`, sem o campo de limite de orçamento do `Category` do rascunho original (fora de escopo, só cadastro+seleção). `general_transactions.categoria` (texto livre) foi **removida** e substituída por `category_id` (FK opcional) — migration única fazendo `CREATE TABLE` da categoria nova + 2 `ALTER TABLE` separados (`ADD COLUMN category_id`, `DROP COLUMN categoria`; um `alter_table` por operação — SQLite/sea-query rejeita mais de uma por `ALTER TABLE`, mesmo achado de `add_equity_fields_to_assets`). Sem FK declarada em `category_id` (SQLite não suporta `ALTER TABLE ADD CONSTRAINT`, só em `CREATE TABLE` — nenhuma outra coluna de FK do projeto foi adicionada assim; a relação `belongs_to` do SeaORM funciona no nível do ORM independente de constraint no banco). Perda de dado aceita conscientemente: as 2 transações de teste da sessão anterior perderam a `categoria` em texto (só dado de teste).

**Segunda rodada de feedback, mesma sessão**: depois de testar a Category ao vivo, o dono do projeto pediu **popup em vez de tela/form fixo** pros 3 fluxos de criação (conta, categoria, transação) — "não sei se precisa de uma tela pra categorias, acho que pode ser um botão na tela de transações". Reaproveitado o padrão `Dialog` (shadcn) já usado em `AssetSection.tsx`/`NewValuationDialog.tsx`/`DocumentExtractDialog.tsx` — trigger button fora do popup, form dentro de `<DialogContent>`, fecha sozinho em `onSuccess` da mutation. **Mudança de estrutura**: a aba própria "Categories" (`CategorySection.tsx`, sub-aba de `FinancesPanel`) foi **removida por completo** — virou `CategoryManagerDialog.tsx` (cadastro+lista+exclusão, mesmo conteúdo, agora dentro de um popup), acionado por um botão "Manage categories" na aba Transactions, e também por um botão "New" ao lado do seletor de categoria dentro do próprio popup "New transaction" (criar categoria sem sair do fluxo de lançamento — os dois popups empilham, Radix Dialog suporta popup-sobre-popup sem conflito). `BankAccountSection.tsx`: form "Add account" virou botão "New account" + `Dialog` (edição inline na tabela continua igual, não mudou). `GeneralTransactionSection.tsx`: form "New transaction" virou botão + `Dialog`; a tabela de histórico (`Transaction history`) continua sempre visível na tela principal — só o formulário de criação é que virou popup. **Verificado de verdade**: `cargo check`/`cargo test --lib` 110/110 (só um teste ajustado — helper de teste `tx()` em `bank_account.rs` trocou `categoria: None` por `category_id: None`), `npx tsc --noEmit` limpo, migration aplicada contra o banco de dev real (`cargo run -- up`, schema conferido via `sqlite3 .schema`, coluna `categoria` sumiu/`category_id` apareceu, `general_transaction_categories` criada certa), testado no app real rodando (popup de conta abre/fecha/cria de verdade, popup de categoria idem — categoria "Salary" criada e listada ao vivo). **Limitação encontrada, não resolvida**: os componentes `Select` (dropdown de Type/Account/Category) dentro dos novos popups **não abriram** com clique/teclado sintético via `xdotool` nesta sessão — botões, inputs de texto e até popup-dentro-de-popup funcionaram normalmente nos mesmos popups, e é o mesmo componente `Select` que funcionava antes da conversão pra popup (confirmado ao vivo momentos antes, na fatia do núcleo). Avaliado como limitação da automação (XTest + Radix `Select` com portal aninhado dentro de `Dialog`, não um bug de código) — não confirmado com clique real de mouse. **Pendência real pro dono do projeto**: testar manualmente que os 3 seletores (Type/Account/Category) abrem e selecionam corretamente dentro do popup "New transaction", e que o fluxo completo (selecionar conta+categoria, "Log transaction") ainda funciona ponta a ponta.

**Continuação — `Liability` (dívida) com geração automática de parcelas, Sessão 71**: próxima fatia sugerida pelo rascunho, escolhida pelo dono do projeto. Três decisões confirmadas via `AskUserQuestion` antes de planejar (com `EnterPlanMode`): (1) suportar **Price e SAC**, selecionável por dívida; (2) incluir `linked_asset_id` (FK opcional pra `assets` do Portfolio) desde já no schema, sem calcular retorno alavancado ainda; (3) incluir um resumo simples de **dívida total** na aba, sem cruzar com o Portfolio.

**Achado do levantamento, corrigido no caminho**: `compute_balances` (`commands/bank_account.rs`) somava todas as `general_transactions` de uma conta sem filtrar por data — inofensivo enquanto só existiam lançamentos manuais (presumivelmente já ocorridos), mas quebraria assim que `Liability` passasse a gerar parcelas **futuras** de uma vez só (a conta apareceria com saldo já debitado de parcelas que ainda não venceram). Corrigido: só conta transações com `transaction_date <= hoje`.

**Modelo de dados**: `liabilities` (`id`, `workspace_id`, `bank_account_id` — de onde as parcelas são debitadas, `linked_asset_id` nullable, `nome`, `titular`, `principal`, `taxa_percentual` — mensal, guardada como o número digitado (ex. `1.5`), convertida pra decimal só na hora de chamar a matemática pura, mesmo espírito de `fi_taxa_percentual`, `indexador`, `prazo_meses`, `sistema_amortizacao` — `"price"`/`"sac"`, `data_inicio`, `created_at`). `liability_installments` (uma linha por parcela gerada: `liability_id`, `general_transaction_id` — o registro de fluxo de caixa de fato, `numero_parcela`, `valor_parcela`/`valor_juros`/`valor_amortizacao`/`saldo_devedor_apos`, `data_vencimento`). As duas tabelas nascem numa migration só, com FK completas desde a criação (SQLite não suporta `ALTER TABLE ADD CONSTRAINT`, então diferente de `category_id` — que teve que nascer sem FK declarada — aqui não há esse problema).

**Matemática pura, testada**: novo `domain/liability.rs` — `compute_installments` (Price: parcela fixa via `PMT = principal × [i(1+i)^n]/[(1+i)^n−1]`, com caso especial `i=0`; SAC: amortização fixa `principal/n`; última parcela de ambos absorve resíduo de arredondamento pra saldo final zerar exatamente) e `compute_outstanding_balance` (saldo devedor "hoje" — principal menos amortização de toda parcela já vencida, comparação lexicográfica de string `YYYY-MM-DD`). 10 testes: guardas (principal/taxa/prazo inválidos), parcela Price constante, amortização SAC constante, soma de amortizações = principal (os dois sistemas), parcelas SAC decrescentes, taxa zero sem juros, filtro de data do saldo devedor.

**Comandos**: `commands/liability.rs` — `create_liability` (valida sistema, chama a matemática pura, depois abre uma **transação de banco** — `db.begin()`/`TransactionTrait`, primeira vez usada no projeto — pra inserir a `Liability` + N `general_transaction`\[tipo `"parcela_divida"`, `notes` = `"{nome} — parcela {n}/{prazo}"`\] + N `liability_installment` atomicamente), `list_liabilities` (saldo devedor atual calculado em memória), `delete_liability` (deleta explicitamente os `general_transactions` linkados antes de deletar installments/liability — SQLite não garante cascade real aqui, sem `PRAGMA foreign_keys=ON` no projeto). `add_months` (soma meses a uma data, clampando o dia ao fim do mês de destino) é um helper local, mesmo espírito de `days_in_month`/`month_end_date` já usados em `commands/profitability.rs`, sem acoplar os dois módulos. `compute_balances` ganha `parcela_divida` como tipo que desconta do saldo (igual `despesa`).

**Frontend**: `finances/LiabilitySection.tsx` — terceira aba do `FinancesPanel` (Accounts/Liabilities/Transactions), tabela com saldo devedor + resumo "Total debt", popup "New liability" com o form completo (conta de pagamento e ativo vinculado opcional via `Select` alimentado por `list_bank_accounts`/`list_assets` — `Asset` importado de `portfolio/types.ts`, único ponto de acoplamento consciente entre os dois módulos, justificado pelo próprio campo `linked_asset_id`). `GENERAL_TRANSACTION_TYPE_LABELS` ganha `parcela_divida: "Loan installment"` (só rótulo — não é selecionável no form manual).

**Verificado**: `cargo check`/`cargo test --lib` **125/125** (15 novos — 10 de `domain::liability`, 3 de `add_months`, 2 de `compute_balances` com filtro de data), `npx tsc --noEmit` limpo, migration aplicada de verdade contra o banco de dev real (`sqlite3 .schema` conferindo as 2 tabelas com FK completas). **Teste ao vivo no app real não foi concluído** — achado incomum desta sessão: depois de dezenas de interações bem-sucedidas anteriores (núcleo + Category, mesmo app, mesma sessão), o popup "New liability" começou a apresentar um problema de entrega de eventos de teclado — teclas de controle (`Backspace`, `Home`/`End`, `Ctrl+A`, `Escape`) pararam de chegar ao webview (só caracteres digitáveis simples continuavam funcionando), impedindo limpar/corrigir campos digitados errado e fechar o diálogo via teclado. Reiniciar o container (`docker compose down`/`up`) não resolveu. **Não é um bug de código** — mesma UI/mesmos componentes (`Dialog`+`Select`+`Input`) que já funcionaram de ponta a ponta nesta mesma sessão pras telas de conta e categoria — é uma instabilidade do ambiente de automação (X11/XTest + WebKitGTK) que surgiu no meio da sessão. Confirmado via banco (`SELECT count(*) FROM liabilities` = 0) que nenhum registro foi criado — não há dado de teste sujo pra limpar. **Pendência real pro dono do projeto**: testar o fluxo completo de criar uma dívida (preencher o form, conferir que as parcelas aparecem certinho no histórico de transações com os valores de juros/amortização batendo, saldo devedor atualizando, exclusão desfazendo tudo). **Confirmado pelo dono do projeto no app real, logo depois** ("funcionou, mt top") — reforça que a instabilidade era mesmo só do ambiente de automação desta sessão, não um bug de código.

**Continuação — limite de orçamento mensal em `Category`, Sessão 75 (2026-08-11)**: fatia deixada de fora de propósito na criação da tabela (Sessão 71), sempre prevista no rascunho original (`Category` — "nome, tipo, limite mensal opcional"). Duas decisões confirmadas via `AskUserQuestion`: (1) limite é um **valor fixo recorrente** por categoria (`limite_mensal`, `Option<f64>` direto em `general_transaction_categories`), não um valor diferente por mês/ano específico; (2) progresso (gasto do mês atual vs. limite) aparece **dentro do popup "Manage categories"** já existente, sem tela/aba nova. Limite é sobre **gasto**, não receita — `type` da categoria em si não foi adicionado (categorias continuam utilizáveis tanto em `receita` quanto `despesa`, como já eram).

**Modelo de dados**: migration `m20260811_200000_add_budget_limit_to_general_transaction_categories`, um `alter_table`/`add_column` só (mesmo padrão de `add_equity_fields_to_assets`). Achado no caminho: `entity::general_transaction_category::Model` derivava `Eq`, que não compila com um campo `f64` (`Eq` não implementado pra ponto flutuante em Rust) — removido o derive, alinhando com `general_transaction`/`liability` (outras entities com campo `f64`, sem `Eq`).

**Backend**: `create_general_transaction_category` ganha `limite_mensal` opcional; novo comando `update_general_transaction_category` (categoria só tinha create/list/delete até aqui — mesmo molde de `update_bank_account`). Nova função pura testada `compute_spent_this_month(transactions, category_id, month_prefix)` — soma só `valor` de transações `despesa` daquela categoria cujo `transaction_date` cai no mês (`"YYYY-MM"`), mesmo espírito de `compute_balances`. `list_general_transaction_categories` passou a retornar `GeneralTransactionCategoryView` (`limite_mensal` + `spent_this_month` calculado em memória a partir de todas as `general_transactions` do workspace).

**Frontend**: `CategoryManagerDialog.tsx` ganhou campo "Monthly limit (optional)" no form de criação, edição inline por linha (mesmo idiom de `BankAccountSection.tsx`) e 2 colunas novas na tabela ("Limit", "Spent this month" com destaque `text-red-600` quando o gasto ultrapassa o limite).

**Verificado**: `cargo check`/`cargo test --lib` **130/130** (125 + 5 novos de `compute_spent_this_month`), `npx tsc --noEmit` limpo, migration aplicada contra o banco de dev real (backup `anchor.db.bak.session75` mantido), schema conferido via `sqlite3 .schema`. App real subido e navegado até o popup "Manage categories" — renderizou exatamente como desenhado (form, tabela com as colunas novas, edição inline ativando corretamente ao clicar "Edit"). **Mesma limitação de teclado já documentada na Sessão 71** (X11/XTest + WebKitGTK + Radix `Dialog`) impediu digitar em qualquer campo do popup (testado com `xdotool type` e tecla a tecla, tanto no campo numérico quanto no de texto) — não é um bug de código, é a mesma instabilidade de ambiente já vista e depois confirmada inofensiva pelo dono do projeto na Sessão 71. Pra conferir a lógica de leitura sem depender de teclado, valores foram gravados direto via `sqlite3`; uma segunda tentativa de reabrir o app pra visualizar o resultado esbarrou num problema à parte (tela "Choose a Workspace" pós-restart não avançava por clique) e não deu tempo de contornar — dados de teste foram revertidos antes de encerrar, banco real sem sujeira. **Pendência real pro dono do projeto**: testar o fluxo completo (criar/editar categoria com limite, lançar despesa nela, conferir "Spent this month" e o destaque vermelho ao estourar).

**Continuação — retorno alavancado (Alavancagem/spread cruzando `Liability`↔Portfolio), Sessão 82 (2026-08-16)**: ponto em aberto desde a criação do `Liability` (Sessão 71, item 2 acima — "sem calcular retorno alavancado ainda"). Escopo confirmado via `AskUserQuestion`: alavancagem e spread aparecem **dentro da aba Liabilities**, não em tela nova nem no Summary do Portfolio.

**Backend**: `domain::liability::annualize_monthly_rate` — anualiza a taxa mensal da dívida por capitalização composta (`(1+i)^12 - 1`), mesma matemática que `compute_installments` já aplica internamente. `domain::twr::trailing_twelve_month_pct` — retorno 12M a partir de uma série `r_cumulative_pct`, portado do precedente client-side `SummarySection.tsx::trailingTwelveMonthPct` (Fase 13.1) pro backend porque aqui o consumidor precisa de um número por ativo, não de uma série pra gráfico; diferente do precedente, com menos de 13 pontos mensais devolve `None` em vez de um número mal rotulado (a coluna é explicitamente "12M"). `domain::net_worth::compute_leverage(total_debt, net_worth)` — guard de divisão por PL zero/negativo. `commands::profitability::compute_profitability` mudou de assinatura — `portfolio_id: i32` virou `portfolio_ids: &[i32], asset_id: Option<i32>` — porque `Liability` só carrega contexto de workspace, não de portfolio, então o retorno do ativo vinculado precisa somar as transações dele em todos os portfolios do workspace da dívida; `asset_id: Some` isola o retorno de um único ativo em vez de agregar a sub-carteira inteira. `commands::bank_account::compute_total_cash` extraído de `compute_balances` (soma o saldo de todas as contas do workspace). `commands::liability::list_liabilities_view` — cada `LiabilityView` ganhou `annualized_debt_cost_pct`/`linked_asset_return_12m_pct`/`spread_12m_pct` (`Option<f64>`, aparecem juntos — sem `linked_asset_id`, "custo da dívida" sozinho não tem com o que ser comparado). Novo comando `commands::net_worth::get_net_worth_summary` — patrimônio líquido = valor de mercado de todas as posições de todos os portfolios do workspace + caixa − dívida total; alavancagem = dívida total / patrimônio líquido. Nenhuma migration nova — tudo derivado on-the-fly, mesmo padrão de `BankAccountView.balance`/`LiabilityView.saldo_devedor_atual`.

**Frontend**: `LiabilitySection.tsx` — faixa de 5 KPI cards (Market value/Cash/Total debt/Net worth/Leverage) substituindo a linha simples "Total debt" que existia antes; reaproveita o padrão `KpiCard` de `portfolio/ProfitabilitySection.tsx` (título + valor grande + subtexto opcional), duplicado localmente em vez de extraído pra util compartilhado (mesma convenção já estabelecida no projeto de cada arquivo ter sua própria cópia de helpers de formatação). Tabela ganhou 3 colunas — "Debt cost (p.a.)"/"Linked asset (12M)"/"Spread" — só preenchidas quando a dívida tem `linked_asset_id` (senão `"—"`); ticker do ativo vinculado resolvido via `assetsQuery` já carregada no componente, mostrado entre parênteses ao lado do retorno. Cor do spread verde (`text-primary`)/vermelha (`text-destructive`) via `pctColorClass`, mesmo precedente duplicado de `ProfitabilitySection.tsx`. Moeda segue a convenção do próprio módulo `finances/` (`.toFixed(2)`, sem `toLocaleString`/símbolo), diferente de `portfolio/` que usa `pt-BR`.

**Achado no início da sessão, não relacionado ao código em si**: a janela do Claude Code fechou (Alt+F4 acidental) no meio da sessão anterior, matando o plano recém-gerado antes de salvar em disco — mas o backend inteiro descrito acima já tinha sido escrito e ficou intacto no working tree, não commitado. Recuperado por levantamento direto do `git status`/`git diff` (sem o plano original pra reconstituir), confirmado compilando limpo e com testes passando antes de retomar o trabalho.

**Verificado**: `cargo check`/`cargo test --lib` **103/103** nos módulos de domínio tocados (`liability`/`twr`/`net_worth`), `npx tsc --noEmit`/`npm run build` limpos. App subido ao vivo via `desktop/dev.sh`, confirmado pelo dono do projeto com mouse real.

**Ainda fora de escopo**: retorno alavancado não aparece em nenhuma outra tela além da aba Liabilities (não cruza com o Summary do Portfolio); edição/quitação antecipada de dívida (ponto em aberto do rascunho original, ver item "`Liability`" abaixo); Vault/anexos financeiros, IA de extração, reconciliação e réplica mobile (Fase 12) seguem sem trabalho iniciado.

**Rascunho original completo, ainda não implementado além do núcleo + Category + Liability acima**:

**Visão geral**: dentro da área de investimentos, o usuário montaria suas próprias "partes" (workspaces/módulos plugáveis) — uma pode ser uma carteira de ativos (o que já existe hoje, Fase 10), outra pode ser um módulo de finanças gerais (dívidas, planejamento e controle de gastos). Tudo compartilharia o mesmo conceito de titular (PF/CPF ou PJ/CNPJ) já usado hoje em `Custodia` (Fase 10, item 9). Objetivo central: permitir registrar dívida tomada pela empresa/pessoa pra investir, e que isso influencie as métricas da carteira (alavancagem, retorno alavancado por ativo).

**Entidades principais propostas**:

- **`Transaction` (núcleo do sistema)** — modelo único cobrindo *toda* movimentação de dinheiro, não tabelas separadas pra "extrato bancário" e "extrato da carteira" (a segunda seria uma view filtrada da primeira). Campos: valor, data, `type`, categoria, `account_id`, titular. `type`: `receita`, `despesa`, `parcela_divida`, `compra_ativo`, `venda_ativo`, `aporte`, `provento` (dividendo/juros/aluguel de FII). Quando `type` é `compra_ativo`/`venda_ativo`: campos extras `asset_id`, quantidade, preço unitário. `source`: manual ou `generated_from_liability_id` (parcela gerada automaticamente). Tela "Transações gerais" = todas; tela "Transações da carteira" = filtro `WHERE type IN ('compra_ativo','venda_ativo')`. Proventos também gerariam `Transaction` automaticamente.

- **`Liability` (dívida/passivo)** — entidade irmã de `Asset`, não um "ativo negativo": tem semântica própria de amortização e gera fluxo de caixa. Campos: principal, taxa (fixa / CDI+spread / IPCA+ — mesmo vocabulário `fi_indexador` já usado em `transactions` pra renda fixa, Fase 10.2), prazo, sistema de amortização (Price ou SAC), data de início, titular (PF/PJ), `linked_asset_id` **opcional** (se a dívida financiou uma posição específica, permite calcular retorno alavancado daquele ativo). Ao cadastrar, geraria automaticamente as N `Transaction` futuras do tipo `parcela_divida`, já com split juros/amortização (só o juros conta como "gasto" no orçamento; amortização é devolução de principal). Métricas derivadas: Patrimônio Líquido = ativos − passivos; Alavancagem = dívida total / PL; spread/carry entre custo da dívida e retorno do ativo linkado. **Ponto em aberto do rascunho**: como editar/quitar uma parcela gerada automaticamente (pagamento antecipado, renegociação de taxa) — editar só aquela `Transaction` ou recalcular as futuras (padrão "essa e as futuras" vs. "só essa", como evento recorrente de calendário).

- **`Category`** — pra orçamento/controle de gastos: nome, tipo (receita/despesa), limite mensal opcional. Agregação de `Transaction` por categoria e período, comparado ao limite.

- **`BankAccount`** — conta bancária pra organização: banco, agência/conta (ou apelido), titular (PF/PJ), saldo derivado (soma das `Transaction` daquela conta, não editável diretamente). Toda `Transaction` teria `account_id` obrigatório.

- **`Attachment`** — tabela própria (não campo embutido em cada entidade), pra reuso em qualquer `Transaction`: nota fiscal, comprovante, contrato de dívida, extrato bancário anexado. Campos: referência ao arquivo, tipo, `transaction_id` (ou entidade vinculada). **Diferença chave do mecanismo de anexo já existente** (`thesis_attachments`/`asset_attachments`, Fase 10.5/10 item 8, ambos em disco local via `app_data_dir()`): o rascunho propõe que documentos financeiros fiquem **armazenados dentro do Vault (TruthID)**, não em disco local puro — ver integração com o Vault abaixo.

**Integração com o Vault (TruthID)**: documentos financeiros (notas fiscais, comprovantes, extratos) são mais sensíveis que o estado de app já pensado pro `SyncRegistry` (Fase 8) — como IPFS é content-addressed e público por padrão, o conteúdo precisaria ser **criptografado no cliente antes de subir** (mesmo padrão de `pin_content_cipher.rs`/`ecies.rs` já existente da Fase 8.6, HKDF+AES-256-GCM). **Ponto em aberto do rascunho**: o arquivo binário inteiro vai no blob sincronizado do Vault, ou só o metadado/hash da transação vai pro Vault (sync entre dispositivos) enquanto o arquivo fica local/IPFS separado, só referenciado pelo hash? Afeta volume e velocidade de sync. **Depende de Fase 8.5 (sync completo) existir antes de fazer sentido.**

**IA pra automação (extrato + anexos), dois passos separados**:
1. **Extração**: parsing de PDF/OFX/CSV/foto de extrato ou nota fiscal em linhas estruturadas (data, descrição, valor). Layout varia muito por banco/emissor — é o ponto que mais quebra, precisa de fallback manual. Reaproveitaria a infra de extração de documento já existente (Fase 2.4, `document_extraction.rs` — os 3 provedores de IA já aceitam PDF nativo + JSON validado contra schema).
2. **Categorização**: inferir `category` e `type` a partir da descrição. Se a descrição bate com uma `Liability` cadastrada (valor + dia do mês parecido), sugerir já linkar como `parcela_divida` daquela dívida em vez de virar despesa genérica.

**Regra fixa proposta**: nenhuma `Transaction` seria criada ou alterada pela IA sem confirmação do usuário — fila de "sugestões pendentes" que o usuário aprova/edita/rejeita em lote (nunca automático 100%), mesmo espírito do mecanismo humano-no-loop que `ai_valuation_proposal` (Fase 7.10.4) já implementa. Dois tipos de confirmação, tecnicamente distintos mas talvez unificáveis numa única tela: (1) confirmação de dado — usuário revisa/aprova o que a IA extraiu, local, sem tocar em chave nenhuma; (2) confirmação de escrita no Vault — assinatura via TruthID (sessão delegada) pra atualizar o CID do `VaultRegistry`, idealmente com batching (várias confirmações acumulam e sincronizam de uma vez, não uma escrita por lançamento).

**Reconciliação**: ao importar extrato bancário, pode haver duplicata com uma `Transaction` já criada manualmente (ex.: compra de ativo lançada na mão + o mesmo débito aparecendo no extrato importado depois). Precisaria de matching por valor + data (± tolerância) e estado de "possível duplicata" — nunca deletar/mesclar automático sem confirmação, mesmo princípio de nunca-automático-100% do resto do módulo.

**Ordem de implementação sugerida pelo rascunho** (não confirmada com o dono do projeto ainda):
1. `Transaction` + `BankAccount` (núcleo do fluxo de caixa)
2. `Liability` + geração automática de parcelas
3. `Category` + controle de gastos/orçamento
4. `Attachment` + integração com Vault (criptografia antes de tudo)
5. IA de extração/categorização de extrato e notas fiscais (com fila de confirmação)
6. Reconciliação/deduplicação

**Dependências e sobreposições com fases existentes, não resolvidas**: `Attachment`+Vault depende de Fase 8.5 (sync completo, não iniciada) pra fazer sentido de verdade — sem sync, é só mais um disco local, o que o mecanismo mais simples de `thesis_attachments`/`asset_attachments` já resolve hoje. `Transaction` como "núcleo único" se sobrepõe conceitualmente a `transactions` (Fase 10.2, hoje só cobre compra/venda/aporte/retirada/provento/transferência de ativos) — precisa decidir se este módulo estende a tabela existente ou nasce paralelo com um relacionamento entre as duas. `Liability` puxaria o mesmo vocabulário de indexador/taxa que os campos `fi_*` de `transactions` já usam pra renda fixa.

### Fase 13 — Redesenho da Carteira (Portfolio "estilo investidor10", ideia levantada na Sessão 75)

**Objetivo**: dono do projeto pediu pra repensar a experiência de Carteira de ponta a ponta, com o site [investidor10.com.br](https://investidor10.com.br) como referência de modelo — na visão dele, a Carteira hoje "mal existe". Confirmado por um agente `Explore` que mapeou o estado atual: `PortfolioPanel.tsx` é uma faixa plana de 6 abas (Transactions & Positions, Profitability, Assets, Custodies, Watchlists, Theses), sem nenhuma tela de resumo/visão geral — `TransactionSection.tsx` mistura form de lançamento, lista de transações e tabela de posições num componente só. Mesmo feedback já registrado na Fase 10.5 sobre Teses ("dps temos que dar uma melhorada geral... importante é funcionar por enquanto, não é assim que eu imaginei o UX") se aplica à Carteira inteira, não só a Teses. **Puro registro — nenhuma decisão de escopo/arquitetura tomada ainda.**

**Referência visual confirmada na Sessão 76**: sem ferramenta de navegador disponível nesta sessão (CLI, sem extensão Claude in Chrome conectada), o dono do projeto trouxe 10 screenshots da própria conta (`/home/masterlxz/Pictures/Screenshots/Screenshot_20260811_2117*.png` a `2120*.png`) cobrindo Resumo, Proventos, Patrimônio, Rentabilidade e Lançamentos. Analisados só pela estrutura de layout — nenhum valor monetário, ticker ou nome de ativo da carteira real do dono do projeto foi copiado pra este documento (dado sensível, fica só nos prints locais dele). Achado importante: **o site tem 10 abas, não 5** — Resumo, Posições, Proventos, Patrimônio, Rentabilidade, Análise, Lançamentos, DARFs, IRPF, Metas — mais dois controles persistentes no header em toda aba (botão "Integração B3" e botão primário "+ Adicionar Lançamento"). O pedido original (Sessão 75) cobriu só 5; Análise, DARFs (guia de recolhimento de IR sobre ganho de capital), IRPF (suporte à declaração anual) e Metas ficam **fora de escopo por enquanto**, registrados aqui só pra não esquecer que existem no modelo de referência.

**Cinco telas pedidas — todas implementadas (Sessões 78-81), fase fechada**:

- [x] 13.1 — **Resumo (Summary/Overview)** — **fatia enxuta implementada na Sessão 78** (`SummarySection.tsx`, nova aba "Resumo", primeira da lista/aba padrão do Portfolio). Escopo completo original abaixo pra referência; decisão da Sessão 78 (`AskUserQuestion`) foi construir só o que tinha dado real disponível — o resto fica pendente, listado no final deste item.
  - Estrutura completa pedida (referência, prints da Sessão 76): **4 KPI cards** no topo (Patrimônio total com % de variação e "Valor investido" como subtexto; Lucro total com "Ganho de Capital"/"Dividendos Recebidos" lado a lado; Proventos Recebidos 12M; um card duplo com Rentabilidade 12M + Rentabilidade Total lado a lado). Abaixo, **2 gráficos lado a lado**: "Evolução do Patrimônio" (barra composta, Valor aplicado + Ganho de Capital empilhados, filtro de período tipo "12 Meses" + filtro "Todos os tipos" — mesmo padrão de filtro de período já usado em `stock-lookup/PriceHistoryChart.tsx`) e "Ativos na Carteira" (donut por classe com legenda e %, filtro próprio). Abaixo dos gráficos, **"Meus Ativos"**: grupos colapsáveis por classe (Ações/Criptomoedas/Tesouro Direto/...), cabeçalho de cada grupo arrastável (reordenável) mostrando contagem/valor total/variação/rentabilidade/"% na carteira" (dois números — real vs. algo tipo alvo); expandido revela tabela com colunas Ativo/Quant./Preço Médio (editável)/Preço Atual/Preço-Alvo (com alerta tipo sino por linha)/Variação/Saldo/P/L/P/VP/ROE/Margem Líquida/% Carteira/Opções, todas ordenáveis. Rodapé de cada grupo tem 4 ações: "Lançamentos", "Gráficos", **"Editar colunas"** (confirma que colunas selecionáveis pelo usuário é um padrão real do site, não invenção) e "Adicionar Lançamento".
  - **O que foi implementado** (Sessão 78): backend novo `domain::position_pricing::price_position` (função pura, 7 testes) precifica cada posição por classe — cotação automática (`stock_price_history`, mesma tabela de `profitability.rs`) pras 9 classes de `ASSET_CLASSES_WITH_AUTO_QUOTE`, última reavaliação manual (`asset_valuations`) pra Imóvel/Empresa não listada, fallback pro preço médio de compra pra Tesouro Direto/Renda Fixa (e qualquer classe sem fonte melhor) — sinalizado via `price_source` (`market`/`manual_valuation`/`avg_buy_price`/`none`). `get_portfolio_positions`/`PositionView` (`commands/transaction.rs`) ganharam `current_price`/`market_value`/`unrealized_pl`/`unrealized_pl_pct`/`price_source` (aditivo, sem quebrar `TransactionSection.tsx`); a reconstrução de posição foi extraída pra `compute_positions`, reaproveitada pelo comando novo `get_portfolio_summary` (`commands/portfolio_summary.rs`) que agrega patrimônio total/investido/L&P, proventos recebidos (12M), alocação por classe e conta de posições sem preço. Frontend: 4 KPI cards (o de Rentabilidade reaproveita `get_portfolio_profitability` sem mudar esse comando, rebaseando a série encadeada — ver `trailingTwelveMonthPct` em `SummarySection.tsx`), donut de alocação (`recharts`, primeiro donut do app — paleta categórica validada pela skill `dataviz` contra a superfície fixa do app, `#111820`, 8/8 checks OK), "Meus Ativos" agrupado por classe com `Collapsible` (shadcn, já existia no repo, não usado ainda) e colunas Ativo/Quantidade/Preço Médio/Preço Atual/Saldo/Variação/% Carteira. Validado contra o banco real de dev (1 posição, BBAS3): `cargo test --lib` **147/147**, `tsc --noEmit` limpo, e um teste temporário (`compute_positions` chamado direto contra o SQLite real, depois removido) confirmou os números batendo à mão (preço médio R$20,00, preço atual R$20,06, saldo R$20,06, L&P +R$0,06/+0,30%). **Não testado na GUI real** — `dev.sh` sobe o app via Docker + X11 (`xhost +local:docker`), sem display disponível neste ambiente de execução; fica pro dono do projeto rodar `desktop/dev.sh` e abrir a aba Resumo.
  - **Ficou de fora, pendência clara pra continuar depois**: gráfico "Evolução do Patrimônio" (exigiria reconstruir série mensal de valor de mercado cruzando as 13 classes, incluindo as 4 sem histórico de preço automatizado — mais trabalho que o resto desta fatia, decisão de escopo da Sessão 78 de deixar pra depois); Preço-Alvo por posição com alerta tipo sino (não existe schema pra isso hoje, só `watchlist_items.target_price`, por watchlist — não por posição); colunas de fundamentos na tabela (P/L, P/VP, ROE, EV/EBITDA, Margem Líquida — nenhuma armazenada de forma pronta pra uso direto, ROE existe só em `stock_fundamentals` por ticker de ação BR); "Editar colunas" (preferência de usuário, decisão de onde guardar ainda em aberto); grupos arrastáveis/reordenáveis; toggle "Exibir posição ideal" (alocação real vs. alvo); "Rentabilidade 12M"/"Total" lado a lado com comparativo tipo "X% acima/abaixo do CDI" (implementado só o número, sem o comparativo); alocação por região/país (ainda esbarra no limite de 1 valor de exposição por ativo, Sessão 33, Fase 10.2).
- [x] 13.2 — **Lançamentos (Ledger)** — **implementada por completo (gráfico + tabela) na Sessão 79**, nova aba "Ledger" (nome em inglês — o rascunho abaixo usa "Lançamentos" só por vir do site em português) logo após "Summary", antes de "Transactions & Positions" (que perdeu a lista de lançamentos, mantendo só o formulário "New transaction" e a tabela "Consolidated positions"). Estrutura pedida originalmente: gráfico "Consolidação de aportes" no topo (barras Compras acima do zero / Vendas espelhadas abaixo, por mês, tooltip com valores exatos ao passar o mouse, filtros Mensal/2 Anos + tipo de ativo), abaixo tabela de lançamentos **agrupada por classe** (mesmos grupos colapsáveis do Resumo) com colunas Ativo/Tipo de investimento/Tipo de ordem (badge Compra/Venda)/Quantidade/Preço unitário/Total/**Quantidade Total** (posição acumulada correndo, linha a linha)/Data do lançamento/**Fonte** (badge "B3" quando veio de importação automática)/Opções.
  - **O que foi implementado** (Sessão 79, escopo completo confirmado via `AskUserQuestion` — sem cortar pra fatia enxuta desta vez): backend novo `domain::transaction_ledger::running_quantities` (função pura, 5 testes) calcula a quantidade líquida do ativo após cada lançamento, mesmas regras de `compute_positions` (compra soma/venda subtrai/resto neutro), empate de data desfeito por `id` ascendente. `TransactionView`/`list_transactions` (`commands/transaction.rs`) ganharam `asset_class` e `running_quantity` (aditivo). Frontend `LedgerSection.tsx`: gráfico "Monthly contributions" (`recharts` `BarChart`, agregação mensal 100% client-side, vendas negativas pro efeito espelhado, par de cores azul/vermelho validado via skill `dataviz` reaproveitando cores já existentes no app — azul de `acao_br` da donut do Resumo, vermelho `--destructive` do tema) com filtros de período (12M/24M/All) e classe de ativo; tabela "Transactions" agrupada por classe com `Collapsible` (mesmo padrão do Resumo), aporte/retirada caem num grupo sintético "Cash flow" sempre por último, badge de tipo de ordem colorido (azul compra/vermelho venda/secundário pro resto), coluna Source com badge "Manual" fixo (sem importação B3 ainda — coluna só preparada pro futuro). `cargo test --lib` **152/152**, `tsc --noEmit` limpo, testado ao vivo no app real (chart+tabela renderizando certo contra o banco de dev, BBAS3) e **confirmado pelo dono do projeto com mouse real** que o colapsar de grupo e os filtros funcionam (a automação via `xdotool` não conseguiu confirmar isso na sessão). Commit `609eba0`.
  - **Achado corrigido no caminho, antes de começar a 13.2**: `SummarySection.tsx` (Fase 13.1, Sessão 78) tinha ido pro ar em português, violando `GUIDELINES.md` — corrigido nesta sessão (labels/subtextos/headers/tooltips traduzidos, aba "Resumo" → "Summary"), junto de um placeholder solto em português achado de brinde em `AboutCompanySection.tsx` (Fase 7, Sessão 60).
- [x] 13.3 — **Posições** — **fatia enxuta implementada na Sessão 80**. Tela dedicada, separada dos lançamentos (mesma separação do item acima) — provavelmente uma versão maior/isolada da tabela "Meus Ativos" do Resumo, como já suspeitado abaixo.
  - **Achado antes de começar**: a frase original deste item ("`get_portfolio_positions`/`PositionView` hoje devolve só quantidade e preço médio de compra — sem cotação atual, valor de mercado nem P&L") **estava desatualizada** — ficou obsoleta desde a Fase 13.1 (Sessão 78), que já tinha adicionado `current_price`/`market_value`/`unrealized_pl`/`unrealized_pl_pct`/`price_source` a esse mesmo comando (via `domain::position_pricing::price_position`), só que sem essa nota ser corrigida na hora. Confirmado por agente `Explore` antes de planejar — **nenhum comando novo de backend precisou ser escrito** pra esta fatia, tudo já vinha pronto do que a 13.1 fez.
  - **Escopo confirmado via `AskUserQuestion`** (fatia enxuta, entre 3 opções de tamanho): reaproveitar o padrão de agrupamento por classe (`Collapsible`) já usado em "Meus Ativos" do Resumo, mas como aba própria e maior, sem preço-alvo/alerta, sem editar colunas, sem P/L·P/VP·ROE·EV/EBITDA·Margem Líquida, sem grupos arrastáveis, sem % Carteira real-vs-alvo — tudo isso fica de fora, pendência clara pra uma fatia futura (ver "Padrões transversais" acima).
  - **O que foi implementado**: `PositionsSection.tsx` novo, montado na aba nova "Positions" (`PortfolioPanel.tsx`, logo após "Summary", antes de "Ledger"). Mesmo agrupamento por classe/`Collapsible` de `SummarySection.tsx` reaproveitado (duplicado localmente, não extraído pra util compartilhado — segue a convenção já estabelecida no repo de duplicar helpers pequenos por arquivo, ver `formatCurrency`/`formatPct` já duplicados em 5 arquivos do `portfolio/`, nenhum `utils.ts` compartilhado existe). Colunas: Asset/Quantity/Avg. price/Current price/Market value/**Unrealized P&L** (valor em moeda, novo — a tabela do Resumo só mostra a versão em %)/P&L %/% Portfolio/**By custody** (saiu da tabela "Consolidated positions", removida de `TransactionSection.tsx` nesta mesma sessão — mesmo precedente da Sessão 79, que moveu a lista de lançamentos pra fora de `TransactionSection.tsx` quando a aba Ledger nasceu; `TransactionSection.tsx` agora tem só o formulário "New transaction", a query/invalidação de `["positions", portfolioId]` continuou lá só pra manter o cache consistente com a aba nova).
  - **Verificado**: `npx tsc --noEmit` limpo (mudança 100% frontend, backend não tocado). Testado ao vivo no app real (Docker + X11 disponível nesta sessão, `docker compose up` + `xhost +local:docker`, janela "Anchor" confirmada via `xdotool search --name`) — **confirmado pelo dono do projeto com mouse real** ("já deu certo, confirmei aqui"), mesmo padrão de confirmação manual já visto nas Sessões 75/79 pra interação de clique/collapse que a automação via `xdotool` não confirma de forma confiável.
- [x] 13.4 — **Proventos** — **implementada na Sessão 81**, tela dedicada nova (`DividendsSummarySection.tsx`, aba "Dividends"). Estrutura completa pedida (referência, prints da Sessão 76) pra registro: sidebar esquerda com "Resumo" (Média Mensal últ. 12 meses + **"Criar meta"** com barra de progresso — ligação direta com a aba Metas fora de escopo, ver acima; Total de 12 meses; Total da carteira; donut "Distribuição de proventos em 12 meses" por ativo); à direita "Evolução de Proventos" (barra Mensal/Anual, empilhando **Proventos recebidos** (sólido) sobre **Proventos a receber** (translúcido) — ou seja, o gráfico já projeta proventos anunciados e ainda não pagos, não só o histórico); abaixo "Histórico mensal" (tabela Ano × Jan-Dez + Média + Total, multi-ano); abaixo "Meus proventos" (linha a linha: Ativo/Tipo de ativo/**Status do pagamento** badge "A Receber"/"Pago"/Tipo de pagamento — Dividendos vs. **JSCP** com IR retido na fonte, daí a coluna extra/Data Com/Data Pagamento/Quantidade/Valor do dividendo por cota/Valor total/**Total líquido** já descontando IR do JSCP).
  - **Decisões de escopo via `AskUserQuestion`** (Sessão 81): JSCP **entra** nesta fase (distinção real + retenção fixa de 15% de IR, alimentando "Total líquido" de verdade — não uma versão sem JSCP, que também foi oferecida); card **"Criar meta"** fica de fora (sem integração desenhada com a aba Metas); yield on cost também fica de fora (ambíguo no pedido original).
  - **O que foi implementado**: schema novo — `payment_type` (Dividendo/JSCP) em `suggested_dividends` (NOT NULL, default `"dividendo"`) e em `transactions` (nullable, só significativo quando `transaction_type = "provento"`), propagado pelos 3 fluxos de escrita (gerar sugestão via Yahoo sempre nasce Dividendo, já que a fonte não distingue; confirmar sugestão deixa corrigir o tipo; lançamento manual de "provento" ganhou o mesmo seletor). Novo `domain::proventos` (função pura, 7 testes): `net_total` (retenção fixa de 15% só no JSCP), `bucket_monthly` (recebido vs. a receber por mês) e `by_asset` (soma por ativo pra donut). Novo comando `get_proventos_summary` (`commands/proventos.rs`) — combina `transactions` (pago) com `suggested_dividends` ainda não confirmado/descartado (a receber), recupera Data Com de linhas pagas via junção `(asset_id, payment_date)` contra a sugestão `confirmed` correspondente (`confirm_dividend_suggestion` não guarda o id da transação criada, mas grava a mesma data/ativo). KPIs e "Total líquido" são todos **líquidos de IR** (Média Mensal 12M = total líquido/12 fixo, não dividido pela contagem de meses com dado); `get_portfolio_summary::dividends_received_12m` (Resumo, Fase 13.1) não foi tocado e continua bruto — divergência aceita entre as duas telas, não é bug.
  - **Frontend**: donut colorida por **ranking** (top-7 ativos + "Others"), não por classe fixa — primeiro caso do app onde a cor segue a posição no ranking em vez da identidade, já que aqui é genuinamente um "top N", não um filtro que muda de composição. Gráfico de evolução é a **primeira barra empilhada do app** (`stackId` compartilhado, recebido sólido + a receber a 35% de opacidade + traço de borda) — decisão validada consultando o skill `dataviz` antes de implementar (opacidade separa por luminância, sobrevive a daltonismo; legenda sempre visível evita depender só da cor).
  - **Verificado**: `cargo check`/`cargo test --lib` **159/159** (7 novos), `tsc --noEmit` limpo. Migrations aplicadas no banco real de dev via `sea-orm-cli migrate up` (mesmo achado de sempre: app não migra sozinho no startup). Testado ao vivo pelo dono do projeto (app subido via `desktop/dev.sh` a pedido dele) — confirmado funcionando. Commit `f218515`.
  - **Ajuste de UX, Sessão 81.3 (2026-08-15, mesmo dia)**: dono do projeto pediu pra mover a revisão de sugestões de provento (gerar/confirmar/descartar/registrar futuro, Fase 13.6) de aba própria ("Dividend suggestions") pra dentro da aba "Dividends", como popup — clicando um botão "Manage suggestions" em `DividendsSummarySection.tsx`. Mesmo componente `DividendSuggestionsSection`, só mudou onde ele vive na UI (`PortfolioPanel.tsx` perdeu a aba dedicada). Mudança pequena, só frontend, sem plano formal. Commit `59281bc`.
- [x] 13.5 — **Rentabilidade** — **implementada na Sessão 81**, expandindo a aba já existente (`ProfitabilitySection.tsx`, Fase 10.3). Estrutura completa pedida (referência, prints da Sessão 76) pra registro: 3 cards empilhados à esquerda (Rentabilidade total / Últimos 12 meses / Último mês, cada um com um comparativo expansível tipo "X% abaixo/acima do CDI"); à direita "Rentabilidade comparada com índices" — gráfico de linha com a rentabilidade da carteira sobreposta a **7 benchmarks simultâneos, ligáveis/desligáveis pela legenda**: CDI, IPCA, IFIX, IBOV, SMLL, IDIV, IVVB11. Abaixo, tabela "Rentabilidade" (Ano × Jan-Dez % colorido + Retorno anual + Acumulado).
  - **Pesquisa de fonte de dado antes de planejar** (1 agente `Explore` de código + 1 agente `Explore` fazendo pesquisa/requisições reais): dos 7 benchmarks pedidos, só **4 têm fonte histórica gratuita** — CDI e IPCA via API SGS do Banco Central (séries 4391/433, JSON público, sem chave), IBOV via Yahoo Finance (`^BVSP`, mesmo endpoint que o app já usa pra ações), e IVVB11 é só um ETF B3 comum (zero fonte nova, reaproveita 100% a infra existente). **IFIX/SMLL/IDIV ficam de fora — sem fonte gratuita achada**: Yahoo só devolve cotação atual pra esses índices (sem histórico backfillable), a API interna do B3 (`sistemaswebb3-listados.b3.com.br`, engenharia reversa feita nesta sessão) não expõe a série de pontos do índice (só valor de mercado agregado dos ativos que o compõem), e o brapi.dev exige plano pago pra histórico. **Pendência registrada, não resolvida**: decidir depois se assina o brapi.dev ou aceita entrada manual pra esses 3.
  - **Decisões de escopo via `AskUserQuestion`**: (1) implementar os 4 com fonte gratuita agora, IFIX/SMLL/IDIV ficam pendência (a alternativa, esperar os 7 completos, foi oferecida e recusada); (2) mantém o recorte atual de `get_portfolio_profitability`/`ASSET_CLASSES_WITH_AUTO_QUOTE` (9 das 13 classes) sem tentar expandir — Tesouro Direto/Renda Fixa/Imóvel/Empresa não listada continuam fora da "Rentabilidade geral", agora uma pendência **explícita** (documentada), não implícita; (3) fatia completa (KPIs + gráfico + tabela) numa sessão só, mesmo padrão das Fases 13.2/13.4. Plano formal via `EnterPlanMode`/`ExitPlanMode`.
  - **O que foi implementado**: schema novo só pros 2 benchmarks "macro" — `macro_index_monthly` (`index_code`/`year_month`/`value_pct`, `INSERT OR REPLACE` em vez de IGNORE porque a BCB pode revisar um valor recém-publicado). IBOV/IVVB11 **não** ganham tabela nova — reaproveitam 100% `stock_price_history` (mesma tabela de qualquer ativo), com um `domain::benchmark::monthly_returns_from_prices` novo (função pura, 4 testes) calculando retorno mensal a partir do preço mais próximo de cada fim de mês, pulando mês sem dado em vez de falhar tudo-ou-nada (mês corrente ainda não publicado é normal, não erro). `get_portfolio_profitability` foi extraído pra `compute_profitability` reaproveitável (mesmo precedente de `compute_positions`, Fase 13.1); novo comando `get_profitability_comparison` junta a carteira com os benchmarks disponíveis, recortados pra mesma janela de meses da carteira antes de qualquer `chain()` (comparar com anos de histórico de CDI enquanto a carteira só tem 2 meses seria enganoso). Novo coletor Python (`data-collector/sources/bcb_sgs.py` + flag `--benchmark-returns`).
  - **Paleta do gráfico corrigida durante a implementação**: o agente de planejamento alegou que 5 cores categóricas passavam a validação da skill `dataviz` — rodando o validador de verdade (`validate_palette.js --pairs all`, não só pares adjacentes), a alegação **estava errada**, falhava em 2 pares. Confirmado que nenhuma combinação de mais de 3 das 8 cores categóricas do app passa a checagem completa (documentado no próprio `references/palette.md` da skill). Solução: só 3 matizes de verdade (os únicos 3 validados), reusados em pares por traço sólido/tracejado — cada par usa o mesmo hex, então não existe ambiguidade de cor entre eles.
  - **Verificado de verdade**: `cargo check`/`cargo test --lib` **163/163** (4 novos), `tsc --noEmit` limpo. Migration aplicada no banco real de dev via `sea-orm-cli migrate up`. **Coletor rodado contra as APIs reais** (BCB SGS + Yahoo, fora do Docker, venv efêmero criado pra isso): 481 meses de CDI, 559 de IPCA, 2484 pregões de IBOV, 2490 de IVVB11 gravados certo no banco real; rodado de novo pra confirmar idempotência (IBOV/IVVB11 zero linha nova; CDI/IPCA sem crescer, contagem de linhas idêntica). Teste visual na GUI (abrir a aba, clicar nos botões/legenda) fica pro dono do projeto.
  - **Pendência fechada na Sessão 83**: pesquisa nova (a partir do pacote R `rb3`/rOpenSci, que documenta publicamente como consome dados da B3) achou uma rota **diferente** da tentada na Sessão 81.2 — `indexStatisticsProxy` em vez de `indexPage`/`indexProxy` — que expõe sim a série histórica de pontos do índice: `GET https://sistemaswebb3-listados.b3.com.br/indexStatisticsProxy/IndexCall/GetPortfolioDay/{base64({"language":"en-us","index":"IFIX","year":2024})}`, sem chave, sem autenticação, cert válido. Testado ao vivo pros 3 índices: **IFIX desde dez/2010, SMLL e IDIV desde ago/2005**. Novo coletor `data-collector/sources/b3_index_stats.py` (`fetch_index_history`, sem I/O de banco) + `collect_b3_index_history` em `main.py`, gravando em `stock_price_history` (ticker `IFIX`/`SMLL`/`IDIV`) — mesmo tratamento de IBOV/IVVB11, zero migration nova, zero lógica Rust nova (só mais 3 chamadas a `build_price_series` em `get_profitability_comparison`). **Decisão de paleta** (`AskUserQuestion` com o dono do projeto): como o validador só aprova 3 matizes pro app inteiro e os 4 benchmarks já usavam as 3 em pares sólido/tracejado, IFIX/SMLL/IDIV entraram no MESMO gráfico reaproveitando o matiz do grupo "mercado" (IBOV/IVVB11), diferenciados só por `dashArray` (pontilhado/traço-ponto/traço longo), sem gráfico novo nem 4º matiz. **Verificado de verdade**: `cargo check`/`cargo test --lib` **170/170**, `tsc --noEmit` limpo, coletor rodado de verdade dentro do Docker (`--benchmark-returns`) — 3881/5196/5114 pontos gravados em `stock_price_history` com `source='b3_index_stats'`, indo até 2010-12/2005-12 respectivamente, conferido via `sqlite3`. Testado no app real (`docker compose up`, screenshot via `spectacle` + `xdotool`): as 7 séries renderizam no gráfico "Return vs. benchmarks", legenda clicável esconde/mostra a IDIV normalmente. **Limitação não resolvida**: a legibilidade dos 3 padrões de traço novos (todos no mesmo matiz aqua) não deu pra avaliar direito com a carteira de dev usada no teste (só ~2 meses de histórico, linhas comprimidas perto de 0%) — fica pro dono do projeto olhar com a carteira real e ajustar os padrões se precisar.

**Padrões transversais confirmados pelos prints, além dos 5 itens acima**: toggle **"Exibir posição ideal"** (comparar alocação real vs. alvo) aparece em vários gráficos de composição na aba Patrimônio (Tipo de ativos / Ativos / Exposição ao exterior, cada um como sub-aba do mesmo card) — conceito de alocação-alvo por ativo/classe que o Anchor não tem hoje, relacionado à decisão em aberto do item 13.1 sobre "% na carteira" mostrar dois números. Header inteiro do site (fixo em toda aba) tem: troca de carteira (dropdown, útil se o dono do projeto tiver mais de uma — mapeia pra múltiplas `Portfolio`/Custódias já existentes no Anchor), ícone de "olho" pra **modo privacidade** (esconder valores em R$ sem sair da tela — relevante já que este documento evitou registrar valores reais), toggle de tema claro/escuro.

**Infra reaproveitável já existente**: `recharts` (já dependência, usado em `stock-lookup/PriceHistoryChart.tsx`/`DividendHistoryChart.tsx`/`portfolio/ProfitabilitySection.tsx`) é o precedente natural pros gráficos novos, mesmo padrão `ResponsiveContainer` + tooltip customizado — evita introduzir uma segunda lib de gráficos.

### Fase 13.6 — Automação de Proventos (ideia levantada na Sessão 75, desenhada na Sessão 76, parte "passado" + conciliação implementadas na Sessão 77/77.2, parte "futuro" implementada na Sessão 94 via CVM em vez de brapi.dev; **parte "futuro" removida na Sessão 101, Fase 20** — dependia de IA pra ler o PDF do aviso)

**Parte "futuro" — Sessão 94 (2026-08-30)**: brapi.dev, cogitado desde a Sessão 76, **não tem
plano gratuito pra ticker real** (confirmado ao vivo — só sandbox de 4 tickers sem token; real
exige assinar Startup, R$99,99/mês+) — dono do projeto pediu pra avaliar a alternativa gratuita
já registrada, CVM Dados Abertos. Achado que destrava: a categoria **"Relatório Proventos"** do
dataset `cia_aberta-doc-ipe` (481 documentos em 2026) é um **formulário padronizado da CVM**
(confirmado baixando um PDF real, Banco do Brasil) — Valor Bruto/cota, Data Pagamento, "Último
dia de negociação com Direitos" (= Data Com) e ISIN por classe de ação, tudo em campos fixos —
bem mais confiável que Fato Relevante/Comunicado ao Mercado (texto livre, maioria não é sobre
provento apesar de mencionar a palavra). Extração via IA (mesmo formulário, mas a ordem do texto
cru sai embaralhada pelo layout multi-coluna — resolvido lendo o PDF nativamente, não
`pdftotext`).

**Arquitetura**: mesmo padrão da Fase 14 — fonte externa nova centralizada no `easybusiness`
(`api/app/sources/cvm_ipe.py`, mirror curto de `cvm_dfp.py`; diferença: o zip do ano corrente
muda o ano inteiro, não cacheia em disco pra sempre como o DFP faz; endpoint novo
`GET /v1/companies/{cvm_code}/dividend-notices`), Anchor consome via
`finance_api::client::fetch_company_dividend_notices`. Resolução ticker→`cvm_code` reaproveita o
caminho que já existia pra fundamentos (`fetch_stock_bolsai_fundamentals`, extraído pra
`finance_api::stocks::resolve_cvm_code` — ganhou um segundo chamador). Extração via IA reaproveita
as 3 funções por provider de `commands::document_extraction` (viraram `pub(crate)`, já recebiam
`pdf_base64` puro, sem precisar de arquivo local). Sugestão vira `suggested_dividends` direto
(`source = "cvm"`) — **zero tela de revisão nova**, mesmo fluxo de confirmar/editar/descartar que
o "passado" (Yahoo) já tinha. Ledger de idempotência novo (`cvm_dividend_notice_documents`) evita
reprocessar o mesmo documento — mas só grava permanentemente resultado *estável* (sucesso,
ambíguo, `payment_date` já passado, ou "não é PDF de verdade"); falha transitória (rede, Gemini
503) não entra no ledger, fica elegível pra nova tentativa no próximo clique.

**3 achados reais só descobertos testando ao vivo contra o portfolio real do dono do projeto**
(BBAS3/Banco do Brasil, `cvm_code` 1023, único ticker de Ação BR que ele tinha lançado): (1)
`extract_via_gemini` (função já existente da Fase 2.4/2.5, nunca antes exercitada de verdade
contra Gemini) manda o schema em JSON Schema padrão, mas a API do Gemini só aceita um
subconjunto OpenAPI — sem `additionalProperties`, `type` precisa ser string maiúscula única
(nulidade via `nullable: true`, não `type: ["string","null"]`) — corrigido na fonte
(`to_gemini_schema`, conversor recursivo) pra beneficiar qualquer chamador futuro, não só este;
(2) a quantidade tinha que ser reconstruída na data do **próprio aviso** (`payment_date`), não em
"hoje" — a CVM devolve o histórico inteiro de "Relatório Proventos" (passado e futuro juntos),
não só os futuros, e usar "hoje" geraria sugestão de dinheiro que quem comprou depois de um
provento antigo nunca recebeu; corrigido, e escopo restrito a `payment_date > hoje` (o "passado"
já é do fluxo Yahoo, mais confiável — evita duplicar); (3) a CVM devolve uma página de erro HTML
no lugar do PDF de verdade pra algumas versões antigas de filing (`numVersao` superseded) — sem
checar isso, o app gastava uma chamada de IA numa "PDF" que era HTML, sempre falhando do mesmo
jeito; corrigido com um guard de magic bytes (`%PDF-`) antes de gastar a chamada de IA,
classificado como falha permanente (não retry) já que é sempre o mesmo resultado.

**Resultado real, confirmado no banco de dev do dono do projeto**: 16 "Relatório Proventos" da
CVM pra BBAS3, 1 sugestão futura criada (JSCP, R$0,10245643978/cota, pagamento 2026-09-11, Data
Com 2026-09-01, `status = pending`, aguardando confirmação do dono do projeto), 6 já passados
(cobertos pelo Yahoo), 5 ambíguos/sem linha utilizável, 3 com PDF inválido do lado da CVM, 2
documentos resolvidos mas colapsados numa sugestão só (2 filings pro mesmo evento — dedup
funcionando). Teste `#[ignore]` novo (`live_check_cvm_dividend_notices_is_idempotent`) roda de
novo pra confirmar 0 reprocessamento contra o mesmo dado real. `cargo test --lib` **187/187 sem
regressão** (12 testes novos: 7 domínio puro em `domain::cvm_dividend_notice`, 5
`to_gemini_schema`), `tsc --noEmit` limpo, suíte do `easybusiness` **209/209** (5 testes novos de
`cvm_ipe.py`). Frontend: botão "Check CVM notices" em `DividendSuggestionsSection.tsx` (mesmo
padrão de auto-seleção de key/modelo do `AboutCompanySection.tsx`, some sem key de IA
configurada), coluna Source ganhou o rótulo "CVM".

**Objetivo**: dono do projeto quer automatizar o lançamento de proventos (hoje 100% manual, um `transaction_type = "provento"` por evento em `transactions`, sem campo de status). Quer tanto passados quanto futuros, com confirmação manual antes de virar lançamento de verdade — não confia (nem deveria) em gravação automática direto no ledger.

**Estado atual mapeado** (1 agente `Explore`, antes de desenhar): a única fonte de proventos hoje é o Yahoo Finance (`data-collector/sources/acoes_yahoo.py::fetch_dividend_payments`), que alimenta `stock_dividend_payments` — **só histórico já pago**, sem calendário futuro, sem distinção Dividendo/JCP, sem Data Com. `transactions` (`entity/transactions.rs`) não tem campo de status (a receber vs. pago). Não existe integração de corretora/B3 no código, só menções em rascunho de UI (Fase 13.2).

**Mecanismo desenhado pra parte "passado"** (viável hoje, sem fonte nova):
1. Pra cada evento de provento de uma fonte de dados (ticker, data de pagamento, valor/cota) sem lançamento correspondente ainda em `transactions`
2. Reconstruir a quantidade que o dono do projeto tinha do ativo somando compras/vendas com `transaction_date` até a data relevante
3. Calcular `quantidade × valor/cota` e gerar uma **sugestão** (não grava direto) — precisa de um lugar novo pra guardar sugestões pendentes, já que `transactions` não tem status; decisão em aberto se isso é uma tabela nova (`suggested_dividends` ou similar) ou um campo de status adicionado em `transactions`
4. Dono do projeto confirma, edita ou descarta cada sugestão numa tela de revisão

**Decisão confirmada nesta sessão**: na hora de confirmar a sugestão, o formulário tem um **campo manual pra Data Com** — por enquanto nenhuma fonte gratuita testada devolve isso de forma confiável o bastante pra preencher sozinho, então fica como input do usuário até (se) uma fonte melhor for validada. Reduz o risco do problema descrito abaixo (quantidade errada se houve compra/venda entre Data Com e Data Pagamento).

**Pesquisa de fontes pra "futuro" (parte que não dava pra fazer só com Yahoo)** — 3 buscas na web + 1 teste direto de API:
- **B3 direto**: não existe API pública documentada e acessível pra pessoa física; o consenso da busca é que acesso via API da B3 tende a ser caro/inviável pra projeto pessoal. Descartado por ora.
- **CVM Dados Abertos** (`dados.cvm.gov.br`, dataset `cia_aberta-doc-ipe` — Documentos Periódicos e Eventuais): tem metadados estruturados de Fato Relevante/Comunicado ao Mercado por categoria (inclusive dividendo/JCP), mas os valores e datas ficam dentro do documento anexado (PDF), não em campo estruturado — precisaria de extração de documento (reaproveitando a ideia já rascunhada em Fase 2.4/Finanças Gerais, mas pra nota de mercado, não extrato bancário). Fica como alternativa de fallback, mais trabalhosa.
- **brapi.dev — melhor achado**: API de ações brasileiras já mantida por terceiros. Testado ao vivo (`curl https://brapi.dev/api/quote/PETR4?dividends=true`, funciona sem token porque PETR4 é 1 dos 4 tickers de sandbox liberados) — o retorno tem **exatamente** os campos que faltavam: `paymentDate` (confirmado com datas futuras, ainda não pagas, no momento do teste), `rate` (valor/cota), **`label`** (`"DIVIDENDO"`/`"JCP"`/`"RENDIMENTO"` — resolve a distinção fiscal que o Yahoo não dá), `approvedOn` (data de aprovação em ata) e **`lastDatePrior`**, que pelo padrão dos valores retornados parece ser o equivalente à Data Com (data-limite de corte). **Não confirmado ainda** com tickers reais da carteira do dono do projeto (só os 4 de sandbox) nem com token pago de verdade — só uma amostra promissora.
- **Custo**: acesso a qualquer ticker fora do sandbox exige plano pago — mais barato é **Startup, R$ 99,99/mês, 150 mil requisições/mês**, atualização a cada 15min (site descreve o plano como cobrindo "cotações e dividendos do último ano" — precisa confirmar na prática se isso inclui os eventos futuros já aprovados que apareceram no teste de sandbox, ou se é uma limitação diferente).

**Decisão desta sessão**: **não assinar/implementar ainda** — dono do projeto quer fazer isso depois. Próximo passo combinado, quando essa conversa for retomada: criar uma conta de teste no brapi.dev (grátis, sem cartão) e confirmar com um token real que `paymentDate` futuro + `lastDatePrior` funcionam igual pros tickers reais da carteira, **antes** de decidir assinar o plano pago. Só depois disso desenhar onde a chave de API fica guardada (mesmo padrão de segredo que outras fontes do `data-collector` já usam, a verificar) e o schema da tabela/campo de sugestões pendentes.

**Implementado na Sessão 77 (2026-08-12) — parte "passado"** (o escopo viável sem fonte nova; o "futuro" via brapi.dev fica como combinado acima). Três decisões confirmadas via `AskUserQuestion` (escopo "passado agora, sem brapi"; tabela nova `suggested_dividends` em vez de campo de status em `transactions`; aba nova "Dividend suggestions" no Portfolio). Migration `m20260812_100000_create_suggested_dividends` (índice único `(portfolio_id, asset_id, payment_date)` → geração idempotente). Comandos em `commands/dividend_suggestion.rs` (`generate_dividend_suggestions` — cruza `stock_dividend_payments` com a posição reconstruída por `domain::dividend_suggestion::quantity_held_by` (função pura, compras−vendas até a data da fonte, 6 testes), pula provento já lançado manualmente na data, pula sugestão decidida, pula pagamento futuro e quantidade nula; `list_`/`confirm_` (grava o `provento` + marca `confirmed` atomicamente, com quantidade editável e **Data Com manual** — decisão da Sessão 76)/`discard_`). Frontend `DividendSuggestionsSection.tsx` + aba nova no `PortfolioPanel.tsx` (gerar → revisar → confirmar/editar/descartar). Aplicada no banco real de dev; `cargo test --lib` 136/136, `tsc --noEmit` limpo. **Não testado no app real ainda** (carteira atual não tem pagamento pós-compra — 0 sugestões; validado contra dados reais via simulação em Python; teste ao vivo fica pro dono do projeto, que pode adicionar uma compra retroativa pra ver a tela com dados).

**Continuação, Sessão 77.2 (2026-08-12, mesmo dia) — provento esperado manual + conciliação**: dono do projeto pediu pra poder **cadastrar um provento futuro manualmente** (ex.: empresa aprovou dividendo, ele anota antes do pagamento) e que, quando o automático puxar de verdade, **não lançasse provento novo** — apenas **concilie** o que ele já tinha cadastrado, desde que batam data de pagamento e valor "mais ou menos". 4 decisões via `AskUserQuestion`: (1) proventos esperados ficam na **mesma tabela** `suggested_dividends` com `source = "manual"` (não tabela nova); (2) valor bateu → marca a manual como **`matched`**, ainda exige o clique de Confirmar (princípio de nunca gravar direto no ledger se mantém); (3) valor divergiu → marca a manual como **`divergent`** e cria sugestão automática paralela pra comparar os dois; (4) tolerância **±10%** pro "mais ou menos". Migration `m20260812_110000_expected_dividends_and_source_index`: o índice único passou a incluir `source` (`(portfolio, asset, payment_date, source)`), senão a divergência não teria como coexistir manual+auto na mesma data. Novo comando `create_expected_dividend` (valida asset/portfolio, bloqueia duplicado manual na mesma data, não toca no ledger); `generate_` reescrito: antes de criar sugestão, checa se há manual pendente na data e concilia (`domain::dividend_suggestion::within_tolerance` — função pura com epsilon pra ponto flutuante, 4 testes novos). `confirm_`/`discard_` passaram a aceitar `pending`/`matched`/`divergent`. Frontend: botão "Register future dividend" (asset, data pagamento, valor/cota, quantidade, Data Com opcional, total calculado), coluna Source (Manual/Auto), status novos (Matched/Divergent), ações habilitadas pros 3 estados confirmáveis. Aplicada no banco real; `cargo test --lib` **140/140** (10 testes no domínio), `tsc --noEmit` limpo, schema da coexistência validado via SQLite (manual+auto na mesma data inserem sem colidir). **Não testado no app real ainda** — fluxo a validar: cadastrar provento esperado futuro → quando a fonte trouxer o pagamento, rodar "Generate suggestions" e ver virar `matched` (ou `divergent` + paralela).

### Fase 14 — Terceirizar `data-collector/` de vez pro EasyBusiness (ideia levantada na Sessão 89, planejada por completo, não iniciada)

> **Histórico (superada pela Fase 26, Sessão 107)**: a Fase 14 foi executada (Sessões 89 a 92) e
> depois desfeita no sentido inverso: o sidecar do Lume saiu e as fontes foram portadas pra Rust.

**Objetivo**: pedido explícito do dono do projeto — apagar `data-collector/` por completo (a
Fase 1.7 do EasyBusiness já tinha migrado a maior parte dele pra consumir a Finance API via
HTTP, mas deixou o processo Python local, setup manual, e 4 capacidades sem endpoint
equivalente). A versão free/self-hosted da Finance API deve rodar "já instalada" localmente
(sem Docker/Postgres pro usuário final) e a Settings deve ganhar um espaço pra apontar pra uma
futura instância Cloud paga do EasyBusiness (ainda não existe). Sem copiar código Python entre
os repos — reaproveitar centralizado no `easybusiness`, mesma diretriz que o `GUIDELINES.md`
de lá já tinha adotado independente deste pedido.

**Decisão de empacotamento** confirmada com o dono via `AskUserQuestion`: binário compilado
(PyInstaller), mesmo mecanismo que este repo já usa pro `data-collector` desde a Fase 11.3 —
não Docker Compose gerenciado pelo Anchor, não lib pip. Motivo: zero dependência de Docker pro
usuário final de um app desktop público, zero pasta Python neste repo, mecanismo já provado.

**Etapas** (desenho completo via 1 agente `Plan`, ver `easybusiness/project/PHASE.md` Fase 1.10
pro lado já implementado):
- [x] 14.0 — `easybusiness` Fase 1.10 (Sessão 89, implementada do lado de lá): modo SQLite
  "free/local" — sidecar compilável via PyInstaller, roda Alembic + uvicorn sozinho, porta
  OS-assigned anunciada via stdout. Pré-requisito de tudo abaixo. Nenhum código deste repo
  tocado ainda.
- [x] 14.1 — CI (Sessão 90, continuação): `.easybusiness-version` novo na raiz (SHA fixado,
  `3cc32c5` — Fase 1.10 do `easybusiness`, sem submodule) + 3 steps novos em `build.yml`, no
  mesmo job/matrix que já builda o `anchor-collector` (matrix ganhou `pyinstaller_data_sep`, `:`
  Linux/macOS e `;` Windows — sintaxe do `--add-data` do PyInstaller muda por SO): lê o SHA
  pinado, `actions/checkout@v4` do `easybusiness` nesse ref (repo público, sem token extra),
  `pyinstaller --onefile` contra `easybusiness/api/sidecar_main.py` com os `--add-data` de
  `migrations/`/`alembic.ini` (achado real da Fase 14.2 — sem isso o binário quebra em runtime).
  `tauri.conf.json::bundle.externalBin` já tinha `binaries/anchor-finance-api` registrado desde
  a 14.2, nada novo lá. Dev continua manual (`docker compose up` no `easybusiness`) — só o path
  de release ganha o sidecar. **Verificado**: sintaxe do YAML validada (`yaml.safe_load`), e a
  perna Linux reproduzida localmente linha por linha fora do Docker do Anchor — clone do
  `easybusiness` fixado exatamente no SHA pinado (`git worktree`, não o working tree local, que
  já tinha 1 commit a mais), `pyinstaller` com a interpolação exata do matrix, binário resultante
  rodado de ponta a ponta (migrations aplicaram, `/healthz` respondeu). Windows/macOS **não
  verificados de verdade** — `build.yml` nunca disparou neste repo (sem tag `v*` nenhuma ainda),
  só validação de sintaxe; só fecha de verdade quando alguém disparar o workflow real.
- [x] 14.2 — Rust (Sessão 90): `desktop/src-tauri/src/finance_api/` novo —
  `sidecar.rs` (`FinanceApiHandle`, `cfg!(debug_assertions)`: dev aponta fixo pro `docker
  compose up` manual do `easybusiness` — `http://localhost:8000`/`local-dev-key-change-me`,
  sem spawn nenhum, mesma convenção de `commands::collector::run_collector`; release spawna via
  `app.shell().sidecar("anchor-finance-api").spawn()`, API key aleatória em memória via `rand`,
  banco próprio `finance_api.db` em `app_data_dir()`, parseia `SIDECAR_PORT=` do stdout,
  healthcheck em `/healthz`, mata o processo no `RunEvent::ExitRequested` do `lib.rs`) +
  `client.rs` (22 métodos/DTOs `serde`, um por endpoint da Finance API — `ResponseMeta` com
  `#[serde(flatten)]` pros 4 campos comuns a toda resposta, `AppError::FinanceApiNotFound`/
  `FinanceApi` novos). Sem `#[tauri::command]` novo — client fica sem chamador real até a 14.4.
  **Verificado ao vivo, não só em teoria**: branch de dev testado contra uma Finance API real
  (`sidecar_main.py` do easybusiness rodando fora de Docker, já que o host tinha um problema de
  rede do Docker nesta sessão — módulo `veth` do kernel indisponível, bloqueando qualquer stack
  multi-container; branch de release não depende disso, já que roda 1 container só) — 6 testes
  `#[tokio::test] #[ignore]` novos em `client.rs` (`cargo test --lib -- --ignored finance_api`)
  cobrindo quote/price-history/roe/macro-series/fear-greed/404, todos passando contra chamadas
  de rede reais, sem mock. Branch de release testado de ponta a ponta também: binário
  PyInstaller do sidecar buildado dentro do mesmo container Docker do Anchor (build.yml vai
  fazer o mesmo na Fase 14.1) e registrado em `tauri.conf.json::bundle.externalBin`, app rodado
  em `cargo build --release` de verdade — sidecar spawnou, passou no healthcheck, app abriu
  janela real, fechar a janela matou o processo filho (confirmado via `ps` dentro do container e
  ausência de processo órfão no host). `cargo test --lib` **173/173 sem regressão** (+ 6
  ignorados).
  **2 achados reais, fora do escopo deste repo, corrigidos no `easybusiness`** (aprovados pelo
  dono do projeto via `AskUserQuestion`): (1) o binário PyInstaller da Fase 1.10 de lá não
  empacotava `migrations/`/`alembic.ini` (lidos do disco em runtime, não via `import` — o
  PyInstaller não segue essa dependência sozinho), quebrando com `Path doesn't exist:
  .../migrations` assim que tentava migrar o banco — corrigido documentando o comando
  `--add-data` correto no docstring do `sidecar_main.py` de lá, testado com o fix aplicado; (2)
  um binário buildado num host com GLIBC mais novo que o container Debian onde o app realmente
  roda falha silenciosamente (`GLIBC_2.38' not found`) — não é bug de código, é lembrete pra
  Fase 14.1: o CI precisa buildar o sidecar dentro do mesmo ambiente onde o app roda (ou um
  runner com GLIBC igual ou mais velho), não em qualquer host.
- [x] 14.3 — Settings (Sessão 90, continuação 2): tabela `finance_api_settings` (linha única,
  padrão "replace" de `commands::home_layout` — `mode`: `"local"`/`"remote"`, `remote_url`
  nullable, `updated_at`), migration `m20260829_141241_create_finance_api_settings`. Chave
  remota no keyring sob username fixo `"finance_api_remote"` (`commands::api_key::
  KEYRING_SERVICE` reaproveitado, não o esquema `"{provider}:{id}"` de `ai_api_key` — schema
  errado pro caso). `commands::finance_api_settings::{get_,set_}finance_api_settings` (em
  branco no campo de chave mantém a já salva, não força redigitar toda vez que só a URL muda).
  Seção nova "Finance API" em `SettingsPage.tsx` (`SECTIONS` ganha um 3º item, arquivo próprio
  `FinanceApiSettingsSection.tsx`, mesmo padrão de `TruthIdSettingsSection.tsx`). **Escopo
  fechado com o dono do projeto via `AskUserQuestion`**: só armazenamento + UI, sem ligar em
  `finance_api::sidecar::init` (14.2) — a configuração fica salva mas sem efeito real até uma
  fatia futura consumir, fiel ao desenho original da Sessão 89. **Achado real durante a
  implementação**: `sea-orm-cli generate entity` sem flag nenhuma regenerou 5 entities
  existentes (`bank_account`/`general_transaction`/`general_transaction_category`/`liability`/
  `liability_installment`) com nome de arquivo/módulo **plural**, batendo com o nome real da
  tabela SQL mas divergindo do nome singular hand-editado que já estava commitado — teria
  quebrado a compilação de `commands::bank_account.rs` e companhia se não fosse revertido antes
  do `cargo check`. Corrigido restaurando os arquivos afetados via `git checkout` e inserindo só
  as 2 linhas novas (`mod.rs`/`prelude.rs`) à mão, sem rodar `generate entity` de novo — mesmo
  cuidado que `macro_index_monthly.rs` já documentava ("hand-written to match the generated
  style") por esse motivo exato. **Verificado**: `cargo check`/`cargo test --lib` **173/173 sem
  regressão**, `tsc --noEmit` limpo, migration aplicada e entity gerada contra o banco real de
  dev. Testado ao vivo interrompido antes de confirmar a seção na tela — um clique automatizado
  (`xdotool`) calculado errado, no ambiente de X11 real do dono do projeto (não uma tela de
  teste isolada), acabou clicando na aba do navegador atrás da janela do Anchor por engano;
  parado ali por segurança (achado registrado em memória de sessão — `xdotool` neste projeto
  toca o desktop real, não uma tela isolada). **Confirmação visual da seção fica pro dono do
  projeto**, mesma política já adotada pra cliques em componentes Radix/WebKitGTK.
- [x] 14.4 — Rust: portar a lógica de fetch+write do `data-collector/main.py` pra dentro do
  Tauri — um módulo por classe de ativo (`finance_api/stocks.rs`, `crypto.rs`, `us_stocks.rs`,
  `price_history.rs`, `benchmarks.rs`, `fii.rs`), cada `#[tauri::command]` existente em
  `collector.rs`/`fii.rs` troca o corpo (chamar `run_collector` → chamar `finance_api::*`
  direto) sem mudar assinatura, então o frontend não muda. **Maior fatia do trabalho**, só fecha
  de verdade depois que a Fase 1.11 do EasyBusiness cobrir as 4 capacidades que hoje só existem
  local — provável 2-4 sessões próprias.
  - [x] **Ação BR/FII/ETF-BR/BDR** (Sessão 91, dono do projeto pediu pra deixar rodando sem
    interação): `finance_api/stocks.rs` novo — espelha `collect_stock_{quotes,dividends_avg,
    technicals,dividend_payments,price_history,fundamentals,dcf_fundamentals}` do coletor
    Python linha a linha, mesma sequência e mesmas regras de escrita (`stock_quotes`/
    `stock_dividends_avg`/`stock_technicals`/`stock_fundamentals`/`stock_dcf_fundamentals`
    sempre inserem linha nova; `stock_dividend_payments`/`stock_price_history` usam `ON
    CONFLICT DO NOTHING` no índice único `(ticker, data)`, via `sea_orm::sea_query::OnConflict`
    — `DbErr::RecordNotInserted` normalizado pra sucesso quando todo o lote já existia).
    `finance_api::skip_not_found` novo (helper genérico, `finance_api/mod.rs`) reproduz o
    contrato "404 pula o ticker, qualquer outro erro propaga" que `finance_api_client.py` já
    tinha. `commands::collector::run_stock_collector` (branch padrão `_`, sem `asset_class`
    especial) e `run_price_history_backfill` trocaram de `run_collector` (subprocess Python)
    pra chamar `finance_api::stocks::*` direto, mesma assinatura — frontend intocado. Os
    outros branches (cripto/metal/ação americana/REIT/ETF-US) continuam no coletor Python.
    **Achado real durante a implementação**: `sea-orm-cli generate entity` sem cuidado quase
    quebrou 5 entities existentes (ver nota da 14.3 acima) — desta vez as novas escritas usaram
    as entities já existentes (`stock_quotes` etc.) sem precisar gerar nada novo, então não
    reincidiu. **Verificado ao vivo**: 4 testes `#[tokio::test] #[ignore]` novos em
    `finance_api/stocks.rs` (`cargo test --lib -- --ignored finance_api::stocks`) — quote,
    técnicos+dividendos médios e 2 testes de idempotência (rodar 2x não duplica proventos nem
    preço histórico), todos contra a Finance API real e o banco real de dev, sem mock.
    `cargo test --lib` **173/173 sem regressão** (+10 ignorados no total), `tsc --noEmit`
    limpo. **Não coberto ao vivo**: o passo de fundamentos/DCF (`collect_fundamentals`/
    `collect_dcf_fundamentals`, via bolsai+CVM) precisa de uma `BOLSAI_API_KEY` real (serviço
    pago de terceiro) — o classificador de permissão do modo auto bloqueou até um `ls` no
    `.env` que guarda essa chave (corretamente, é onde o segredo mora), então esse trecho ficou
    só revisado por código, replicando a lógica do Python (roe da CVM sempre sobrescreve o da
    bolsai, ticker sem ROE é descartado inteiro) sem confirmação ao vivo — pendência explícita
    pra quando alguém puder rodar com a chave de verdade.
  - [x] **Cripto** (Sessão 91, mesma continuação): `finance_api/crypto.rs` novo —
    `collect_ticker` (quote+histórico de preço de qualquer moeda + Fear & Greed global, mesmo
    `market_chart` da Finance API que já dava os dois do lado Python) e
    `collect_eth_indicators` (os 4 indicadores do ciclo ETH, `coin` sempre `"ETH"`). Reaproveita
    `domain::crypto_score::classify` já existente (não duplicou a lógica de sinal verde/
    amarelo/vermelho). `insert_ignoring_conflicts` (helper genérico do `ON CONFLICT DO NOTHING`,
    movido de `stocks.rs` pra `finance_api/mod.rs` pra ser reaproveitado pelos dois módulos).
    `run_stock_collector` (branch `Some("cripto")`) e `run_crypto_collector` trocaram de
    `run_collector` pra `finance_api::crypto::*` direto. **Verificado ao vivo**: 2 testes
    `#[ignore]` novos (quote+histórico real de BTC, 4 indicadores ETH com sinal válido),
    `cargo test --lib` **173/173 sem regressão** (+12 ignorados no total), `tsc --noEmit`
    limpo.
  - [x] **Metal** (Sessão 91, mesma continuação): `finance_api/metals.rs` novo —
    `collect_ticker`, mesmo papel de `crypto::collect_ticker` sem o Fear & Greed. Grava com o
    ticker original (maiúsculo, convenção do frontend), não o `metal_code` minúsculo que
    `client::fetch_metal_quote` usa só na URL. `run_stock_collector` (branch `Some("metal")`)
    trocou pra `finance_api::metals::collect_ticker` direto. **Verificado ao vivo**: 1 teste
    `#[ignore]` novo (XAU real), `cargo test --lib` **173/173 sem regressão** (+13 ignorados),
    `tsc --noEmit` limpo.
  - [x] **FII CVM** (Sessão 91, mesma continuação): `finance_api/fii.rs` novo —
    `collect_cvm_data` (indicadores mensais + imóveis, `ON CONFLICT DO NOTHING` nos índices
    únicos `(cnpj, reference_date)`/`(cnpj, reference_date, nome_imovel)`). `resolve_fii_cnpj`
    (bolsai+CVM pra sugerir CNPJ) **não** portado — uma das 4 capacidades sem endpoint
    equivalente na Finance API ainda, continua no coletor Python. `commands::fii::
    run_fii_cvm_collector` trocou pra `finance_api::fii::collect_cvm_data` direto.
    **Bug real achado e corrigido**: a Finance API exige o CNPJ só com dígitos na URL
    (`11728688000147`, 200) — `assets.cnpj`/`fii_cnpj_cache` (e o resto do app,
    `list_fii_cvm_monthly`/`list_fii_cvm_properties` incluídos) sempre guardou formatado
    (`11.728.688/0001-47`, 404 se mandado direto). O `data-collector/sources/
    finance_api_client.py` (Fase 1.7, Sessão 88) **nunca normalizava isso** — bug real já em
    produção, só não pego antes porque o teste ao vivo da Sessão 88 usou um CNPJ já em formato
    de dígitos por fora do fluxo real do app. Corrigido no port: normaliza (só dígitos) pra
    chamada da API, grava no banco com o `cnpj` original recebido (mesma decisão de
    `finance_api::metals` com o ticker) — sem isso, `list_fii_cvm_monthly`/
    `list_fii_cvm_properties` (que filtram pelo `cnpj` formatado de `assets.cnpj`) nunca
    achariam as linhas gravadas. **Verificado ao vivo**: 1 teste `#[ignore]` novo (HGLG11 real,
    CNPJ formatado do banco de dev), `cargo test --lib` **173/173 sem regressão** (+14
    ignorados), `tsc --noEmit` limpo.
  - **Estado da 14.4 ao final da Sessão 91**: todas as classes/fluxos sem dependência das 4
    capacidades locais (ver Sessão 88/89) já portadas — Ação BR/FII/ETF-BR/BDR, Cripto, Metal,
    FII CVM. **Bloqueado de propósito, aguardando a Fase 1.11 do easybusiness**: ação
    americana/REIT/ETF-US (`main_us_stock`/`main_reit`/`main_etf_us` misturam, numa função só, o
    trecho local Yahoo sem sufixo com o trecho já coberto pela Finance API — dividir os dois
    dentro de **um** comando/invocação exigiria chamar Python e Rust juntos, redundante e
    frágil; melhor esperar o gap fechar e portar o fluxo inteiro de uma vez), benchmarks
    (`--benchmark-returns` mistura CDI/IPCA/IFIX/SMLL/IDIV/IVVB11, já portáveis, com o IBOV
    local Yahoo sem sufixo — mesmo problema de entrelaçamento) e `resolve_fii_cnpj` (bolsai+CVM
    pra sugerir CNPJ, sem endpoint equivalente). `data-collector/` continua necessário até esses
    3 fluxos fecharem — 14.5 (limpeza) segue bloqueada.
  - **Sessão 92 (mesmo dia, trabalho do lado `easybusiness`)**: Fase 1.11 do `easybusiness`
    fechada por completo — as 4 capacidades acima ganharam endpoint (`/v1/us-stocks/{ticker}/
    quote,technicals,dividends-avg,price-history,dividend-payments,reit-fundamentals` e
    `/v1/fiis/resolve/{ticker}`), verificadas ao vivo e commitadas lá (ver `SESSIONS.md`/
    `PHASE.md`/`ROADMAP.md` do `easybusiness` pros detalhes — nenhum código deste repo mudou).
    **Bloqueio removido**: os 3 fluxos acima ficam livres pra portar pra Rust, mesmo padrão das
    4 fatias já feitas na Sessão 91 — trabalho ainda não iniciado neste repo.
  - [x] **Ação americana/REIT/ETF-US, benchmarks e `resolve_fii_cnpj`** (Sessão 92, continuação
    — fecha a 14.4): `finance_api/us_stock.rs` novo (5 recursos Yahoo sem sufixo, escrevendo nas
    **mesmas tabelas** que Ação BR já usa — `stock_quotes` etc., não existe `us_stock_*` do lado
    Rust, mesma decisão do Python original; `run_us_stock_collector` acrescenta fundamentos/DCF
    via `client::fetch_us_stock_fundamentals/payout/dcf_fundamentals`, já existentes desde a
    Fase 1.7 mas sem chamador até agora; `run_etf_us_collector` é só os 5 recursos, sem
    fundamentos). `finance_api/reit.rs` novo (reaproveita os 5 de `us_stock` + indicadores
    imobiliários via `client::fetch_reit_fundamentals`). `finance_api/benchmark.rs` novo (CDI/
    IPCA com upsert real; IBOV via `us_stock::collect_price_history("^BVSP")`, sem tratamento
    especial; IVVB11/IFIX/SMLL/IDIV já portáveis desde a Fase 1.7). `finance_api::fii::
    resolve_cnpj` novo (cache-then-fetch via `client::fetch_fii_cnpj_resolution`, cacheando só
    quando resolve de verdade — mesma disciplina do original, nunca cacheia ausência).
    `commands::collector::run_stock_collector` (branches `acao_internacional`/`reit`/`etf_us`)
    e `run_benchmark_backfill` trocaram de `run_collector` pra chamar `finance_api::*` direto;
    `commands::fii::resolve_fii_cnpj` idem, perdendo os parâmetros `app`/`lock` (a chamada HTTP
    não precisa do lock de subprocess — frontend só passa `{ ticker }`, confirmado antes de
    tirar). **3 achados reais ao vivo, todos corrigidos antes de fechar**: (1)
    `reit_fundamentals` nunca teve índice único — a Finance API agora devolve a série histórica
    completa a cada chamada (Fase 1.11.2), então sem índice o `ON CONFLICT` nem compilava
    ("does not match any PRIMARY KEY or UNIQUE constraint") e, se removido, duplicaria cada ano
    já existente a cada re-run; migration nova cria o índice `(ticker, reference_year)`,
    deduplicando antes 2 linhas que já existiam no banco de dev de testes manuais anteriores à
    Fase 14; (2) a Finance API devolve o CNPJ resolvido só com dígitos (normalizado do lado dela
    pra caber na própria coluna), mas o resto do Anchor sempre usa o formato pontuado —
    `finance_api::fii::format_cnpj` novo, achado testando KNRI11 real; (3)
    `commands::collector::run_collector` (subprocess Python) ficou sem nenhum chamador depois
    que `resolve_fii_cnpj` (seu último uso) portou pra Rust — removida (o binário/script Python
    em si continua até a 14.5 apagar `data-collector/` de vez). **Verificado ao vivo**: AAPL (5
    endpoints + fundamentos/DCF), `^BVSP`/IBOV (2483 pregões via o mesmo mecanismo sem sufixo),
    IVV (ETF-US), Realty Income + Simon Property (REIT, incluindo idempotência pós-migration e o
    fallback `NetIncomeLoss`→`ProfitLoss`), os 7 benchmarks completos com upsert de CDI/IPCA sem
    duplicar, HGLG11 (cache hit, sem chamada de rede) e KNRI11 (cache miss, formato corrigido).
    `cargo test --lib` **175/175 sem regressão** (+13 testes `#[ignore]` novos, todos passando),
    `tsc --noEmit` limpo. **Fase 14.4 completa** — `data-collector/` não tem mais nenhum fluxo
    exclusivo, só falta a 14.5 apagá-lo.
- [x] 14.5 — Limpeza: apagar `data-collector/` inteiro (script, `sources/`, `.venv`, spec do
  PyInstaller, `.env*`, artefatos de banco soltos), remover o step de build Python velho do
  `build.yml` e a entrada `anchor-collector` do `externalBin`, atualizar
  `desktop/docker-compose.yml` (bind mount) e este `README.md`.
  - **Sessão 92 (mesma sessão, continuação — fecha a Fase 14)**: `data-collector/` apagado por
    completo — `main.py`, `sources/` (5 arquivos), `config.yaml`, `.env.example`, `README.md`,
    `requirements*.txt` (`git rm -r`) e `.venv/`/`build/`/`dist/`/`__pycache__/`/`.cache/`/
    `*.spec` (`rm -rf`, não versionados). **Achado antes de apagar**: `data-collector/anchor.db`
    é o banco SQLite real de dev (não um artefato Python) — morava ali só porque Rust e Python
    compartilhavam container (decisão original, `project/ARCHITECTURE.md`); movido pra `data/`
    (diretório novo, irmão de `desktop/`/`contracts/`), a alternativa que a própria
    `ARCHITECTURE.md` já cogitava desde a Sessão 1. Junto foram preservados (não descartados sem
    checar) um `.env` real com `BOLSAI_API_KEY`/`SEC_EDGAR_CONTACT_EMAIL`/`ETHERSCAN_API_KEY`
    (renomeado `.env.data-collector-bak`, mesmos segredos já duplicados no `.env` do
    `easybusiness`) e um backup antigo `practice_valuation.db.bak` (nome pré-rebranding, Sessão
    65) — ambos sem chamador de código, preservados por precaução, não descartados por
    suposição. **Achado real de segurança durante a limpeza**: o `.env` renomeado e o backup do
    banco desta sessão (`anchor.db.bak-sessao92`) não batiam em nenhum padrão do `.gitignore`
    (só `.env`/`.env.*`/`*.db.bak` exatos) — corrigido renomeando pra `.env.data-collector-bak`/
    `anchor-sessao92.db.bak`, confirmado com `git check-ignore` antes de seguir, pra não arriscar
    commitar segredo/binário grande sem querer. `desktop/src-tauri/src/db.rs::
    DEV_DATABASE_FILE_PATH` e os 7 `dev_db()` de teste (`finance_api::{stocks,crypto,metals,
    fii,reit,benchmark,us_stock}`) trocaram pra `/data/anchor.db`.
    `desktop/docker-compose.yml` trocou o bind mount `../data-collector:/data-collector` por
    `../data:/data`; `desktop/Dockerfile` perdeu o bloco `apt-get install python3` (nada mais no
    container precisa de Python — a Finance API roda fora do container em dev, sem processo
    spawnado do lado do Anchor); `tauri.conf.json::externalBin` perdeu `"binaries/anchor-
    collector"`; `.github/workflows/build.yml` perdeu o step "Build Python sidecar" mas manteve
    o step "Python" (`actions/setup-python@v5`, ainda usado pelo sidecar da Finance API logo
    depois); `.gitignore` perdeu as 2 entradas `data-collector/*`. `README.md` trocou a linha
    "Data collector" da tabela de Architecture e o parágrafo de "subprocess sem rede" por uma
    menção ao sidecar HTTP da Finance API (`easybusiness`). **Verificado ao vivo**: container
    reconstruído do zero (`docker compose up -d --build`, Dockerfile sem Python compila e sobe
    normal), `/data` montado e `/data-collector` confirmadamente ausente dentro do container,
    `cargo test --lib` **175/175 sem regressão**, e pelo menos 1 teste `#[ignore]` de cada um dos
    7 módulos rodado contra a Finance API real + `/data/anchor.db` no caminho novo (11 testes no
    total, todos passando — confirma que o caminho resolve de verdade, não só que compila),
    `tsc --noEmit` limpo. **Fase 14 fechada por completo** — nenhum resquício de
    `data-collector/` sobra no repo.

Sequência: 14.1/14.2/14.3 podem andar em paralelo entre si (todas dependem só da 14.0, já
pronta); 14.4 era a maior e só fechou de verdade depois da Fase 1.11 do EasyBusiness (Sessão 92,
independente de 14.1-14.3); 14.5 fechou na mesma sessão logo em seguida. **Fase 14 completa.**

### Fase 15 — Storage Provider seletivo (desacopla anexos/documentos do Vault Web3, ideia trazida via spec na Sessão 95, implementada na Sessão 97 — LocalFSProvider único real)

**Objetivo**: hoje o plano é que anexos e documentos financeiros (extratos bancários,
comprovantes, notas fiscais, fotos/documentos de imóveis e empresas não listadas) fiquem
guardados só no Vault descentralizado Web3 (Fase 8, ainda `[ ]`) — e isso é majoritariamente
ideia, não código ainda. Antes de avançar essa implementação, desacoplar o storage do Web3 como
dependência obrigatória, suportando também opções locais e hospedadas, sem tratar Web3 como a
única opção possível. **Escopo explícito: só desenhar o contrato/arquitetura agora — não
implementar billing/infra do plano pago hospedado.**

**Princípio de design**: o core do app nunca fala direto com a stack Web3, só com duas
interfaces abstratas — `StorageProvider` (onde arquivos são lidos/escritos) e `AuthProvider`
(como o usuário se identifica e, no futuro, paga por um tier de storage). A integração Web3 já
planejada na Fase 8 vira **uma implementação** dessas interfaces (`DecentralizedVaultProvider`),
registrada como plugin — nunca hard-coded no core.

```
interface StorageProvider {
  read(path: string): Promise<Blob | string>
  write(path: string, content: Blob | string): Promise<void>
  list(prefix?: string): Promise<string[]>
  delete(path: string): Promise<void>
  exportAll(): Promise<Map<string, Blob | string>>   // necessário pro fluxo de migração
  importAll(files: Map<string, Blob | string>): Promise<void>
}

interface AuthProvider {
  getUserId(): string | null
  isSubscriptionActive(): Promise<boolean>  // só relevante pra providers pagos, futuro
  login(): Promise<void>
  logout(): Promise<void>
}
```

O resto do app (transações, anexos de compra/venda, documentos de imóvel/empresa, extratos
importados) só chama esses métodos, nunca sabe qual provider está por trás.

**As 4 implementações propostas**:

| Provider | Custo | Onde roda | Depende de Web3? |
|---|---|---|---|
| `LocalFSProvider` | Grátis | Disco da máquina atual | Não — vira o default |
| `SelfHostedProvider` | Grátis | Servidor próprio do usuário (endereço configurável) | Não |
| `ManagedCloudProvider` | Pago (futuro) | Infra hospedada pelo dono do projeto | Não — **não implementar agora**, só reservar o contrato |
| `DecentralizedVaultProvider` | Pago (assinatura, futuro) | Vault Web3 da Fase 8 | Sim — único provider que depende da stack Web3 |

Config nova: `storage_provider: "local" \| "self_hosted" \| "managed_cloud" \| "decentralized_vault"`,
resolvida em runtime por uma factory. `managed_cloud` precisa existir como valor válido no enum
e na UI (desabilitado/"em breve") mesmo sem implementação, pra ativar no futuro sem migração de
schema. UI de configuração deve explicar em termos simples o que cada opção significa (quem tem
acesso aos dados, precisa de servidor próprio ligado, custo), não só um dropdown técnico.

**Fluxo de migração entre providers** (troca de provider com documentos já salvos): confirmar
com o usuário → `oldProvider.exportAll()` → `newProvider.importAll(dados)` → validar integridade
(contagem/tamanho/checksum) → só então liberar a troca definitiva do `storage_provider` no
config. Vale desenhar o contrato `exportAll`/`importAll` desde já mesmo que só `local` e
`decentralized_vault` fiquem prontos no curto prazo, pra não quebrar contrato quando
`managed_cloud` for ativado depois.

**Escopo sugerido quando isto for implementado**: `LocalFSProvider` (default) +
`DecentralizedVaultProvider` (integra com o que a Fase 8 for entregando) primeiro;
`ManagedCloudProvider` fica só reservado (enum + UI desabilitada); `SelfHostedProvider` avaliar
se entra junto ou depois, dependendo do esforço de rede que exigir. Fora de escopo mesmo depois
de implementado: modelo de billing do `ManagedCloudProvider`, implementação de
rede/descoberta do `SelfHostedProvider`, UI final das telas de configuração (só o contrato de
dados que ela precisa respeitar).

**Estado**: **implementada na Sessão 97 (2026-09-13)** — `StorageProvider` (trait Rust,
`desktop/src-tauri/src/storage/mod.rs`) + `LocalFSProvider` (`storage/local_fs.rs`, único provider
real) + `StorageProviderKind` (enum hand-rolled no estilo `domain::chat_provider`, reservando
`self_hosted`/`managed_cloud`/`decentralized_vault` sem lógica). `resolve_active_provider` lê a
tabela nova `storage_settings` (linha única, mesmo padrão "replace" de `finance_api_settings`,
Fase 14.3) e por ora só sabe construir `LocalFSProvider` — `set_storage_settings` recusa
qualquer outro valor até ganhar implementação real. `AuthProvider` ficou de fora do escopo desta
fatia (só relevante pra providers pagos, nenhum existe ainda).

**Retrofit dos anexos existentes** (decisão do dono do projeto via `AskUserQuestion`, escolhida
sobre deixar o `StorageProvider` sem consumidor até a Fase 12): `commands/property.rs` e
`commands/thesis.rs` (Fase 10 item 8 e Fase 10.5) trocaram os `std::fs::*` diretos por chamadas
ao provider ativo — `add_{asset,thesis}_attachment` lê os bytes do `source_path` (caminho do OS
file picker) e chama `provider.write()`; `delete_{asset,thesis}_attachment` chama
`provider.delete()`; `delete_thesis` (que apagava a pasta inteira) chama
`provider.delete_prefix()`; `get_{asset,thesis}_attachment_path` chama `provider.local_path()`
(usado pelo frontend via `convertFileSrc`). `stored_relative_path` manteve o mesmo formato
(`"asset_attachments/{id}/{arquivo}"`) — zero migração de dado nas linhas já existentes, zero
mudança de contrato pro frontend (`ManualAssetDetails.tsx`/`ThesisSection.tsx` inalterados).

**UI**: seção nova "Storage" em Settings (`StorageSettingsSection.tsx`, mesmo esqueleto de
`FinanceApiSettingsSection.tsx`) — `<Select>` de 4 opções, as 3 não implementadas desabilitadas
com "(coming soon)" e uma linha explicando em termos simples o que cada provider significa (quem
tem acesso ao dado, precisa de servidor próprio ligado, custo), conforme pedido explícito da spec.

**Verificação**: `cargo test --lib` 195/195 sem regressão (8 testes novos — 3 round-trip
parse/reject de `StorageProviderKind`, 5 de `LocalFSProvider` cobrindo
write/read/delete/delete_prefix/export_all+import_all), `tsc --noEmit` limpo. **Teste ao vivo da
UI (app completo via WebKitGTK) pulado nesta sessão** — decisão explícita do dono do projeto via
`AskUserQuestion`: os builds Rust já tinham disparado o watchdog de memória da máquina várias
vezes (precisou `-j 1` e recuperar containers órfãos que sobreviviam a processos mortos), e o app
completo seria mais pesado ainda. **Pendência registrada**: confirmar manualmente (criar
asset/anexo/preview/apagar, mesmo fluxo pra tese, conferir a seção Storage) numa sessão futura ou
diretamente pelo dono do projeto.

**Segue reservado, sem implementação real**: `SelfHostedProvider` (avaliar quando/se entra),
`ManagedCloudProvider` (billing/infra do plano pago, fora de escopo mesmo depois), fluxo de
migração entre providers (`exportAll`/`importAll` — o contrato existe na trait e está testado
via `LocalFSProvider`, mas sem UI/comando de migração, já que não há um segundo provider real pra
migrar pra ele ainda), `DecentralizedVaultProvider` (depende da Fase 8/Vault Web3 avançar).

### Fase 16 — Backup do Workspace (ideia trazida em conversa pelo dono do projeto na Sessão 96, implementada na Sessão 99)

**Bug corrigido na Sessão 106 (achado por teste automatizado)**: o restore voltava **incompleto
e sem erro**. O sqlx liga as FKs do SQLite por padrão e o `import_snapshot` apagava e inseria
tabela por tabela, em ordem alfabética; o `DELETE` de uma tabela "mãe" que vinha depois
(`workspace`) cascateava (`ON DELETE CASCADE` age na hora, `defer_foreign_keys` só adia a checagem
de violação) sobre filhas já restauradas (`bank_accounts`, `borrowers`, `loans`...). Agora apaga
**todas** as tabelas antes de inserir qualquer uma. Coberto por
`flow_tests::backup_round_trips_the_real_schema_with_data_and_attachments` (exporta o schema real
com dados e anexo, passa pelo JSON, restaura num banco com lixo e compara **tabela por tabela**).
O teste de clique manual desta fase nunca tinha sido confirmado, e teria perdido dados.

**Objetivo**: mecanismo explícito de backup/restauração de um Workspace inteiro (dados +
anexos, ver Fase 15), distinto do sync contínuo multi-dispositivo já desenhado na Fase 8 — cobre
o caso de o usuário simplesmente querer um snapshot restaurável (proteção contra perda de
dado/corrupção/troca de máquina), não a sincronização em tempo real entre dispositivos.

**Decisões de escopo (Sessão 99, `AskUserQuestion`)**: export **estruturado (JSON)**, não cópia
bruta do `.db` — mas dado que o schema tem 55 entidades (`entity/`), em vez de código específico
por tabela, o export é **genérico via introspecção SQL** (`sqlite_master` + `PRAGMA
table_info`), cobrindo qualquer tabela presente ou futura sem precisar ser tocado de novo.
Escopo banco + anexos (reaproveita `StorageProvider::export_all`/`import_all` da Fase 15).
Gatilho manual (botão em Settings, sem scheduler). Destino só local (file picker do SO).

**Implementado**: `desktop/src-tauri/src/backup.rs` (engine, sem depender de Tauri) — lista
tabelas via `sqlite_master`, lê tipo/afinidade de cada coluna via `PRAGMA table_info` (afinidade
decide como extrair/reconstituir cada valor: Integer/Real/Text/Blob — cobre tudo que os
migrations deste projeto realmente geram, incluindo o caso `boolean` caindo no fallback NUMERIC
do SQLite, sempre 0/1). Formato do arquivo: `{format_version, schema_version, created_at,
tables: {nome: [linhas...]}, attachments: [(path, base64)]}`. **Restore roda numa única
transação com `PRAGMA defer_foreign_keys = ON`** (diferente de `PRAGMA foreign_keys`, que não
pode mudar dentro de uma transação já aberta) — apaga+insere todas as tabelas em qualquer ordem
e o `commit()` falha sozinho (rollback automático) se alguma FK ficar quebrada no fim, sem
precisar de checagem manual. **Guard de segurança**: restore recusa rodar se o `schema_version`
do arquivo (última migration aplicada no banco de origem) não bater com o banco atual — evita
restaurar um backup de um schema incompatível, já que o projeto não tem nenhuma ferramenta de
diff/migração parcial. Comandos novos `export_workspace_backup`/`import_workspace_backup`
(`commands/backup.rs`), resolvendo o provider ativo do jeito que `commands/property.rs` já
fazia. UI: seção nova "Backup" em Settings (`BackupSettingsSection.tsx`) — botão de export (file
picker `save()`) e de import (`open()` + confirmação explícita "Confirm — overwrite all data?"
antes de rodar, mesmo padrão "Confirm?" de `IaSettingsSection.tsx`).

**Verificado**: `cargo test --lib` 203/203 sem regressão (3 testes novos — round-trip
preservando null/tipos e respeitando FK entre duas tabelas ad-hoc, guard de `format_version`
desconhecido, guard de `schema_version` incompatível). `tsc --noEmit` limpo. App relançado ao
vivo (rebuild automático do `npm run tauri dev`) sem crash, mas **teste de clique real (export →
import contra um Workspace de verdade) ainda não foi feito** — fica pendência pro dono do
projeto confirmar, mesmo padrão da Fase 15.

**Limitação conhecida, aceita conscientemente**: `import_all` do `StorageProvider` só
sobrescreve por caminho, não apaga arquivo que não está no backup — um restore nunca *perde* um
anexo mais novo que o usuário criou depois do backup, mas também não limpa anexos órfãos. Teste
"contra o schema real com todas as migrations aplicadas" (cogitado no plano) foi descartado por
exigir uma dependência dev nova (crate `migration`, hoje só usado via `sea-orm-cli` manual, fora
do grafo de dependências do crate principal) — os testes unitários com tabelas ad-hoc já cobrem
a lógica de tipo/FK/versão sem essa complexidade extra.

### Fase 17 — Índices macro adicionais via EasyBusiness: SELIC, DI Futuro (ideia trazida em conversa pelo dono do projeto na Sessão 96, puro registro, não iniciada)

**Objetivo**: trazer mais índices de referência pro painel de mercado/benchmarks (Fase 10.4/13.5)
— **SELIC** (meta definida pelo Copom, distinta do CDI que já é coletado via BCB SGS desde a Fase
13.5) e **DI Futuro** (taxa de juros futura negociada na B3), entre outros que o dono do projeto
for pedindo. Dependência de ordem: a coleta desses índices ainda não existe do lado do
`easybusiness` (mesmo padrão de dependência cross-repo da Fase 14) — o Anchor só consome depois
que o endpoint existir lá, sem repetir a lógica de coleta localmente.

**Estado**: puro registro de intenção — nenhum índice novo implementado, nenhuma decisão sobre
quais fontes usar do lado do `easybusiness`. Bloqueada até o `easybusiness` ganhar suporte pra
esses índices.

**Desbloqueada (constatado na Sessão 101, confirmado pelo dono do projeto)**: o Lume (ex-
easybusiness) já tem `/v1/rates/selic/{series_code}` (BCB SGS) e `/v1/rates/di-futures-curve/
{reference_date}` (B3) — `api/app/routers/rates.py` de lá. Falta só o lado do Anchor (client +
painel/benchmarks). Candidata natural à próxima fase.

**Implementada na Sessão 101 (2026-10-14)**, em plan mode. Decisões do dono do projeto
(AskUserQuestion): aparece **nos dois lugares** (widget na Home + tela em Research), **Selic
também como benchmark** na Rentabilidade (contra minha recomendação — anda colada no CDI), DI
Futuro como **curva + prazos-chave**.
- [x] `domain/rates.rs` puro: `selic_monthly_returns` (Meta Selic diária do BCB 432, que vem em
  dias corridos, capitalizada `(1+meta)^(1/252)` por **dia útil** — reaproveita o calendário
  bancário da Fase 24 — com a meta vigente em cada dia), `selic_changes` (decisões do Copom),
  `rate_at_years` (interpolação linear por dias úteis, sem extrapolar).
- [x] Benchmark: `collect_benchmarks` grava `index_code = "selic"` em `macro_index_monthly` (mesmo
  caminho de CDI/IPCA; helper `upsert_monthly` extraído); linha "Selic" no gráfico "Return vs.
  benchmarks" no mesmo laranja do grupo juros (CDI/IPCA) com traço próprio — sem 4º matiz,
  mesma regra da Sessão 83. Conferido no banco de dev: 332 meses desde 1999-03, Selic mensal ≈
  CDI + 0,01 p.p. (ex. set/2026 1,089% vs 1,08%) — coerente com o CDI rodar ~0,10 p.p. a.a.
  abaixo da meta.
- [x] Juros ao vivo: `commands/rates.rs::get_interest_rates` (sem tabela no Anchor — o Lume
  cacheia): Selic atual + desde quando + histórico de mudanças; curva do último pregão (volta
  dia a dia, 404 = sem pregão) e de ~1 mês antes; prazos 1/2/3/5/10 anos. Selic e DI **falham de
  forma independente** (`*_error`), buscadas em paralelo.
- [x] UI: widget **Home → Interest rates** (entra pelo `mergeWithDefaults`, sem migração de
  layout) e **Research → Interest rates** (modo sem ticker, `researchView` em `App.tsx`, atalho
  no estado vazio do Research e "Details" no widget): gráfico em degrau da Selic (1A/5A/tudo) e
  curva DI hoje vs. 1 mês atrás (mesmo matiz, traço + legenda) com prazos-chave marcados; tabela
  de prazos com direção em ícone + texto. Skill `dataviz` carregada antes dos gráficos.
- **Achado ao vivo — BCB instável**: `/v1/rates/selic/meta` do Lume dava 502 seguido. Causa: o
  BCB oscila (mesma janela em 0,4s, ~30s, 502 ou HTML com status 200) e o Lume cortava em 15s,
  sendo que uma janela falha derrubava a série inteira. Corrigido **no Lume** (nova tentativa por
  janela, ver SESSIONS.md de lá) e, no Anchor, a Selic deixou de derrubar a coleta dos outros 7
  benchmarks (falhou → 0 meses, tenta na próxima). **Fragilidade pré-existente registrada**:
  CDI/IPCA mensais do BCB também usam 15s sem nova tentativa — um 502 do BCB ainda derruba a
  coleta inteira (visto ao vivo uma vez nesta sessão).
- Verificado: `cargo test --lib` **202/202** (6 novos), `tsc --noEmit` limpo, testes `#[ignore]`
  ao vivo contra o Lume local passando (Selic plausível — 13,75% hoje —, curva do último pregão
  com > 200 vértices, coleta dos 8 benchmarks). **Não testado em tela** (adiado).
- Fora desta fatia: Selic over/efetiva, IPCA+/NTN-B, interpolação flat-forward, alerta de Copom.

### Fase 18 — Software de apoio à gestão de opções (ideia trazida em conversa pelo dono do projeto na Sessão 96, puro registro, não iniciada)

**Objetivo**: suporte a operações com opções sobre ações (ex. venda coberta, travas, etc.) —
ideia registrada em conversa, ainda sem nenhum desenho de escopo ou arquitetura ("pensamos nisso
depois", palavras do dono do projeto). Não decidido: se vira uma classe de ativo nova (ver
`AssetClass`, Fase 10 item 8), como o cálculo de posição/resultado funciona, nem fonte de dado de
cotação de opções.

**Estado**: puro registro de intenção, o mais em aberto de todas as ideias desta sessão — nenhuma
decisão tomada além de "existe vontade de ter isso um dia". **Atualização (Sessão 109)**: a camada de
dados existe em `finance_core` (séries da B3, último preço, Black-Scholes, IV e gregas; ver seção H da
Fase 32), sem tela nem classe de ativo; nada do desenho de produto foi decidido.

**1ª fatia implementada (Sessão 114, 2026-10-17)**, escolhas do dono do projeto: **só consulta** (cadeia +
gregas, somente leitura, sem schema) e a tela como **seção na análise da Ação BR** (não view própria).
- `commands/options.rs`: `list_option_chain(request { underlying })` e `get_option_greeks(request {
  series_ticker, underlying })`. A cadeia sai de `build_chain`, função pura: descarta vencimentos passados
  (o de hoje ainda conta), agrupa por vencimento crescente e pareia call e put pelo strike (chave em centavos
  inteiros, porque `f64` não serve de chave) em ordem crescente. Raiz sem séries é cadeia vazia, não erro.
- `stock-lookup/OptionsSection.tsx`, plugada em `StockAnalysisSection.tsx` antes das teses: **não carrega
  sozinha** (a primeira carga do dia baixa ~80 MB, ~2 min), só com "Load options"; seletor de vencimento,
  filtro "Only traded" (ligado), tabela Calls | Strike | Puts com a linha ATM destacada (strike mais perto do
  preço da ação) e painel de gregas ao clicar numa série. Consultas com `retry: false` (um download de 80 MB
  não pode repetir sozinho). Avisos fixos: só último preço negociado, aproximação europeia.
- **Fora de escopo, decidido**: simulador de payoff (venda coberta, travas), posições e resultado de opções
  na carteira, `AssetClass` de opção, seção em ETF/BDR, opções de índice. Seguem em aberto, sem desenho.
- **Verificado**: `./dev.sh check` limpo; 7 testes unitários de `build_chain`; CI do push `37c76ff` verde nos
  três workflows, com o E2E da PETR4 conferindo que a seção e o botão aparecem e que nada baixa sozinho.
  **Não verificado**: a carga real (`live_option_chain`, `#[ignore]`, ~2 min e B3 instável), o painel de
  gregas contra dado real e o clique na tela.

**2ª fatia implementada (Sessão 115, 2026-10-17)**: **simulador de payoff**, escolhas do dono do projeto:
motor de **pernas livres + atalhos prontos** e **só resultado no vencimento** (sem curva teórica de hoje).
- `domain/option_payoff.rs` (função pura, sem rede): `Leg { kind: stock|call|put, side: long|short, strike,
  price, quantity }`, 1 a 8 pernas. O resultado é linear por partes em `S` com quebras nos strikes, então
  ganho máximo, perda máxima, "ilimitado" e equilíbrios saem **exatos** de `{0} ∪ strikes` e da inclinação
  para `S → ∞` (ação e call comprada +1, vendidas −1, put 0), sem amostrar. Trecho identicamente zero não
  gera equilíbrio; `S = 0` nunca é equilíbrio. Prêmio líquido só das opções (positivo = débito). A curva tem
  121 pontos mais as quebras e os equilíbrios dentro da faixa (60% da menor a 140% da maior referência).
- Comando `simulate_option_strategy(request { spot, legs })` em `commands/options.rs`.
- `stock-lookup/OptionStrategySimulator.tsx`, dentro de `OptionsSection.tsx`: card **sempre visível** (pernas
  manuais funcionam sem carregar a cadeia), editor de pernas (comprar/vender, tipo, strike, preço,
  quantidade; padrão 100 = 1 lote), atalhos (covered call, protective put, bull call spread, bear put spread,
  long straddle, cash-secured put; usam o vencimento selecionado e o último preço, e avisam quando uma perna
  não tem negócio) e "Add as buy/sell leg" na série escolhida. As pernas moram em `OptionsSection`.
  Tiles (Net premium, Max gain, Max loss, Breakeven), gráfico de ganho (azul) e perda (vermelho) com a linha
  do resultado neutra, linha do preço atual, pontos de equilíbrio, legenda, tooltip e tabela "Show values".
- **Cores** (skill `dataviz`): par divergente azul `#3987e5` / vermelho `#e66767`, passos escuros da paleta de
  referência, validado contra a superfície do card `#111820` (ΔE normal 29,0, CVD 19,2, contraste >= 3:1).
- **Verificado**: `./dev.sh check` limpo; 11 testes unitários do motor com valores à mão (call comprada,
  covered call, put protetora, bull call spread, straddle, put vendida, call vendida a seco, só ação,
  resultado zero, curva, validação); CI do push `98f5de4` verde nos três workflows, com o E2E digitando uma
  call (strike 40, prêmio 2, 100 opções) e conferindo perda de R$ 200, equilíbrio em R$ 42, ganho ilimitado e
  débito de R$ 200. **Não verificado**: o gráfico a olho (o passo de renderizar e olhar da skill não foi
  feito), os atalhos e "Add as buy/sell leg" contra a cadeia real (dependem dos 80 MB da B3) e o clique manual.
- **Fora de escopo, decidido**: curva teórica de hoje, custos e IR, dividendos e exercício antecipado, seção
  em ETF/BDR. (As posições na carteira entraram na 3ª fatia, abaixo.)

**3ª fatia implementada (Sessão 116, 2026-10-17)**: **posições em opções na Carteira**, escolhas do dono do
projeto: **tabela própria** (não classe de ativo) e **encerramento manual + marcação a mercado**.
- **Por que tabela própria**: o livro de compra/venda só tem posição comprada (quantidade líquida de compras
  menos vendas) e não comporta opção lançada; classe nova também obrigaria a adaptar cotação (COTAHIST, não
  Yahoo), vencimento e IR de opções. Isolada como o `Loans` da Fase 29: **não entra** em patrimônio, Resumo,
  alocação, rentabilidade nem IR.
- **Schema**: `option_positions` (migration `m20261017_120000`), FK em `portfolio` com cascade; série e
  ativo-objeto, tipo, strike, vencimento, comprada/lançada, quantidade, prêmio por opção, taxa de abertura, e
  as colunas de encerramento (`close_kind` closed/expired/exercised, preço, data, taxa). Aberta = `close_date`
  nulo. O Backup do Workspace cobre por introspecção; o teste de roundtrip ganhou uma posição.
- **Domínio** (`domain/option_position.rs`): resultado = `sinal · (preço − prêmio) · quantidade − taxas`, com
  comprada +1 e lançada −1; serve para o realizado e para a marcação. Dias até o vencimento e o aviso de
  "vencida e ainda aberta".
- **Comandos** (`commands/option_position.rs`, padrão `*_in` do `loan.rs`): criar (valida tipo, lado, strike,
  quantidade, prêmio, taxas, datas e vencimento >= abertura), listar (**só banco, sem rede**), encerrar
  (`expired` força preço 0; `closed` e `exercised` exigem preço; só posição aberta; data >= abertura), apagar e
  `get_option_marks` (último preço negociado via `b3_options::market()`, **só sob clique**, porque a primeira
  carga baixa ~80 MB). Série que nunca negociou fica sem preço, sem erro.
- **Tela**: aba "Options" na Carteira (`OptionPositionsSection.tsx`): formulário (vencimento já vem daqui a
  30 dias, abertura hoje), três tiles (abertas, resultado a mercado, resultado realizado), tabela de abertas
  com "Expired: close it" quando o vencimento passou, "Refresh market prices", Close e Delete (com confirmação),
  diálogo de encerramento e tabela de fechadas. Ganho e perda levam sinal explícito, não só cor.
- **Exercício**: o usuário informa o valor intrínseco por opção; a ação comprada ou vendida pelo exercício
  **não** é registrada (avisado na tela).
- **Verificado**: `./dev.sh check` limpo; 5 testes unitários do domínio e 4 testes de fluxo no schema real
  (criar/encerrar/realizado com os dois lados, validações e isolamento por carteira, vencida e marcação com
  mercado sintético, cascade ao apagar a carteira) mais o roundtrip do backup; CI do push `f6e904d` verde nos
  três workflows, com um spec E2E novo (registra uma call lançada, encerra sem valor e confere o realizado de
  +R$ 150,00; e o erro do backend na quantidade zero). **Não verificado**: `get_option_marks` contra a B3 real
  (80 MB, só sob clique), a tela a olho e o clique manual.

**Imposto sobre o resultado das opções (Sessão 117, 2026-10-17)**: ligação das posições encerradas à apuração
da Fase 24, escolha do dono do projeto de fazer **só o imposto** (o patrimônio fica para depois).
- **Regras** (renda variável na B3): swing (aberta e encerrada em dias diferentes) na gaveta **comum** a 15 %;
  day trade (mesmo dia) na gaveta de **day trade** a 20 %; **sem a isenção dos R$ 20 mil** (só vale para venda
  de ações; o resultado de opção não entra em `stock_sales_total`); prejuízo compensa na mesma gaveta, inclusive
  contra ação, e o saldo segue para os meses seguintes. Mês = mês do **encerramento** (lançada que expira
  realiza o prêmio nessa data; comprada que vira pó é prejuízo). Alíquotas e DARF (código 6015) são as já
  semeadas em `tax_rules`.
- **IRRF**: o day trade retém 1 % do ganho, como em qualquer day trade (a apuração já fazia isso por ganho);
  o do swing (0,005 % sobre a venda) **não é estimado**, porque `sale_value` fica 0, então o imposto de swing
  pode sair um pouco acima do real. A nota na tela diz isso.
- **Exercida fica de fora**: no exercício o prêmio entra no custo da ação, não é ganho nem perda própria. Gera
  `TaxWarning::OptionExercised` ("update that stock's cost yourself").
- **Código**: `assessment::OptionResult`, `assess_with_options` e `simulate_sale_with_options` (a simulação de
  venda também enxerga as opções; os invólucros antigos `assess`/`simulate_sale` viraram ajudantes só dos
  testes). `domain::option_position::proceeds_and_cost` (receita − custo = resultado). `load_option_results`
  em `commands/tax.rs` lê as posições fechadas das carteiras do Workspace (`day_trade = open_date ==
  close_date`); `assess_workspace` e `simulate_tax_sale_in` passam os resultados. Cada opção vira um
  `SaleResult` com `tx_id`/`asset_id` negativos, sem colisão com os reais. Aba Taxes: aviso novo, nota no mês e
  `data-testid` no valor devido e no "Details".
- **Erro meu corrigido no CI**: eu tinha afirmado (nos testes, na nota e no comentário) que não haveria IRRF
  para opção. Estava errado para o day trade: o código já retém 1 % do ganho, e a lei é a mesma para opções.
  O primeiro push falhou em dois testes por isso; o código estava certo e corrigi as afirmações.
- **Verificado**: `./dev.sh check` limpo; 7 testes de domínio novos (swing sem isenção, convivência com ação
  isenta, compensação de prejuízo, day trade, exercida, apuração sem opções inalterada, simulação) e 2 de fluxo
  no schema real; CI do push `4cc3d30` verde nos três workflows, com o E2E conferindo R$ 28,50 devidos em day
  trade (20 % de R$ 150 menos 1 % de IRRF) e R$ 22,50 depois de antedatar a abertura (swing). **Não
  verificado**: a tela a olho e o clique manual.
- **Fora de escopo, decidido**: IRRF de swing em opção, efeito do exercício na base de custo da ação, DARF
  específico de opções. (O patrimônio entrou na Sessão 118, abaixo.)

**Posições abertas no patrimônio (Sessão 118, 2026-10-17)**, escolhas do dono do projeto: posição sem preço vale
o **prêmio, como custo** (igual ao que a carteira faz sem cotação) e a flag "entra no patrimônio" nasce
**ligada**, por posição, como nos empréstimos.
- **Por que guardar o preço**: o patrimônio não usa rede e a marcação baixa ~80 MB da B3. `get_option_marks`
  passa a **gravar** `mark_price`, `mark_date` (data do último negócio) e `marked_at` nas posições abertas que
  tiveram negócio; série sem negócio mantém o preço anterior. A leitura (`option_marks_in`) segue pura e a
  gravação é `save_option_marks_in`.
- **Schema**: migration `m20261017_130000` acrescenta a `option_positions` `include_in_net_worth` (padrão
  ligado), `mark_price`, `mark_date` e `marked_at` (uma `ALTER TABLE` por coluna, limite do SQLite).
- **Valor** (`domain::option_position::net_worth_value`): comprada é ativo (`+preço · quantidade`), lançada é
  passivo (`−preço · quantidade`, o que custaria recomprar). Preço = o guardado ou, sem ele, o prêmio (marcada
  `at_cost`). **Aberta e vencida vale 0** (falta só encerrar) e não conta como "a custo". O prêmio recebido de
  uma lançada, se foi para uma conta, já está no caixa.
- **Patrimônio**: `compute_net_worth` ganha `total_options_value` e `options_at_cost_count`;
  `net_worth = mercado + caixa + empréstimos + opções − dívidas`. A lançada **não** entra em `total_debt`,
  então a alavancagem não muda por causa dela. `domain::net_worth::sum_options_in_net_worth` respeita a flag.
  `set_option_position_net_worth` liga e desliga.
- **Tela**: aba Options com coluna "Net worth" (valor, "at cost", "expired, counts 0", botão Leave out / Count
  it), tile "Net worth impact" com "N at cost (no market price yet)", preço guardado com a data do negócio e
  resultado a mercado calculado no backend (`market_pnl`, sem taxa de saída) mesmo sem clicar em Refresh;
  Finanças → Liabilities ganhou o cartão "Options" (grade de 7 colunas).
- **Verificado**: `./dev.sh check` limpo; 5 testes de domínio novos e 3 de fluxo no schema real (a custo e depois
  ao preço guardado, série sem negócio mantém o preço, flag, posição encerrada, vencida vale 0, flag de
  posição inexistente); CI do push `da0c670` verde nos três workflows, com um caso de E2E novo (uma lançada a
  custo aparece como −R$ 100,00 na aba e em Finanças e some ao desligar a flag). **Não verificado**: a gravação
  dos preços pelo `get_option_marks` real (80 MB, só sob clique), a tela a olho e o clique manual.
- **Fora de escopo, decidido**: margem ou garantia exigida para lançadas, valor intrínseco no vencimento de
  posição aberta, histórico de preços das posições, opções no Resumo da carteira, na alocação e na
  rentabilidade, atualização automática dos preços.

**Salvar a estratégia do simulador como posições (Sessão 119, 2026-10-17)**: leva as pernas de opção do
simulador para uma carteira de uma vez, em vez de redigitar cada uma na aba Options.
- **O que falta numa perna**: o simulador só tem tipo, lado, strike, preço e quantidade; uma posição exige
  também série, vencimento, ativo-objeto, abertura e carteira. Pernas vindas da cadeia (atalhos e "Add as
  buy/sell leg") agora **carregam** `seriesTicker` e `expiration` (`LegDraft`, `buildPreset`, `legFromSeries`);
  as manuais pedem a série no diálogo, com o vencimento já em hoje + 30 dias. Editar o tipo ou o strike de uma
  perna solta a série que veio da cadeia (deixaria de bater). Ativo-objeto = ticker da análise; abertura = hoje.
- **Perna de ação não vira posição** (as posições só rastreiam opções); o diálogo avisa quantas foram ignoradas.
- **Backend**: `create_option_position_in` ficou genérica em `ConnectionTrait` e o comando novo
  `create_option_positions(request { portfolio_id, underlying_ticker, open_date, legs })` salva de 1 a 8 pernas
  **tudo ou nada** numa transação, reaproveitando toda a validação. Cada perna é uma posição independente (a
  estratégia não vira grupo) e a taxa de abertura não é pedida.
- **Tela**: botão "Save as positions" no simulador (só com perna de opção válida); diálogo com o seletor de
  carteira (já escolhida se for a única; mensagem se o Workspace não tem nenhuma), uma linha por perna com série,
  vencimento, prêmio e quantidade editáveis, e a confirmação com o caminho Portfolio → Options. Invalida as
  consultas de posições e de patrimônio.
- **Verificado**: `./dev.sh check` limpo; 3 testes de fluxo no schema real (trava de 2 pernas salva de uma vez,
  perna inválida não salva nenhuma, limites de 1 a 8 pernas e carteira inexistente); CI do push `a83762f` verde
  nos três workflows, com o E2E salvando uma call manual e conferindo no banco `PETRK400:long:2.0:PETR4`.
  **Não verificado**: pernas vindas da cadeia real carregando série e vencimento (dependem dos 80 MB da B3), a
  tela a olho e o clique manual.
- **Erros no caminho**: um nome de variável de callback escondia o vencimento da cadeia (pego pelo `tsc`); e o
  primeiro push do E2E falhou porque o teste terminava com o diálogo de sucesso aberto, e o diálogo modal
  deixava a busca global inerte para os casos seguintes (corrigido fechando-o no teste).
- **Fora de escopo, decidido**: taxa por perna, agrupar a estratégia, salvar a perna de ação, criar carteira no
  diálogo, editar posição já salva.

### Fase 19 — Moeda base seletiva da carteira (ideia trazida em conversa pelo dono do projeto na Sessão 96, puro registro, não iniciada)

**Objetivo**: permitir escolher, nas Configurações, a moeda base em que a carteira é exibida (ex.:
um usuário americano escolhe USD e o Workspace inteiro passa a mostrar valores convertidos nessa
moeda, em vez do BRL fixo de hoje). Cobre pelo menos: campo de moeda base em Settings (por
Workspace, já que a Fase 10 tornou Workspace multi-usuário — cada um pode querer uma moeda
diferente); conversão de exibição via câmbio (ao vivo vs. fechamento diário, a decidir); e a
questão de exibição vs. armazenamento — provavelmente cada ativo continua guardado/transacionado
na sua moeda de origem e a conversão acontece só na camada de exibição/agregação, não decidido
com certeza ainda.

**Estado**: 1ª entrega implementada na Sessão 120 (Resumo, Posições e Home). Rentabilidade, patrimônio
e as demais telas seguem sem conversão — ver "Pendente" abaixo.

**Decisões (Sessão 120, plan mode)**: moeda base **por Workspace** (tabela `workspace_settings`, sem
linha = BRL); taxa = **último fechamento** do par em `stock_price_history` (sem rede na leitura);
par ausente → o `set_base_currency` coleta sozinho via `finance_api::fx::collect_pair` e, se falhar,
devolve `missing_pairs` e a posição fica fora dos totais com aviso. Cada ativo continua guardado na
moeda de origem: a conversão é só de exibição.

**Implementado**:
- `domain/currency_conversion.rs` (`FxRates`): rota direta, inversa ou cruzada via USD; cotação ≤ 0 é
  ignorada.
- `commands/currency.rs`: `get_workspace_settings`, `set_base_currency`, `positions_in_base`,
  `convert_position`, `sum_in_base` (proventos 12m pela moeda do ativo).
- `compute_positions` continua nativo (patrimônio, rentabilidade e impostos dependem disso);
  `get_portfolio_positions`, `get_portfolio_summary` e `get_workspace_summary` convertem. `PositionView`
  ganhou `native_currency` e `fx_missing`; os resumos ganharam `base_currency` e `positions_missing_fx`.
- Settings → Currency, `formatMoney` em `lib/utils.ts`, selo "from USD" nas posições e aviso de câmbio
  ausente no Resumo e na Home.

**Efeito colateral deliberado**: antes, ativo em USD era somado cru como se fosse BRL; agora é convertido
(com BRL como base e o `USDBRL` coletado). Sem o par coletado, a posição sai dos totais com aviso.

**2ª entrega (Sessão 121) — rentabilidade na moeda base**: `compute_profitability` ganhou o parâmetro
`base` e converte cada fluxo (compra/venda/provento) pela taxa da `transaction_date`, o EMV de cada mês pela taxa
do fim do mês e o DARF (sempre BRL) pela data do pagamento — a variação cambial entra no retorno. Usa
`FxHistory` (`domain/currency_conversion.rs`), que pega a cotação da data mais próxima em qualquer distância
(decisão do dono do projeto); só falha, com `NotFound` nomeando o par, se o par nunca foi coletado. Com a base
≠ BRL os benchmarks (CDI/IPCA/Selic e índices em BRL) vêm vazios e a tela avisa. O retorno alavancado de
`liability.rs` fica em BRL fixo (a taxa da dívida é em reais).

**Mudança de comportamento**: carteira com ativo em USD e base BRL, sem o `USDBRL` coletado, antes mostrava um
TWR com USD somado cru e agora dá erro pedindo para coletar o par.

**Pendente**: patrimônio líquido, caixa, dívidas e empréstimos (sem coluna de moeda, seguem BRL implícito);
custo convertido na data de cada fluxo no P/L das posições (hoje usa a taxa mais recente, então o P/L inclui
variação cambial do custo); formatadores locais de Ledger, Dividendos e impostos; benchmarks convertidos
(hoje só escondidos fora do BRL); impostos ficam sempre em BRL (regra fiscal).


### Fase 20 — Remover a IA do Anchor (Warden vira a única interface de IA; decisão registrada na Sessão 100, **executada na Sessão 101**)

**Origem**: decisão do dono do projeto, trazida no arquivo solto `anchor.md` (raiz do repo, fora
do `project/`) e registrada aqui na Sessão 100.

**Decisão**: tirar o painel de chat de IA do Anchor (entregue na Fase 7 — `desktop/src/chat/`,
`desktop/src/chat-full/`, `desktop/src/components/assistant-ui/`). O **Warden** (repo irmão
`../warden`) passa a ser a **única interface conversacional do ecossistema** — o Anchor não
duplica essa funcionalidade.

**Em aberto (pra debater antes de implementar)**:
- Escopo exato da remoção: só o chat (`ChatPanel`/`ChatScreen`/`ChatToggleButton`/`ApiKeyGate`)
  ou também as outras features de IA que não são conversa — ex. `ai/DocumentExtractDialog.tsx`
  (extração de dados de documento) e `ValuationProposalCard` (IA propõe criar valuation, Fase
  7.10.4). A decisão original fala de "AI chat panel", não de toda IA.
- O que acontece com a chave de API guardada no keyring e com o backend Rust do chat (comandos,
  contador de tokens da Fase 7.10.3).
- Como o Warden passa a enxergar os dados do Anchor pra substituir o chat (ex. tool/MCP do
  Warden lendo o banco/API do Anchor) — sem isso, remover o chat é perda de funcionalidade, não
  migração. Conecta com a Fase 25 (Anchor como servidor expõe uma API que o Warden poderia
  consumir).

**Debate fechado na Sessão 101 (2026-10-14)**, 3 decisões do dono do projeto:
1. **Escopo: toda a IA**, não só o chat (contra minha recomendação de manter as features não
   conversacionais). Sai junto: extração de PDF (botão do landbank no RNAV, Fase 2.4), "About the
   asset" (Fase 10 item 8, Sessão 60), a parte "futuro" da 13.6 (avisos de proventos da CVM lidos
   por IA, Sessão 94) e a tela de chaves de API em Settings.
2. **Dados: migration que apaga** as tabelas (`m20261014_100000_drop_ai_tables`).
3. **Timing: agora**, aceitando perder a funcionalidade até existir ponte com o Warden (Fase 25 ou
   um MCP futuro).

**Executado**:
- [x] Frontend: `src/chat/`, `src/chat-full/`, `src/components/assistant-ui/`, `src/ai/`,
  `AboutCompanySection.tsx` apagados; `App.tsx` sem botão/painel de chat; Settings sem a seção
  "IA"; RNAV com landbank só manual; Dividends sem "Check CVM notices" (sugestões `source =
  "cvm"` já gravadas continuam no banco e aparecem normalmente). Deps npm removidas:
  `@assistant-ui/react`, `@assistant-ui/react-markdown`, `zustand`, `tw-shimmer` (as 4 entraram
  com a Fase 7.10).
- [x] Backend: comandos `chat`/`conversation`/`ai_proposal`/`api_key`/`company_ai_info`/
  `document_extraction`/`cvm_dividend_notice`, `domain::{chat_provider,cvm_dividend_notice}`, 6
  entidades, 5 variantes de `AppError` só de IA, e o código que virou morto em `finance_api`
  (`fetch_company_dividend_notices`, `stocks::resolve_cvm_code`). `KEYRING_SERVICE` mudou pra
  `finance_api_settings.rs` (único consumidor restante).
- [x] Migration irreversível (`down` devolve erro) que dropa `ai_valuation_proposal`,
  `ai_message`, `ai_conversation`, `ai_api_key`, `company_ai_info`,
  `cvm_dividend_notice_documents`.
- `cargo test --lib` **170/170** (eram 203 — os 33 a menos moravam nos módulos apagados),
  `tsc --noEmit` limpo, crate `migration` compila.

- [x] Migration aplicada no banco real de dev (Sessão 101, com cópia antes em
  `data/anchor-pre-fase20.db`). Nenhuma chave de IA órfã esperada no keyring (`ai_api_key` já
  estava vazia).

**Pendente**: teste de clique no app rodando — ver `SESSIONS.md`, Sessão 101.

### Fase 21 — Consumo do Lume (ex-EasyBusiness) com fallback pra instância compartilhada do Warden (registrada na Sessão 100, não iniciada)

> **Superada pela Fase 26 (Sessão 107)**: o Lume foi aposentado e o Anchor busca os dados direto
> das fontes públicas; não há mais sidecar nem instância compartilhada pra descobrir.

**Origem**: arquivo solto `anchor.md`, registrado aqui na Sessão 100.

**Contexto de nomenclatura**: o repo `easybusiness` citado nas Fases 14 e 17 (Finance API) foi
**renomeado pra Lume** (repo irmão `../lume`). O Anchor continua sendo o primeiro consumidor-alvo
do Lume. Os textos antigos das Fases 14/17 mantêm "EasyBusiness" por serem registro histórico;
daqui pra frente o nome é Lume. (O `anchor.md` menciona um `lume.md` de referência que **não
existe** neste repo — provavelmente estava junto com esse arquivo em outro lugar.)

**Decisão**: o Anchor consome o Lume via **sidecar local** (o mecanismo que já existe desde a
Fase 14.2 — `desktop/src-tauri/src/finance_api/`, binário embutido, modo Local/Remote da Fase
14.3), **com fallback pra uma instância compartilhada se o Warden já tiver subido uma** na mesma
máquina — em vez de cada app do ecossistema subir o seu próprio processo do Lume. Sem custo pro
usuário na versão gratuita/local.

**Em aberto**: como o Anchor descobre que já existe uma instância do Warden rodando (porta fixa
conhecida, arquivo de lock/descoberta, registro no SO); ordem de preferência (usar a
compartilhada primeiro e só subir sidecar próprio se não existir, ou o contrário); o que fazer se
o Warden fechar e derrubar a instância no meio do uso do Anchor; compatibilidade de versão entre
o Lume embutido no Anchor (`.easybusiness-version`, fixado por SHA) e o do Warden.

**Estado**: decisão de direção tomada, nada implementado. Hoje o sidecar local já funciona
(Fase 14); falta só a parte do fallback/compartilhamento.

### Fase 22 — Motor de backtesting / portfolio quant como módulo do Anchor (ideia registrada na Sessão 100, não decidida)

**Origem**: arquivo solto `anchor.md` — "ideia registrada pra explorar depois (não decidida, só
anotada)".

**Ideia**: um motor de backtesting de estratégia de carteira — alocação, rebalanceamento,
drawdown, correlação — consumindo dados históricos do Lume (Fase 21).

**Direção já pensada**: faz mais sentido como **módulo dentro do Anchor**, não como projeto/app
separado — seria útil também pro usuário avançado, não só pensando numa futura gestora de fundos,
e aproveita a base de usuários que já valida teses ativo por ativo no Anchor (valuation/teses
viram estratégia de carteira testável).

**Estado**: puro registro de intenção, nada decidido sobre escopo, UI ou onde o cálculo roda.

### Fase 23 — Attestations cross-app via TruthID (provar fatos da carteira sem expor dados; ideia registrada na Sessão 100, não decidida)

**Origem**: arquivo solto `anchor.md` — "ideia registrada pra explorar depois (não decidida, só
anotada)".

**Ideia**: usar o `VaultRegistry`/ZK attestations do TruthID (item 2 do roadmap do TruthID) pra
o Anchor **provar coisas sobre a carteira do usuário sem expor os dados brutos** — ex. "patrimônio
acima de X". Reaproveita essa capacidade do TruthID dentro do próprio ecossistema antes de
oferecê-la pra fora. Conecta com a Fase 8 (Anchor já integra com TruthID pra assinatura/cifra do
sync).

**Estado**: puro registro de intenção, depende do lado TruthID existir primeiro.

### Fase 24 — Gestão de impostos sobre investimentos (apuração automática + DARF; ideia trazida pelo dono do projeto na Sessão 100, não iniciada)

**Origem**: pedido em conversa do dono do projeto na Sessão 100.

**Objetivo**: o Anchor apurar sozinho o imposto devido sobre investimentos, de forma "bem
eficiente". Fluxo desejado, nas palavras do dono do projeto: "comprou tanto, teve tanto de lucro
na venda, portanto terá que pagar isso, clique aqui para gerar o DARF e pagar".

**O que foi pedido explicitamente**:
1. **Cálculo automático** do ganho/prejuízo em cada venda a partir das transações já registradas
   na Carteira (preço médio de compra já existe — ver Fase 13), e do imposto devido resultante.
2. **Geração do DARF** a partir do valor apurado ("clique aqui pra gerar e pagar").
3. **Imposto pago registrado automaticamente** em dois lugares:
   - na **Carteira**, entrando no cálculo de rentabilidade (rentabilidade líquida de imposto);
   - nas **Finanças Gerais** (Fase 12), como saída no fluxo de caixa.

**Contexto de domínio a levantar/validar no debate** (regras BR que o motor provavelmente precisa
cobrir — **não verificadas nesta sessão, legislação muda com frequência, confirmar na fonte
oficial antes de implementar**):
- Apuração **mensal**, separada por categoria: ações swing trade (com isenção pra vendas até um
  limite mensal), day trade (alíquota maior, sem isenção), FII (sem isenção sobre ganho de
  capital), ETF, BDR, cripto (regras e limites próprios), ativos no exterior (regime anual da Lei
  14.754/2023, diferente do mensal).
- **Compensação de prejuízo** acumulado de meses anteriores, dentro da mesma categoria
  (swing com swing, day trade com day trade, FII com FII).
- **IRRF "dedo-duro"** retido na fonte na venda, abatido do imposto devido.
- Regra do **valor mínimo de DARF** (valor abaixo do mínimo acumula pro mês seguinte) e
  **vencimento** (último dia útil do mês seguinte à venda), com multa/juros se atrasar.
- Renda fixa/fundos: IR já retido na fonte (tabela regressiva, come-cotas) — provavelmente só
  informativo/registro, não gera DARF.
- Proventos: dividendos (hoje isentos, mas em discussão legislativa) vs. JCP (retido na fonte) —
  a automação de proventos (Fase 13.6) já existe e pode alimentar isso.

**Em aberto (pra debater)**:
- Como o "gerar DARF" funciona na prática: não há API pública oficial conhecida da Receita pra
  emitir DARF de terceiros — opções a investigar: gerar os dados preenchidos (código de receita,
  período de apuração, valor) e abrir o Sicalc Web da Receita; gerar código de barras/PIX
  localmente se o layout for público; ou só um guia copiável.
- Escopo da primeira fatia (ex. só ações swing trade + FII, deixando cripto/exterior/day trade
  pra depois).
- Se day trade precisa de marcação explícita na transação (hoje provavelmente não distingue).
- Se entra também apoio à **declaração anual do IRPF** (bens e direitos com custo de aquisição,
  rendimentos isentos, prejuízos a compensar) — extensão natural, não pedida explicitamente.
- Onde mora o dado: tabela própria de apuração mensal + DARFs gerados/pagos (status: a pagar,
  pago, vencido), ligada às transações da Carteira e a uma transação em Finanças Gerais.
- Multi-moeda/exterior interage com a Fase 19 (moeda base).

**Estado (Sessão 100)**: puro registro de intenção.

**Fatia 1 implementada na Sessão 101 (2026-10-14)**, planejada em plan mode e aprovada. Pedido
central do dono do projeto: "saber que os impostos existem é o mais importante", e os **valores**
das regras chegam atualizados mas são **editáveis pelo usuário** (a lógica não) — o software não
pode depender dele pra acompanhar mudança de lei.

Decisões (AskUserQuestion): B3 calculado (ações swing/day trade, units, FII, ETF, BDR) + só
**avisos** pra cripto e exterior; regras servidas pelo **Lume** (`/v1/tax-rules/br`, item 1.17
de lá, criado nesta sessão) com seed embutido como fallback offline; apuração no nível do
**Workspace** (imposto é por CPF); "pago" vai pra **Finanças Gerais + rentabilidade líquida**.

Regras levantadas na fonte oficial (Receita: páginas "Bolsa de Valores", "Cálculo e Pagamento",
"Isenções", "Compensações", Manual ReVar) — tabela completa e fontes no plano da sessão:
- Gaveta comum 15% (ações, units, ETF, BDR); isenção de vendas ≤ R$ 20 mil/mês **só pra ações**.
- **Units (final 11) sem isenção** — apontado pelo dono do projeto, confirmado (SC Cosit
  145/2021, Manual do ReVar); também não somam no limite de R$ 20 mil. `acao_br` com ticker
  terminando em `11` = unit.
- Day trade 20% (mesmo ativo, mesmo dia, mesma corretora — detectado automaticamente, não há
  marcação manual), FII 20% em gaveta própria. Prejuízo compensa só pra frente, sem prazo, dentro
  da gaveta. IRRF 0,005% da venda / 1% do ganho de day trade (estimado, abate do imposto). DARF
  < R$ 10 acumula. Vencimento último dia útil do mês seguinte (feriados nacionais + Carnaval/
  Sexta Santa/Corpus Christi/31-12). Código 6015.
- Mudanças recentes: MP 1.303/2025 **caducou** (regras acima seguem); **JCP 15% → 17,5%** desde
  2026 (LC 224/2025) — `JSCP_IR_RATE` hardcoded da Fase 13.4 saiu, `net_total` usa a regra
  vigente na data do provento.
- **Decisão de interpretação a confirmar**: prejuízo de ações em mês isento é levado pra frente,
  e lucro isento não consome prejuízo acumulado — prática majoritária do mercado (fontes
  secundárias), sem resposta oficial achada.

Implementado:
- [x] **Lume 1.17** — `GET /v1/tax-rules/br`, dado curado e versionado (ver PHASE.md de lá).
- [x] Migrations `create_tax_rules` (seed = mesmo conteúdo do Lume; `value` + `default_value` +
  `user_edited`; único por `(rule_key, effective_from)` — regra nova = linha nova com vigência)
  e `create_tax_payments`.
- [x] `domain/tax/` puro: `rules` (regra vigente por data), `classify` (classe → gaveta, units),
  `calendar` (vencimento, Páscoa), `assessment` (custo médio cronológico com taxas e reset ao
  zerar, day trade por corretora, isenção, gavetas, compensação, IRRF, acúmulo < mínimo, avisos).
  O preço médio de exibição (`compute_positions`) segue intocado — ele soma todas as compras da
  história e ignora `fee`, servindo pra tela mas não pra IR.
- [x] `commands/tax.rs`: `list/update/reset_tax_rule`, `sync_tax_rules` (atualiza só o que o
  usuário não editou; editado + valor oficial novo = "conflito" mostrado na UI), `get_tax_
  assessment`, `mark_tax_paid` (cria despesa em Finanças Gerais, categoria "Taxes" criada se
  faltar, numa transação) / `unmark_tax_paid`, `tax_cost_flows` (rateio do DARF pras carteiras/
  ativo pró-rata do ganho tributável).
- [x] Rentabilidade: DARF pago entra como custo no Dietz no mês do pagamento.
- [x] UI: aba **Portfolio → Taxes** (KPIs, avisos, tabela mês a mês com detalhe por gaveta/venda,
  guia do DARF copiável + abrir "Meu Imposto de Renda"/página da Receita, marcar pago) e
  **Settings → Taxes** (editar/resetar cada regra, "Check for updates" via Lume). Não existe API
  pública de DARF; a URL exata do Sicalc não foi confirmada, então os botões apontam só pra
  páginas da Receita verificadas.

Verificado: `cargo test --lib` **196/196** (26 novos, incl. o exemplo oficial da Receita —
ganho 9.967, imposto 1.495,05, IRRF 3,00 → **1.492,05** — units, day trade parcial, gavetas,
compensação, reset do custo médio, acúmulo < R$ 10, vencimentos com feriado, JCP por
vigência); `tsc --noEmit` limpo; migrations aplicadas no banco de dev (cópia antes em
`data/anchor-pre-fase24.db`), seed conferido; teste `#[ignore]` ao vivo contra o Lume local
(`live_lume_rules_match_migration_seed`) passando. **Não testado em tela** (adiado pelo dono do
projeto); o banco de dev não tem nenhuma transação, então a apuração ainda não rodou contra
dado real — sugestão: conferir o primeiro mês real contra o ReVar.

**Fora desta fatia**: cálculo de cripto e exterior (hoje só aviso); eventos corporativos
(desdobramento/bonificação não existem como tipo de transação — quem tiver vai ver custo
distorcido); `metal` (COMEX, não ouro ativo financeiro B3) ignorado; IRRF real informado pela
corretora (hoje estimado); declaração anual (IRPF).

**Sidecar atualizado (mesma sessão)**: Lume 1.17 publicado (`Arven-Labs/lume@a418f24`). De
caminho, o pin foi **renomeado** `.easybusiness-version` → `.lume-version` (já com esse SHA) e o
`build.yml` passou a fazer checkout de `Arven-Labs/lume` (público, sem token) em `lume/` — pedido
do dono do projeto, o nome antigo só sobrevivia por redirect do GitHub. README também. Textos
históricos neste arquivo mantêm "easybusiness". `build.yml` continua nunca disparado neste repo.

**Ideia nova, Sessão 102 — indicador dedicado de prejuízo a compensar** (pedido do dono do projeto,
puro registro): um indicador que mostre quanto de imposto dá pra abater usando prejuízos de vendas
passadas ("se vender no prejuízo, abate de lucro futuro"). O dono não sabia se há prazo e pediu
pesquisa. **Achado ao registrar**: a apuração já implementada acima compensa prejuízo acumulado
(só pra frente, **sem prazo**, dentro da gaveta: ações, FII, day trade — ver "Prejuízo compensa só
pra frente" acima), mas o saldo é só um passo interno do cálculo. O que falta é **exibi-lo**: saldo
de prejuízo acumulado por gaveta, e uma estimativa de imposto que ele economizaria ("quanto
posso abater"). Pontos a confirmar na pesquisa/discussão: regra vigente de prazo e de gavetas
(a Fase 24 assumiu sem prazo, conferir), onde mostrar (aba Taxes e/ou widget na Home), se simula
uma venda hipotética ("se eu vender X com lucro Y, abato Z"). Não implementado.

**Implementado na Sessão 106 (fatia 1, exibição do saldo)**: `assess` devolve `loss_balances`
(`LossBalance`: gaveta, saldo, alíquota, `tax_saved` = saldo × alíquota), lido do `loss_carried`
do último mês em que cada gaveta teve vendas; só gavetas com saldo > 0. Card "Losses to offset"
na aba Portfolio → Taxes, com a regra no texto (gavetas não se misturam, sem prazo). 2 testes
novos em `domain::tax::assessment`. **Segue de fora**: widget na Home. A regra de prazo/gavetas é a já implementada na Fase 24, não foi reconferida na Receita
nesta sessão.

**Simulação de venda (Sessão 106, fatia 2)**: comando `simulate_tax_sale` + card "Simulate a
sale" na aba Taxes. Escolhe ativo B3 (ação, unit, ETF, BDR, FII), quantidade, preço, taxas e
data. O backend (`domain::tax::assessment::simulate_sale`) apura as transações reais **com e sem**
a venda hipotética e devolve a diferença: ganho/prejuízo, imposto do mês antes e depois, prejuízo
consumido, lucro isento, saldos de prejuízo antes e depois e a variação total do imposto (pega o
efeito nos meses seguintes). Nada é gravado. Reaproveita a apuração real, então isenção, custo
médio e compensação seguem as mesmas regras. Limites: a venda simulada nunca casa como day trade
(sem corretora); ativo sem compras registradas dá erro; venda maior que a posição marca
`cost_unknown`. Refatoração: `load_trades` extraído de `assess_workspace` em `commands/tax.rs`.

### Fase 25 — Web app auto-hospedado: o próprio Anchor como servidor (ideia trazida pelo dono do projeto na Sessão 100, não iniciada)

**Origem**: pedido em conversa do dono do projeto na Sessão 100, citando o fluxo desejado do
Warden como referência (ver `../warden/project/ROADMAP.md`, seção "Interface web auto-hospedada no
próprio hub", `PENDING.md` P78, e Fase 9.8 do Warden — "app desktop embute o próprio
`warden-server`").

**Objetivo**: o Anchor ter uma versão **web auto-hospedável** — o mesmo app que existe no desktop
e no mobile, só que acessível pelo navegador — onde **o servidor é o próprio app**, não um
produto separado:
- Uma instância desktop do Anchor pode ser **só app** (como hoje) ou **servidor** (e continuar
  sendo app ao mesmo tempo).
- Isso se define nas **Configurações**: "esta instância é um servidor", **porta**, e esse tipo de
  configuração.
- Com o modo servidor ligado, abrir `http://<ip>:<porta>` no navegador mostra a interface web
  completa do Anchor daquela instância (mesmo espírito Jellyfin que o Warden adotou: o servidor
  que você hospeda já vem com a própria interface web).

**Pontos pra debater** (levantados ao registrar, nada decidido):
- **Frontend**: reaproveitar o React do `desktop/` trocando a camada `invoke()` do Tauri por
  chamadas HTTP pro servidor (exige uma camada de transporte abstrata no frontend) vs. frontend
  web separado. Reaproveitar parece natural já que o app inteiro já é React.
- **Backend**: os `#[tauri::command]` hoje são a API; expor a mesma lógica via HTTP (ex. axum)
  dentro do processo Tauri, reaproveitando os mesmos serviços/SeaORM.
- **Autenticação no navegador**: login próprio vs. usuários/RBAC dos Workspaces (Fase 10) vs.
  TruthID (Fase 8/23). Obrigatório se exposto fora da LAN.
- **HTTPS** quando exposto na internet; bind só em localhost/LAN por padrão.
- **Modo headless**: servidor rodando sem janela (homelab, VPS, imagem Docker) — o dono do
  projeto descreveu "desktop pode ser servidor", mas um servidor 24/7 provavelmente quer rodar
  sem a UI desktop aberta.
- **Relação com o mobile** (Fase 11.2, Flutter com banco próprio, sem sync): o mobile poderia
  virar cliente deste servidor, o que resolveria o problema de sync do mobile por outro caminho
  que não a Fase 8.
- **Relação com a Fase 8** (sync descentralizado via TruthID/Arweave) e com o
  `SelfHostedProvider` reservado na Fase 15 — um servidor Anchor próprio é um candidato natural
  a implementação do `SelfHostedProvider`.
- **Concorrência** no SQLite com vários clientes simultâneos (desktop local + navegadores).
- **Versão paga hospedada** análoga ao tier pago do Warden (P50) e ao `ManagedCloudProvider` da
  Fase 15 — não pedida agora, só registrar a simetria.
- Serve de API pro Warden consumir os dados do Anchor (ver Fase 20).

**Estado**: puro registro de intenção — dono do projeto pediu explicitamente pra só registrar e
debater depois.

### Fase 26 — Anchor autossuficiente: absorver o que era do Lume (spec trazida pelo dono do projeto na Sessão 101, direção decidida, nada explorado nem implementado)

**Origem**: arquivo solto `SPEC-anchor-autossuficiencia.md` na raiz (destinado a "um agente que
trabalha no repositório do Anchor"), registrado aqui e removido da raiz na Sessão 101. A spec cita
um `SPEC-lume-desmontagem.md` **que não existe neste repo** (vive em outro lugar — provavelmente no
repo do Lume ou do ecossistema).

**Regra número um da spec pro agente: não presumir nada.** Nem estrutura de arquivos, nem código,
nem convenções — a conversa que originou a spec foi de alto nível e o código real pode divergir do
que foi imaginado. Antes de propor qualquer coisa: (1) explorar o repo e entender o que existe;
(2) apresentar ao dono do projeto o que foi achado; (3) discutir dúvidas e **confirmar cada decisão
de arquitetura** ("vai ser assim mesmo?"); (4) com mais de um caminho razoável, apresentar opções
com prós e contras e deixar ele escolher. Itens **[DIREÇÃO]** são ideias a validar, não decisões.

**Contexto**: hoje o Anchor consome o Lume (API financeira, repo separado) como sidecar (Fases
14/17/21). **O Lume será desfeito.** O dono do projeto quer poucos repositórios, organização máxima
e simplicidade ("um aplicativo central por área") — o Anchor é o aplicativo central da área
financeira, e deve ser um **núcleo robusto de informações financeiras** (quanto mais dado de
qualidade, melhor). Nenhum projeto dele tem usuários ainda: o momento é de acertar a arquitetura
agora pra não sofrer na hora de escalar.

**Decisão tomada**: **o Anchor deve ser totalmente autossuficiente, sem depender do repositório do
Lume.** As capacidades financeiras que viviam no Lume passam a viver no Anchor.

**Requisitos de intenção**:
1. **Uso sem interface (headless)** — poder usar a API/capacidades de dados financeiros **sem a UI
   do app**. "API financeira sem o aplicativo" é um caso de uso legítimo e distinto.
2. **Consumo por outros projetos** — o **Warden** (agente de IA) deve poder consumir as informações
   financeiras do Anchor. A intenção anterior era via MCP; confirmar se continua fazendo sentido.
3. **Self-host e hospedagem** — o mesmo código roda localmente (grátis) e, no futuro, hospedado
   pelo dono do projeto (pago). Princípio do ecossistema: o grátis/self-hosted sempre funciona com
   qualidade padrão; o pago é conveniência ou qualidade extra, **nunca funcionalidade básica
   trancada**.
4. **Fontes de dados plugáveis** — havia a decisão de uma interface `DataProvider` com múltiplas
   implementações (fonte grátis por padrão, BYO-key pra provedor pago, dado premium reservado ao
   tier hospedado). Verificar se ainda cabe e como se encaixa (análogo ao `StorageProvider` da
   Fase 15).
5. **Sem custo pro dono do projeto na versão gratuita/local.**

**[DIREÇÃO] Organização interna** (hipótese, não decisão): separar, **dentro do mesmo repositório**,
o núcleo de dados (sem dependência de UI), a camada de API/servidor e o app de interface, de modo
que uma eventual extração futura de qualquer parte seja simples. O formato real pode ser outro —
discutir com o dono do projeto dado o código existente.

**Preferências do dono do projeto que guiam as decisões**: mínimo de repositórios possível
(centralizar por organização, descentralizar só quando necessário); evitar "pontas soltas" e
desorganização; arquitetura pensada pro futuro, sem retrabalho pra escalar; respostas técnicas e
diretas, **com tradeoffs apresentados antes de implementar**.

**Questões abertas** (a discutir com o dono do projeto):
- Como o Anchor expõe suas capacidades pro Warden (e pra quem usar só a API): MCP, HTTP, ambos?
- Como migrar o que o Lume tem hoje sem quebrar o que já funciona (Fases 14/17/24 dependem dele —
  `finance_api::*`, `/v1/rates`, `/v1/tax-rules/br`).
- O que fazer com o mecanismo atual de sidecar e a detecção de instância compartilhada (a spec
  acha que **provavelmente some**, mas pede confirmação) — afeta diretamente a **Fase 21**.
- Onde ficam armazenamento e cache de dados financeiros.
- Quais partes de auth/storage/entitlement são comuns ao Warden e como evitar duplicação sem criar
  repositório novo.

**Fora de escopo** (da spec): mudanças no Warden ou no TruthID; criar novos repositórios; apagar ou
arquivar o Lume (decisão exclusiva do dono do projeto, depois de validar).

**Relação com outras fases**:
- **Fase 21** (sidecar do Lume com fallback pra instância do Warden) — **provavelmente superada**
  por esta fase; não implementar o fallback antes de decidir aqui.
- **Fases 14/17/24** — hoje dependem do Lume (Finance API); são o que precisa ser migrado.
- **Fase 25** (web app/modo servidor) — a camada de API/servidor e o modo headless desta fase se
  sobrepõem a ela; decidir juntas.
- **Fase 20** (IA removida) — o Warden consumir dados do Anchor é a "ponte" que ficou pendente lá.
- **Fase 15** (`StorageProvider`) — mesmo padrão de abstração provável pro `DataProvider`.

**Plano aprovado na Sessão 107** (plan mode; detalhe em `~/.claude/plans/fuzzy-humming-crystal.md`):
- **Pula o passo "mover o Python"** da Opção C do Lume: vai direto pra Rust, um domínio por vez; o
  Lume segue como sidecar pro que ainda não foi portado e só é arquivado no fim.
- **Cache no `anchor.db`** (some o `finance_api.db`); **`DataProvider` já na 1ª fonte portada**
  (traits por categoria, cascata, proveniência); **MCP e HTTP pro Warden: os dois, decidir depois**
  (junto da Fase 25); **câmbio e opções/gregas adiados** (Fases 19 e 18).
- **Fachada preservada**: `FinanceApiHandle` e os métodos de `client.rs` ficam; cada domínio troca
  HTTP por nativo sem mexer nos `collect_*`. Etapa 0 = `finance_core/` (HTTP, erros `NoData` ×
  `ProviderUnavailable`, traits, frescor/stale).
- **Ordem**: 1 macro BCB + tax-rules; 2 família Yahoo (+ bolsai); 3 índices B3 + curva DI; 4 cripto
  restante (8 indicadores, fear-greed, CoinGecko); 5 CVM DFP/IPE; 6 FII; 7 SEC EDGAR; 8 aposentar
  sidecar, `externalBin`, passos Python do `build.yml` e `.lume-version`.
- **Estado de execução (Sessão 107)**: **Etapa 0 e domínio 1 feitos** (`finance_core/`: `http`, `provider`
  com `cascade`, `cache` em memória com `stale`, `bcb_sgs` com `MacroSeriesProvider`, `tax_rules`). Em
  `client.rs`, `fetch_benchmark_series`, `fetch_selic` e `fetch_tax_rules_br` agora são nativos (o
  `_handle` fica até a aposentadoria). Erros novos `NoData` e `ProviderUnavailable`; `skip_not_found`
  aceita os dois. O cache de frescor ficou **em memória** (só pro que não é gravado), não em tabelas
  novas: as tabelas do Anchor já têm `fetched_at`/`source`. O teste ao vivo contra o Lume de tax-rules
  virou teste offline (tabela embarcada × seed da migration). **Selic persistida**: o BCB é lento e oscila (a série diária inteira levou de 46 s a 139 s
nos testes ao vivo), então a Meta Selic vive em `selic_daily` no `anchor.db` (migration
`m20261016_150000`, `finance_api::selic_store`): baixa tudo na primeira vez, depois só o trecho novo
(TTL de 1 h) e, com o BCB fora do ar, serve o gravado como `stale`. `client::fetch_selic` passou a
receber `&DatabaseConnection` em vez do handle. O `TtlCache` ficou sem chamador até a curva DI
(domínio 3). **Domínio 2 feito (Sessão 107)**: `finance_core::{market, yahoo, metals, bolsai, api_keys}`.
`MarketDataProvider` (cotação, histórico 10 anos, técnicos, média de dividendos, pagamentos) com
`Yahoo` como único provider por ora; `Instrument { ticker, market: B3 | Global }` decide o símbolo
(`.SA` só na B3). Nativos no `client.rs`: ações B3 (6 métodos, inclusive a bolsai), partes de mercado
das ações EUA (5), metais (2) e, por tabela, IBOV/IVVB11 do benchmark. **Paridade com o Python do
Lume conferida em PETR4**: SMA 50/100/200, CAGR 5/10 anos, média de dividendos, 34 pagamentos e as
2492 cotações batem (diferença só no último dígito do ponto flutuante). **Mudança consciente**:
símbolo desconhecido no Yahoo (404) agora é `NoData` e o coletor pula o ticker; o Lume devolvia 502
e derrubava o lote. **bolsai é o primeiro BYO-key**: a chave vem do keyring (`anchor` /
`bolsai_api_key`) e depois da variável `BOLSAI_API_KEY`; sem chave o erro diz o que configurar.
Achado: o sidecar de release nunca recebia `BOLSAI_API_KEY` (o spawn só passava `API_KEYS` e
`DATABASE_URL`), então fundamentos e resolução de FII só funcionavam em dev. **Falta**: tela em
Settings pra colar a chave (hoje só keyring/ambiente) e testar com uma chave real, que não tenho.
**Domínio 3 feito (Sessão 107)**: `finance_core::{b3_index_stats, b3_taxa_swap}` (IFIX/SMLL/IDIV e a
curva DI×PRE) com `IndexHistoryProvider` e `DiCurveProvider`; `fetch_b3_index_history` e
`fetch_di_futures_curve` do `client.rs` agora são nativos. Nova dependência `zip` (vai servir também
a CVM e ao COTAHIST). A curva usa o `TtlCache` (1 h, inclusive dias sem pregão, que viram `NoData`),
porque a Home volta dia a dia procurando a última curva. **Achado ao vivo**: no arquivo real da B3 os
offsets do diretório central são relativos ao blob `.ex_` inteiro (o zip interno começa no byte
86036 e é lá que o diretório central aponta), então o blob todo abre como zip; a crate `zip` não
tolera a ambiguidade que o Python tolerava, por isso tenta o blob inteiro e, se falhar, o recorte a
partir de `PK\x03\x04` (layout do teste do Lume). **Paridade com o Python do Lume conferida**:
curva de 15/10/2026 com os mesmos 281 vértices e o mesmo primeiro vértice (13,65%), e IFIX com a
mesma última cotação (31/08/2026, 3762,79). **Domínio 4 feito (Sessão 107)**: `finance_core::{coingecko, fear_greed, eth_indicators}`. Nativos no
`client.rs`: `fetch_eth_indicator` (os 8 do Lume: `tvl-trend`, `net-issuance`, `fees-vs-emission`,
`nvt-ratio`, `mvrv-z-score`, `puell-multiple`, `exchange-netflow`, `active-addresses-trend`),
`fetch_fear_greed`, `fetch_crypto_quote` e `fetch_crypto_price_history`. Providers: `CryptoMarketProvider`
(CoinGecko), `FearGreedProvider` (alternative.me) e `EthIndicatorProvider`, em que cada fonte declara
os códigos que atende (`supports`) e a cascata só consulta quem atende; código desconhecido é `NoData`.
Cotação e histórico saem da mesma chamada `market_chart`, com cache de 5 min na memória (a CoinGecko
grátis limita as chamadas) e resolução símbolo→id em cache de 24 h; símbolo sem correspondência exata
é `NoData`. **Paridade com o Python do Lume conferida**: os 8 indicadores batem até o último dígito
(ex.: MVRV Z 0,12189, Puell 1,06706, TVL 1,91299%) e o Fear & Greed lê o mesmo dia. **O Lume já não é
chamado por nenhum caminho de cripto.** `crypto_sources.rs` (Fase 31) segue com seu próprio
`get_text`; unificar com `finance_core::http` fica como arrumação futura. **Domínio 5 feito (Sessão 107)**: `finance_core::{cvm_dfp, disk_cache}`. `fetch_company_roe`,
`fetch_company_payout` e `fetch_company_dcf_fundamentals` do `client.rs` agora são nativos, lendo os
zips anuais da CVM com a crate `csv` (linhas filtradas por `CD_CVM` na leitura, parsing em
`spawn_blocking`, download atrás de `ZipFetcher` pra os testes montarem zips em memória). Os zips ficam
em disco (`app_cache_dir()` do SO, definido no `setup` por `disk_cache::init`; escrita atômica; os dois
anos fiscais mais recentes expiram em 30 dias, os mais antigos nunca; cópia velha serve se o download
falhar). A primeira coleta baixa até 6 zips (~13 MB cada). `dividend-notices` (IPE) **não foi portado**:
o Anchor nunca teve método nem chamador. **Paridade com o Python do Lume**: PETR, VALE, BBAS e ITUB
com `roe`, `payout_avg_5y` e os 9 campos do DCF **idênticos até o último dígito** (inclusive bancos
dando "sem DCF"). **Divergência consciente, bug do Lume corrigido**: o `_find_by_keyword` do Python
aplicava `abs()` sempre, então um prejuízo virava lucro (ROE positivo pra empresa com prejuízo) e os
guardas `lucro <= 0` e `patrimônio <= 0` nunca disparavam; lucro e patrimônio agora mantêm o sinal
(`Sign::Keep`) e o módulo fica só pras saídas de caixa (dividendos, capex, D&A). Só muda o resultado
em empresas com prejuízo ou patrimônio negativo.

**Domínio 6 feito (Sessão 107)**: `finance_core::cvm_fii` (+ `bolsai::fii_summary`).
`fetch_fii_monthly_indicators`, `fetch_fii_properties` e `fetch_fii_cnpj_resolution` do `client.rs`
agora são nativos. Os zips (`inf_mensal_fii_{ano}` e `inf_trimestral_fii_{ano}`, 1 a 2,5 MB) usam o
`disk_cache` e o ano corrente, com o ano anterior só se o corrente não existir. **Melhoria sobre o
Lume**: o Lume guardava o zip do ano corrente em disco para sempre (um zip baixado em janeiro nunca
recebia os meses seguintes); aqui o do ano corrente expira em 3 dias e os de anos anteriores em 30.
A resolução ticker→CNPJ exige administrador **e** nome idênticos (espaços e caixa normalizados), e
zero ou mais de um candidato é `NoData`; o cadastro `geral` repete o fundo uma vez por mês, então
candidatos são contados por CNPJ distinto. **Paridade com o Python do Lume** no HGLG11: indicadores
mensais idênticos campo a campo e os mesmos 37 imóveis (a ordem muda: o Rust ordena por nome, como
o serviço do Lume fazia). **Achado**: o HGLG11 hoje se chama "PÁTRIA LOG" no cadastro da CVM; o nome
das fixtures do Lume ("CSHG LOGISTICA") está desatualizado. A resolução com a bolsai não foi testada
ao vivo (sem chave); o casamento contra o `geral` real foi.

**Domínio 7 feito (Sessão 107)**: `finance_core::sec_edgar`. `fetch_us_stock_fundamentals`,
`fetch_us_stock_dcf_fundamentals`, `fetch_us_stock_payout` e `fetch_reit_fundamentals` do `client.rs`
agora são nativos, lendo o XBRL da SEC por tag (`companyconcept`), com throttle global de 0,11 s (um
`Mutex<Instant>` reserva o horário e a espera fica fora do lock), cache de 1 h por `(cik, tag)` em
`TtlCache` (fundamentos, payout e DCF do mesmo ticker repetem tags) e a lista de tickers em disco por 7
dias. Regras do Lume mantidas: linha anual = 10-K/FY com janela de 350 a 380 dias, retificação pelo
`filed` mais recente, `try_tags` pela linha mais recente entre todas as tags, instantâneos com rank 0 e
1, milhões de dólares. **Credencial nova**: a SEC exige um e-mail de contato no `User-Agent`; ele vem do
keyring (`anchor` / `sec_edgar_contact_email`) ou de `SEC_EDGAR_CONTACT_EMAIL`, nunca embutido no
código. O sidecar de release nunca recebia essa variável, então os fundamentos dos EUA só funcionavam em
dev; agora sem o e-mail o erro diz o que configurar. **Paridade com o Python do Lume**: AAPL, JPM e O
(fundamentos, payout, DCF e REIT) idênticos até o último dígito, inclusive JPM e O dando "sem DCF" e o
REIT devolvendo imóveis só para o O. Cerca de 6 s por ticker em build de debug (14 s no primeiro, com a
lista de tickers). **Todos os 7 domínios de dados estão nativos.**

**Domínio 8 feito (Sessão 107): sidecar aposentado, Fase 26 concluída no código.**
- **Removido**: `finance_api/sidecar.rs` e o `FinanceApiHandle`, que atravessava ~40 funções sem fazer
  nada (o parâmetro saiu de `client.rs`, de todos os `collect_*` e dos comandos Tauri); o HTTP do
  `client.rs`; `tauri-plugin-shell`, `bundle.externalBin` e `shell:allow-execute`; as variantes
  `FinanceApi` e `FinanceApiNotFound` do `AppError`; `.lume-version`; os passos Python, "Checkout Lume" e
  PyInstaller do `build.yml` e os placeholders do binário nos dois CIs; a tela Local/Remote e a tabela
  `finance_api_settings` (nova migration `m20261016_160000`, com `down` que recria a tabela vazia). Os
  DTOs do `client.rs` deixaram de ser `Deserialize` (viraram só o contrato entre fontes e coletores).
- **Novo**: Settings → **Data sources** (substitui "Finance API"): a chave da bolsai e o e-mail de
  contato da SEC, gravados no keyring do SO por `finance_core::api_keys` (`status`, `store`, `clear`,
  validação de entrada) e expostos por `list/set/clear_data_source_credential`. A tela nunca recebe um
  segredo de volta, só de onde ele vem (keyring, variável de ambiente ou nada); o e-mail volta porque não
  é segredo. Isso fecha o problema de que bolsai e SEC só funcionavam em dev.
- **Também**: o campo `lume_value` de `TaxRuleConflict` virou `official_value` (backend e tela), e os
  textos "Couldn't reach Lume" / "from the Finance API" foram trocados.
- **Verificado**: `./dev.sh check` sem warnings (tsc + `cargo check --all-targets`, sem o plugin shell e
  sem `externalBin`); suíte inteira 400 passam, 0 falham (inclui as migrations do zero e 10 testes novos
  de credenciais). **Não verificado**: o `build.yml` sem Python nas 3 plataformas (só roda em tag) e o E2E
  novo da seção, que dependem do CI; e o teste de clique em Settings → Data sources (salvar e remover no
  keyring do SO).
- **Estado do Lume**: o repo não foi tocado. Quando o dono do projeto validar o Anchor sozinho, a decisão
  de arquivar ou apagar é dele. Fases 14 e 21 ficam **superadas** por esta fase.

**Verificação automática da Fase 26 (Sessão 108)**: o dono do projeto não quer teste de clique manual
("não tínhamos automatizado esse tipo de teste, eu não gosto de fazer"), então o que dava foi para o CI.
- **`build.yml` sem Python provado**: tag de teste `v0.0.0-ci-test` rodou o Build com sucesso em Windows
  (`.msi`), Linux (`.deb`), macOS (`.dmg`) e no job `apk`. Release em rascunho e tag apagados depois
  (conferido: nenhuma release, nenhuma tag, remota ou local).
- **E2E novo `desktop/e2e/specs/lookup.e2e.mjs`** (rede real): B3 (PETR4), EUA (AAPL) e cripto (ETH) pela
  tela de Research; confere a tela montada e a cotação gravada no banco. Pula em vez de falhar se a fonte
  responder 429 (o Yahoo costuma limitar IPs de runner do GitHub; nas execuções do CI isso não ocorreu).
  FII e CVM (zips pesados) ficam só nos testes ao vivo do Rust.
- **Bug achado pelo E2E**: sem a chave da bolsai (B3) ou o e-mail da SEC (EUA) o coletor grava cotação,
  dividendos e técnicos e só então erra nos fundamentos; as seções de Research relêem o banco só em
  `onSuccess`, então a tela mostrava "No data found" com os dados já gravados. Trocado por `onSettled`
  em `StockAnalysis`, `Fii`, `Bdr`, `Etf`, `UsStock` e `Reit`; o aviso "add it in Settings → Data
  sources" continua visível. Cripto, metal e ETF dos EUA não passam pelo passo de fundamentos e ficaram
  como estavam. Antes da Fase 26 isso era mascarado: o sidecar de release nem recebia a chave.
- **Ainda sem cobertura automática**: backfill de Profitability, sync de Settings → Taxes, FII e metal
  pela tela, e salvar/remover credenciais em Data sources (o CI não tem secret service para o keyring).

**Estado anterior**: direção decidida (autossuficiência), **nenhuma exploração do código nem decisão de
arquitetura feita ainda**. Próximo passo: explorar o repo e discutir com o dono do projeto, em plan
mode, conforme a regra número um acima.

**Spec irmã: desmontagem do Lume** (arquivo solto `SPEC-lume-desmontagem.md`, trazido na Sessão 101,
removido da raiz). É a **outra metade** da Fase 26 e foi escrita pro agente que trabalha no repo do
**Lume** (não pro do Anchor) — registrada aqui pra este repo ter o contexto completo. Mesma regra
número um (não presumir nada, explorar, apresentar o achado, confirmar cada decisão de arquitetura,
opções com prós e contras; itens **[DIREÇÃO]** são ideias a validar).
- **Decisão**: o Lume (antes EasyBusiness) **será desfeito**. Motivo: o dono do projeto percebeu que
  trabalha em dois repositórios dentro de um único projeto, o que gera desorganização. Hoje o Lume é
  uma API de informações financeiras consumida só pelo Anchor, embarcada como sidecar.
- **Objetivo**: mapear com precisão o que existe no Lume e planejar com o dono do projeto a migração
  do que for necessário pro Anchor, de modo que ao final o Lume seja descartável **sem perda**.
- **O que o agente do Lume deve fazer**: (1) inventariar capacidades, fontes de dados,
  cache/armazenamento, contratos expostos e configuração; (2) identificar o que o Anchor consome e
  como (sidecar, fallback pra instância compartilhada) **verificando, sem presumir**; (3) propor com
  opções e tradeoffs o que migra, o que é reescrito e o que é descartado; (4) documentar a migração
  de forma que o agente do Anchor execute a outra metade com contexto.
- **Estado do repo do Lume após a migração**: permanece no GitHub por enquanto; o dono do projeto
  valida com o tempo que o Anchor funciona sem ele e **só então** arquiva ou apaga (decisão dele).
  Nada pode ser deletado ou arquivado sem confirmação explícita. Considerar um aviso no README do
  Lume ("em desmontagem" + pra onde foram as responsabilidades) — **confirmar antes**.
- **Questões abertas dessa spec**: alguma ideia/decisão antiga do Lume a preservar (padrão
  `DataProvider` com fonte grátis, BYO-key e premium)? O que acontece com a renomeação
  EasyBusiness → Lume, que ainda não havia sido aplicada no repo de lá (provavelmente perde o
  sentido — confirmar)? Algo além do Anchor depende do Lume?
- **Fora de escopo**: mudanças no Warden ou no TruthID; criar novos repositórios.
- **Observação**: ela diz que a renomeação "ainda não havia sido aplicada", mas este repo já usa
  "Lume" (`.lume-version`, `Arven-Labs/lume`, Fases 21/24). Ao desmontar, o nome é irrelevante, mas
  vale conferir o estado real do repo de lá em vez de confiar na spec.
- **Impacto aqui**: os pins (`.lume-version`, `build.yml`), o módulo `finance_api::*` (sidecar +
  client HTTP, Fase 14) e tudo que consome `/v1/*` (Fases 14/17/24) são o que precisa migrar. O
  inventário detalhado deve sair do lado do Lume; este repo executa a metade do Anchor.

### Fase 27 — Suporte a DeFi com maestria (ideia trazida pelo dono do projeto na Sessão 102, puro registro, não iniciada)

**Objetivo**: o dono do projeto "adoraria que o Anchor suportasse DeFi com maestria, gestão
completa e tudo mais". Intenção registrada em conversa, sem nenhum desenho de escopo, arquitetura
ou fonte de dado — nada foi explorado nem decidido.

**Pontos a explorar quando for desenhar** (hipóteses, não decisões):
- Como modelar posições DeFi (staking, liquidity pools, lending/borrowing, yield farming,
  restaking) frente ao modelo atual de `AssetClass`/transações (Fase 10) — classe nova, ou tipo de
  posição próprio, já que uma posição DeFi não é só "quantidade × preço".
- Fontes de dado: leitura on-chain por endereço de carteira, agregadores (DefiLlama já é fonte de
  TVL na Fase 2.3), preço de LP tokens, APY/APR, impermanent loss, rewards pendentes.
- Gestão completa: histórico de entradas/saídas, custo base com rewards como renda, saúde de
  posição alavancada (health factor/liquidação), multi-chain, custos de gas.
- Relações com outras fases: impostos (Fase 24, tratamento fiscal de rewards/swaps), moeda base
  (Fase 19), `DataProvider` plugável (Fase 26), TruthID/carteira (Fase 8).

**Estado**: puro registro de intenção. Seguir a regra das specs recentes: explorar e confirmar cada
decisão com o dono do projeto antes de propor arquitetura.

### Fase 28 — Biblioteca/SDK de integração (ideia trazida pelo dono do projeto na Sessão 102, puro registro, não iniciada)

**Objetivo**: criar uma biblioteca tipo SDK de integração do Anchor. O dono do projeto ainda **não
tem certeza em que sentido seria útil**, só sente que seria ("depois pensamos nisso"). Nada
desenhado.

**Pontos em aberto**: quem consumiria (outros apps do dono, como o Warden/TruthID, scripts de
terceiros?); o que expõe (dados da carteira, cálculos/valuation, coleta de dados); em que forma
(crate Rust, pacote TS/Python, cliente HTTP); relação com a Fase 25 (Anchor como servidor) e a
Fase 26 (headless, `DataProvider` plugável, consumo pelo Warden) — possivelmente o SDK nasce
dessas duas. Só faz sentido definir depois que o caso de uso aparecer.

**Estado**: puro registro de intenção.

### Fase 29 — Controle de empréstimos concedidos a terceiros (ideia trazida pelo dono do projeto na Sessão 102, puro registro, não iniciada)

**Objetivo**: registrar e acompanhar dinheiro que o usuário **emprestou a outra pessoa** (o oposto
do `Liability` da Fase 12, que é dívida do próprio usuário).

**Requisitos descritos pelo dono do projeto**:
- **Contabilizar ou não na carteira/patrimônio — escolha por empréstimo.** Caso real: empréstimo
  pra mãe, que sempre paga, e que o dono **não quer ver no patrimônio**. Então precisa de uma flag
  por empréstimo (entra / não entra nos totais de net worth).
- **Juros opcionais.** Pode haver cálculo de juros, mas **não é obrigatório nem tem taxa fixa**: o
  caso real é sem juros contratado, em que a pessoa às vezes devolve "um pouquinho a mais".
- **Baixa manual de valor livre.** Cadastrar um pagamento e dar baixa na dívida com o valor que o
  usuário escolher, maior ou menor que o recebido. Exemplo: recebeu R$ 1.050 sobre uma dívida de
  R$ 1.000 e dá baixa só de R$ 1.000 (ou outro valor), sem precisar de taxa de juros configurada.
  Ou seja, **valor recebido e valor baixado do saldo são campos independentes**; a diferença pode
  ser tratada como juros/bônus informal, ou ignorada.

**Pontos a decidir depois**: tabela própria ou reaproveitar `GeneralTransaction`/`BankAccount` da
Fase 12; como registrar a diferença entre recebido e baixado (receita, juros, nada); se o
devedor é entidade cadastrada ou só texto livre; vencimento, parcelas e alertas; como entra
nos relatórios quando a flag está ligada.

**Estado**: puro registro de intenção, discussão detalhada fica pra depois.

**Implementado na Sessão 106 (fatia 1)**, com as decisões do dono do projeto:
- **Conta bancária opcional**: com conta, o empréstimo gera saída (`emprestimo_concedido`) e cada
  baixa gera entrada (`recebimento_emprestimo`, no valor recebido) em Finanças Gerais; sem conta,
  só registro. `compute_balances` passou a tratar `emprestimo_concedido` como débito.
- **Recebido × baixado**: dois campos independentes em `loan_repayments`. A diferença só aparece
  como "extra recebido" no empréstimo, **não** vira receita sozinha. Recebido pode ser 0 (baixa
  sem dinheiro entrando).
- **Devedor como entidade** (`borrowers`), não texto livre. Não dá pra remover devedor com
  empréstimo.
- **Flag por empréstimo** `include_in_net_worth` (default ligado). `NetWorthSummary` ganhou
  `total_loans_receivable` (só os com flag ligada), somado ao patrimônio.
- **Juros opcionais**: taxa mensal nula = sem juros. Quando existe, são juros **simples** sobre o
  principal por meses completos (`domain::loan::accrued_interest`), informativos. Saldo a
  receber = principal + juros − Σ valor baixado, nunca negativo, sempre derivado.
- Código: migration `m20261016_120000_create_loans` (3 tabelas), `domain/loan.rs`,
  `commands/loan.rs`, aba **Finances → Loans** (`LoanSection.tsx`).
- **Fora desta fatia**: parcelas/vencimento/alertas, juros compostos, edição de principal e
  data, aviso de atraso. Teste de clique pendente.

### Fase 30 — Mais países e mais tipos de investimento (ideia trazida pelo dono do projeto na Sessão 102, puro registro, futuro, não iniciada)

**Objetivo**: dar suporte a cada vez mais países e mais tipos de investimento. Registrado como
direção de longo prazo ("isso fica para um futuro"), sem escopo, ordem nem prioridade definidos.

**Pontos de contato** (hipóteses): Fase 10 (`AssetClass`, multi-ativos e regiões), Fase 19 (moeda
base seletiva), Fase 24 (impostos hoje só do Brasil; cada país teria regra própria), Fase 26
(`DataProvider` plugável pra fontes de dado por país/mercado), Fases 18 e 27 (opções e DeFi seriam
tipos novos). Em aberto: quais países e classes primeiro, como regras fiscais e fontes de cotação
variam por país.

**Estado**: puro registro de intenção.

### Fase 31 — Indicadores cripto, trabalho pesado (ideia trazida pelo dono do projeto na Sessão 102, puro registro, futuro, não iniciada)

**Objetivo**: investir forte nos indicadores cripto, expandindo o que existe (score cripto da
Fase 3 e coleta da Fase 2.3). Dito como "depois vamos trabalhar pesado nisso", sem lista de
indicadores nem desenho.

**Ponto de partida**: dos 9 indicadores atuais, 8 já são automáticos (CoinMetrics, DefiLlama,
ultrasound, CoinGecko — ver Fase 2.3) e só `staking_yield` seguia manual por falta de fonte
gratuita (**automatizado na Sessão 106**, ver "Implementado" abaixo).
Hoje a cobertura é centrada em ETH. Em aberto: quais indicadores novos, outros ativos além de ETH,
fontes pagas ou gratuitas, relação com DeFi (Fase 27) e backtesting (Fase 22).

**Estado**: puro registro de intenção até a Sessão 106 (ver abaixo).

**Estudo de fontes, Sessão 106 (2026-10-16)**, testado ao vivo, todas sem chave:
- **Binance Futures** (OKX e Bybit também respondem): funding atual e histórico, open interest
  histórico (só ~30 dias), razão long/short de contas (só ~30 dias).
- **Deribit** (opções): DVOL em série diária e put/call por open interest.
- **DefiLlama**: oferta de stablecoins; yields (APY do stETH 2,26%, rETH 2,06%, cbETH 2,31%).
- **CoinMetrics Community**: 31 métricas liberadas para ETH e BTC, incluindo `CapMVRVCur`,
  `FlowIn/OutEx`, `IssTotUSD/Ntv`, `AdrActCnt`, `TxCnt`, `HashRate`, `SplyExNtv/USD`, `PriceBTC`.
  **Não tem** SOPR, realized cap nem NVT ajustado. A Talos (dona) avisou que vai aposentar
  métricas do Community sem dizer quais, vale health check. **Licença não comercial**, relevante
  para a versão hospedada paga da spec de autossuficiência.
- **bitcoin-data.com** (só BTC): SOPR (geral/STH/LTH), NUPL, realized price, MVRV Z, Puell,
  reserve risk, NVTS. Plano grátis: 10 req/h e 15/dia, 4 anos de histórico e **atraso de 7 dias**;
  uso comercial não esclarecido.
- **Sem fonte grátis**: fluxo de ETFs de BTC/ETH (Farside e SoSoValue atrás de Cloudflare),
  liquidações (Coinglass, pago).

**Implementado na Sessão 106 (1º lote)**, decisões do dono do projeto: subscores por grupo,
primeiro lote = `staking_yield` automático + derivativos de ETH, coleta **em Rust no Anchor** (os
8 do Lume seguem pelo sidecar; a spec manda desfazer o Lume):
- Novo `crypto_sources.rs` com cálculos puros testados contra JSON real e `fetch_*` finos:
  `funding_rate_7d` (média de 21 funding de 8h anualizada em %), `open_interest_change_30d`
  (variação % do OI **em ETH**, sem efeito preço), `long_short_ratio` (média 7d) e
  `staking_yield` real (APY do stETH menos a inflação bruta `IssTotNtv`×365÷`SplyCur`).
- Seeds calibrados com valores reais do dia (migration `m20261016_130000`): funding 5/20, OI
  0/25, long/short 2,0/3,5, todos "menor é melhor". São ponto de partida, **não há editor de
  thresholds na UI** (só SQL), vale uma fatia própria.
- Score agora vive no backend (`domain::crypto_score::compute_score`, antes só no TS): 3 grupos
  (On-chain 5, Network fundamentals 4, Derivatives 3 = **12** indicadores), subscore por grupo e
  veredito por percentual (≥70% tese intacta, 40–70% neutro, <40% reduzir risco, equivalente às
  faixas 7-9/4-6/0-3 dos 9 originais). Novo comando `get_crypto_score`.
- `collect_eth_indicators` agora tolera falha parcial (uma fonte fora do ar, ex. Binance
  bloqueada na região, não derruba as outras) e só dá erro se nada foi coletado.
- **Fora desta fatia**: suporte a BTC e o que o lote 2 cobriu (ver abaixo). Teste de clique
  pendente.

**Lote 2 (Sessão 106)**: opções, stablecoins e fallback de região. Total vira **15 indicadores em
4 grupos** (On-chain 5, Network fundamentals 4, Derivatives & options 5, Market liquidity 1).
- `dvol`: último fechamento do DVOL do ETH (Deribit). Menor é melhor, seed 55/75 (faixa real
  46–75 em 200 dias, mediana 53).
- `put_call_ratio`: Σ open interest de puts ÷ Σ de calls das opções ETH (Deribit). Maior é
  melhor (medo é oportunidade, calma demais é topo), seed 0,7/0,4 (hoje 0,52).
- `stablecoin_supply_change_30d`: variação % em 30 dias da oferta total de stablecoins
  (DefiLlama). Maior é melhor, seed 2/0 (faixa de 1 ano: −2,8% a +5,8%). Base = último ponto
  **até** 30 dias antes do mais recente (a série tem falhas, não dá pra contar 30 posições).
- **Fallback Bybit** para funding, open interest e long/short: tenta a Binance e, se falhar (rede,
  HTTP ou bloqueio de região), a Bybit; o `source` gravado na leitura diz qual respondeu e o erro
  só sobe se as duas falharem. **Armadilha achada ao vivo**: a Bybit devolve as séries do mais
  novo pro mais antigo (a Binance é o contrário), o parser inverte, com teste. Erros da Bybit
  vêm com `result: {}`, então `list` tem valor padrão pra mensagem do `retMsg` aparecer. OKX ficou
  de fora de propósito (um fallback já cobre o risco de região).
- Migration `m20261016_140000_seed_crypto_options_liquidity_thresholds`; `DEFAULT_THRESHOLDS`
  (Reset do editor) passou a 15, **duplicando os seeds** das 3 migrations.
- Fixtures dos testes: Bybit, DVOL e stablecoins no formato real de 2026-10-16; a lista de opções
  e os pontos de stablecoin do teste de janela têm valores de exemplo na forma real.

**Lote 3 (Sessão 111)**: só indicadores novos do ETH, escolha do dono do projeto (sem score de BTC, que
exigiria `indicator_thresholds` por moeda, hoje único por indicador, e coletores parametrizados). Total
**17 indicadores em 4 grupos** (On-chain 6, Network fundamentals 4, Derivatives & options 5, Market
liquidity 2). Fonte: CoinMetrics Community, sem chave.
- `exchange_supply_change_30d` (On-chain): variação % em 30 dias de `SplyExNtv` (ETH em exchanges). Menor é
  melhor, seed −1,0/1,5 (faixa real −3,4% a +1,7%, mediana −1,8%). O último ponto vem como dado
  provisório (`flash`) e pode ser revisado.
- `eth_btc_trend_30d` (Market liquidity): variação % em 30 dias de `PriceBTC`. Maior é melhor, seed
  3,0/−3,0 (faixa real −8,4% a +11,4%, mediana +4,1%, hoje −6,4%).
- Os dois reusam `pct_change_back` de `eth_indicators.rs`; códigos `exchange-supply-trend` e
  `eth-btc-trend`; migration `m20261017_100000`; seeds duplicados em `DEFAULT_THRESHOLDS` (convenção).
- Calibragem com 70 janelas reais de 30 dias (o plano Community devolve só ~100 dias por consulta).
- **Achado de processo**: a contagem "15" estava espalhada em testes (`flow_tests.rs`, 2 specs E2E, totais
  por grupo em `crypto_score.rs`) além do código; meu grep inicial pegou só parte e o CI pegou o resto.
  Ao mudar o número de indicadores, buscar `15` em `e2e/` e `flow_tests.rs`.

**Fluxo de ETFs, rechecado na Sessão 113 (2026-10-17), segue sem fonte utilizável**; decisão do dono do
projeto: manter bloqueado. Testado ao vivo: Farside e SoSoValue respondem 403 (Cloudflare); a API da
CoinGlass exige chave ("API key missing"); The Block publica o dataset do BTC (licença nos termos de serviço
deles), mas `data.tbstat.com` responde `AccessDenied` e não há endpoint aberto; o Bitbo
(`bitbo.io/treasuries/etf-flows/`) responde 200 com uma **tabela HTML só de BTC** (IBIT, FBTC, GBTC...),
sem API e sem licença esclarecida. **Para ETH não achei nenhuma fonte.** Alternativas descartadas por
ora: indicador manual (`etf_flow_7d`, sem coletor) e raspagem do Bitbo (só BTC, frágil). Só vale reabrir se
surgir API grátis ou se o dono assinar uma fonte paga.

**Score próprio do BTC (Sessão 112)**, decisões do dono do projeto: limites **por moeda** e só os
**análogos diretos** do ETH. **12 indicadores em 3 grupos** (On-chain 6, Derivatives & options 5, Market
liquidity 1; sem Network fundamentals). Ficam fora, por serem do ETH: `staking_yield`, `tvl_trend`,
`net_issuance`, `fees_vs_emission`, `eth_btc_trend_30d`.
- **Schema**: `indicator_thresholds` ganhou `coin` (`UNIQUE(coin, indicator)`), migration
  `m20261017_110000` recria a tabela (SQLite não troca constraint), copia as linhas como `ETH` e semeia as
  12 do BTC. Só ETH e BTC têm conjunto próprio; **qualquer outra moeda usa o do ETH**
  (`domain::crypto_score::threshold_coin`), preservando o registro manual de, por exemplo, SOL.
- **Seeds do BTC** (verde/vermelho), calibrados com dados reais de 2026-10-17: MVRV Z 0/7, NVT 0,9/1,3,
  Puell 0,5/4, endereços ativos 0/−10, netflow −0,02/0,03 (faixa estreita, só 100 dias de dado), oferta nas
  exchanges −1/2, funding 5/15, OI 0/25, long/short 1,3/2,0, DVOL 40/55, put/call 0,7/0,4, stablecoins 2/0.
  Duplicados em `BTC_DEFAULT_THRESHOLDS` (convenção: mudar um exige mudar o outro; o teste de fluxo confere).
- **Domínio**: `IndicatorGroup::indicators(coin)`, `for_coin`, `compute_score(coin, ..)`,
  `default_threshold(coin, indicator)`; o ETH segue com os mesmos 17.
- **Comandos**: limite buscado por `(threshold_coin(coin), indicador)`, incluindo a validação do alerta
  (`alert_rule.rs`, que não filtrava moeda). `apply_threshold(coin, ..)` reclassifica só as leituras que usam
  aquele limite: editar o BTC mexe só no BTC, editar o ETH mexe nele e nas demais moedas.
  `list_indicator_thresholds`, `reset_indicator_threshold` e `run_crypto_collector` recebem `coin`.
- **Coleta**: `collect_eth_indicators` virou `collect_indicators(db, coin)`; derivativos usam `{coin}USDT`
  (Binance, fallback Bybit) e `currency={coin}` na Deribit; `eth_indicators::indicator(coin, code)` consulta o
  CoinMetrics com `assets={coin}` e o NVT com o id CoinGecko (`bitcoin`/`ethereum`). Código só do ETH pedido
  para outra moeda é `NoData`, sem rede.
- **Tela**: ETH e BTC têm botão do coletor, formulário manual e totais próprios; o editor de limites recebe a
  moeda.
- **Verificação ao vivo antes de codar**: CoinMetrics (`assets=btc`), Binance, Bybit e Deribit respondem
  para BTC; o formato dos nomes das opções da Deribit é o mesmo do ETH.
- **Erro de contagem no plano**: o plano dizia 13 indicadores, mas 6+5+1 são 12; o código e os testes usam 12.

**Editor de thresholds (Sessão 106, fatia 2)**: botão "Edit thresholds" na seção do score cripto
abre `ThresholdEditorDialog` com uma linha por indicador (verde, vermelho, direção inferida ao
vivo, Save e Reset). Comandos `list_indicator_thresholds`, `update_indicator_threshold` e
`reset_indicator_threshold`. **Decisão de projeto**: salvar **reclassifica todas as leituras já
guardadas** daquele indicador numa transação (`raw_value` intacto, só o `signal` muda), porque o
score e os alertas (`alert_checker`) leem o sinal gravado; sem isso ficariam com a regra antiga
até a próxima coleta. Valida limites finitos e diferentes (`EqualThresholds`). Os valores de
fábrica do Reset ficam em `domain::crypto_score::DEFAULT_THRESHOLDS`, **duplicando** os seeds das
migrations (mudar um seed exige mudar os dois). Testes: domínio (defaults válidos e cobrindo os 12
indicadores, validação, direção) e 3 testes com SQLite em memória da reclassificação, do erro sem
efeito e do Reset.


### Fase 32 — Inventário de ideias herdadas do Lume (levantado na Sessão 107, puro registro, nada decidido nem iniciado)

**Origem**: o dono do projeto pediu pra ler o repo do Lume (`../lume`, `git pull` já em dia) e
registrar aqui tudo de interessante na linha financeira, pra não se perder com a desmontagem. Lidos:
`project/OVERVIEW.md`, `ROADMAP.md`, `DESMONTAGEM.md`, `PENDING.md` e trechos de `ARCHITECTURE.md` e
`PHASE.md`. **Tudo abaixo é ideia a debater**; várias já têm o dado pronto no Lume e só falta
consumir ou portar. Ligada à **Fase 26** (decisão de migração) e à **Fase 31** (cripto).

**Estado da desmontagem no Lume (para a Fase 26 não recomeçar do zero)**: o Lume registrou em
`DESMONTAGEM.md` um inventário completo (13 routers, ~25 fontes, 38 tabelas, 41 arquivos de teste,
13 hosts externos) e a **Opção C (híbrido faseado) foi escolhida na Sessão 23 do Lume**: (1) mover o
`api/` Python pro repo do Anchor, (2) reescrever em Rust por domínio na ordem de uso (macro/BCB →
ações BR → CVM/FII → EUA/SEC → cripto/metais/índices), cada um com teste de paridade contra o
sidecar Python, (3) aposentar o sidecar. A execução foi **adiada** ("não vamos fazer agora", Sessão
24 do Lume). As decisões ortogonais seguem em aberto: onde guardar o cache (`finance_api.db` próprio
vs. dentro do `anchor.db`), como expor ao Warden (MCP, HTTP ou ambos), o que fazer com os domínios
sem consumidor e se o `DataProvider` nasce já na migração. **Divergência resolvida na Sessão 107**: o inventário do Lume (Sessão 22) marcava taxas/DI e tax-rules
como sem consumidor, mas o Anchor usa os dois (Home, Research, Settings → Taxes). Só câmbio e opções
não têm chamador (nem método no `client.rs`).

#### A. Dados que o Lume já tem e o Anchor ainda não usa (ganho barato, o dado existe)

- **Opções: catálogo de séries + último preço EOD + gregas** (`options/{underlying}/series`,
  `options/{series}/greeks`; B3 Séries Autorizadas + COTAHIST, Black-Scholes próprio em `math` puro).
  É a base de dados da **Fase 18** (gestão de opções). Limites confirmados: não existe fonte grátis
  pra bid/ask, open interest nem IV; o preço do ativo-objeto é informado por quem chama (decisão
  pra evitar grega errada por classe de ação). O COTAHIST anual leva ~2 min na primeira chamada.
  **Pendência P2 do Lume**: a raiz da opção nem sempre é o ticker sem o dígito (Embraer: `EMBR3` →
  raiz `EMBJ`); sem mapa genérico, tratado caso a caso. Ao portar, trazer esse mapa.
- **Curva DI futuro (PRE) e Meta Selic diária** (`rates/di-futures-curve/{data}`, `rates/selic/{code}`;
  B3 "Pesquisa por Pregão" + BCB 432): alimenta marcação a mercado de renda fixa prefixada, desconto
  de DCF com taxa livre de risco e o widget de juros da Home.
- **Câmbio multi-moeda** (`currencies/{par}/quote`, `price-history`; Yahoo `{BASE}{QUOTE}=X`,
  validado até em cross não-BRL como `GBPJPY=X`): base da **Fase 19** (moeda base seletiva) e da
  **Fase 30** (mais países).
- **Índices B3** (IFIX, SMLL, IDIV) e **metais** (ouro, prata etc.): benchmarks e classe de ativo.
- **FII**: indicadores mensais e imóveis por CNPJ (CVM) e `resolve/{ticker}` (bolsai + CVM). Pendência
  P1: `properties` devolve `data: []` tanto pra CNPJ desconhecido quanto pra FII de papel; o consumidor
  trata `[]` como "sem imóvel", nunca como erro.
- **EUA/SEC EDGAR**: fundamentos, DCF, payout e `reit-fundamentals` (FFO/AFFO não existem como tag
  XBRL, mas receita/patrimônio/LPA/lucro dão pra automatizar). Exige `SEC_EDGAR_CONTACT_EMAIL` no
  User-Agent. Útil à **Fase 30**.
- **Avisos de dividendos da CVM (IPE)** (`companies/{cvm}/dividend-notices`): fonte natural pra
  alimentar o "provento esperado" (Sessão 77.2) sem depender de plano pago.

#### B. Ideias de produto novas, do `ROADMAP.md` do Lume

- **`DataProvider` + `ProviderRegistry`** (grátis por padrão → BYO-key → premium só no hospedado).
  Interfaces por categoria (`QuoteProvider`, `FundamentalsProvider`, `CryptoScoreProvider`), cascata
  por prioridade, chave do usuário no keyring do SO, cache ortogonal ao provider, e contrato de erro
  que separa **"sem dado"** de **"provider caiu"**. Hoje cada service chama a fonte direto; já existem
  cascatas hardcoded (bolsai × Yahoo pra ação B3; CoinGecko/CoinMetrics/DefiLlama em cripto; Binance
  × Bybit, que a Fase 31 acabou de fazer). **Proveniência**: gravar qual provider gerou cada linha e
  nunca sobrescrever dado premium por grátis. Mesmo padrão do `StorageProvider` (Fase 15).
- **Ingestão proativa por agendamento**: separar *ingestão* (buscar e gravar em cadência própria) de
  *serving* (a UI só lê do banco). Cadências: intraday pra cotação (1–5 min), diária pós-pregão pra
  EOD/curva DI/índices, baixa frequência pra fundamentos e macro mensal. Cache-through vira rede de
  segurança pra ticker nunca visto. Pede tabela de observabilidade (`ingestion_runs`: fonte, início,
  fim, status, linhas, erro) pra detectar fonte que parou sem ninguém reclamar. No Anchor desktop o
  "agendador" seria uma tarefa Tokio em segundo plano, não um worker separado.
- **Hierarquia comum de erros** (`NoData` × `ProviderUnavailable`): hoje espalhada por service no Lume
  (`NoDividendDataError`, `TickerNotFoundError`, `FundNotFoundError` × `YahooFinanceError`,
  `B3TaxaSwapError`); no Anchor, 404 vira `FinanceApiNotFound` e o resto propaga. Vale já nascer
  certo no Rust (`error.rs`).
- **Cache com `stale`**: resposta sempre traz `source`, `cached`, `stale`, `fetched_at`; com fonte
  fora do ar e cache vencido devolve o dado velho marcado como `stale`. Dá pra exibir "dado de ontem"
  na UI em vez de erro. TTLs do Lume: 1 h padrão, cotação ação/cripto 5 min, fundamentos 24 h, opções 24 h.
- **Regra de escopo ("régua")** pra decidir o que entra no núcleo de dados: público, somente leitura,
  precisa de normalização/cache, ≥ 2 consumidores. Fora: qualquer coisa com credencial, certificado
  ou estado do usuário (Open Finance, custódia B3 ficam fora).
- **Princípio do ecossistema**: "grátis + self-hosted sempre funciona com qualidade padrão; pago é
  conveniência ou qualidade extra, nunca funcionalidade básica trancada". Já na Fase 26; vale copiar
  pro `GUIDELINES.md` quando a Fase 26 for decidida.
- **Instância local compartilhada com o Warden** (daemon com handshake em `/health` com nome e
  versão, lockfile de descoberta com porta+PID+versão, regra de versão mínima, ciclo de vida
  independente do Anchor). É a Fase 21; o Lume já listou os pontos a resolver. **Some** se o Anchor
  absorver o núcleo de dados.
- **Namespaces por domínio** no núcleo (`finance/`, `weather/`, `geo/`, `sec/`) com cache e
  rate-limit compartilhados. Só faz sentido se o núcleo aceitar domínios não financeiros.
- **Expor o catálogo de fontes como SDK** (Python/Node): é a Fase 28.
- **Documentação pública gerada do OpenAPI** (site Fumadocs): se a API headless da Fase 26 virar
  produto, precisa de algo equivalente.
- **Auth por consumidor**: o Lume só tem lista estática em `API_KEYS`. O tier hospedado pago exigiria
  chave por consumidor, limite por plano e entitlement (mesmo desenho do TruthID).

#### C. Fontes e domínios novos sugeridos (o Lume só listou, nenhum foi pesquisado)

Financeiros, na ordem do que mais ajuda o Anchor:
1. **Calendário B3**: feriados e dias úteis (pro vencimento de opção, DARF, prazo de proventos).
2. **Calendário de resultados e fatos relevantes** (CVM IPE): alerta e atualização do preço-teto
   quando sai balanço novo; casa com o "ano_ref desatualizado" da Fase 1.
3. **Commodities e setor**: celulose, minério, clima e safra, pro módulo de commodities (Suzano,
   Klabin, Vale).
4. **Cadastros públicos**: CNPJ, CEP, CNAE, municípios e códigos IBGE, pra custódia por titular,
   empresa não listada e cadastro de imóvel.
5. **On-chain**: preço de token, dados da Base, saldo de endereço público, sem o app falar direto
   com RPC. Base da **Fase 27** (DeFi).
6. **Notícias por ativo** com dedupe: deixar por último, manutenção alta; só com caso de uso claro.
7. **Open Finance** (extratos): fica **fora** do núcleo pela régua; se vier, entra como integração
   com consentimento, não como dado público.

Não financeiros (o Lume sugeriu como utilidades do ecossistema; só entram se houver ≥ 2 consumidores,
senão moram no Warden): clima e astronomia (INMET, Open-Meteo), dados abertos do governo (Querido
Diário, PNCP), CVEs e releases (NVD, GitHub advisories), calendários (ENEM, SISU), status de serviços
(uptime de BCB, CVM, RPC da Base). Ordem sugerida pelo Lume: clima, geo, releases/CVEs.

#### D. Cuidados ao portar (achados que o Lume acumulou e não podem se perder)

- Regras de negócio sutis nos 41 testes: CNPJ só dígitos, retry por janela no BCB, mapeamento de
  taxonomia SEC, travas de catálogo. **Portar os testes junto**, senão os achados somem.
- `fetched_at` precisa ser **UTC** (bug de serde+chrono achado ao vivo no Anchor).
- Contrato de erro que o Anchor depende: **404 = "não tem esse dado"**, pulado por ticker; o resto propaga.
- Licenças: **CoinMetrics Community é não comercial** (Fase 31) e a bolsai é a única fonte com chave.
  Ambos pesam no tier hospedado pago.
- Resíduos de nome: o CI do Anchor usa `.lume-version` (já renomeado) mas ainda há comentários "easybusiness";
  o repo já migrou no GitHub pra `Arven-Labs/lume`.
- Dev e CI acoplados: dev assume `docker compose up` do Lume em `localhost:8000` com chave fixa; o
  `build.yml` faz checkout do Lume por SHA. Qualquer caminho da Fase 26 precisa mexer nisso.

#### E. Fases 2–4 do Lume (visão original, hoje fora de escopo)

Engine fiscal SEFAZ (NF-e), WhatsApp/Meta e Workspace web do empreendedor, mais Unified Payment API
(Asaas, Mercado Pago, Pagar.me, Stripe) e baixa automática de boleto/Pix. **Ficaram de fora** na
reorientação do Lume (iriam pro projeto de integrações burocráticas, parado) e **não pertencem ao
Anchor**. Registradas só pra não serem esquecidas; a única que toca o Anchor é conciliação de caixa
de empresa (módulo B3 corporativo), e só se o dono do projeto quiser a linha de empresário.

#### F. Situação de cada item da seção A depois da Fase 26 (Sessão 108)

Conferido no código do Anchor em 2026-10-17. O Lume foi relido por inteiro (o clone local estava
desatualizado e foi avançado com `git pull --ff-only` até `7767069`).
- **Já portado para Rust (`finance_core/`)**: Selic diária e curva DI; índices B3 (IFIX, SMLL, IDIV);
  metais; FII (indicadores mensais, imóveis, resolução ticker→CNPJ); SEC EDGAR (fundamentos, DCF,
  payout, REIT); CVM DFP; Yahoo (B3 e EUA); cripto (CoinGecko, Fear & Greed, os indicadores do ETH);
  regras de IR (`tax_rules`); **câmbio multi-moeda, avisos de dividendos da CVM e opções da B3 com
  gregas (Sessão 109)**. Com isso, **tudo que era só do Lume está no Anchor**.
- **NÃO portado, só existe no Lume** (é o que se perde se o repo for apagado sem copiar o desenho):
  1. ~~**Câmbio multi-moeda**~~ — **portado na Sessão 109** (ver o fim da seção I).
  2. ~~**Opções: catálogo de séries, preço EOD e gregas**~~ — **portado na Sessão 109, só a camada de
     dados** (ver o fim da seção H); base da Fase 18, que segue sem consumidor.
  3. ~~**Avisos de dividendos da CVM (IPE)**~~ (`companies/{cvm}/dividend-notices`) — **portado na
     Sessão 109, só a camada de dados** (ver abaixo); o "provento esperado" (Sessão 77.2) segue sem
     consumidor.
  4. **Cache em Postgres/SQLite do Lume** (38 tabelas): o Anchor guarda o resultado no `anchor.db` e
     usa cache de frescor em memória; nada disso precisa voltar.
- **Avisos de dividendos da CVM portados (Sessão 109), só a camada de dados** (sem tela, tabela,
  comando Tauri nem leitura do PDF; escolha do dono do projeto):
  - `finance_core/cvm_ipe.rs`: zip anual do IPE (`ipe_cia_aberta_{ano}.zip`), categoria exata "Relatório
    Proventos", `Codigo_CVM` como inteiro, ano atual e anterior (falha de um ano é tolerada; sem linhas e
    com algum ano fora do ar sobe o erro; empresa sem aviso é lista vazia, nunca `NoData`). Retorna
    `{ protocolo_entrega, data_entrega, link_download }`, do mais antigo ao mais novo; o protocolo cai no
    link porque vem em branco nessa categoria. Fachada `fetch_company_dividend_notices(cvm_code)`.
  - **Diferença do Lume**: o Lume nunca guardou o zip em disco (o TTL ficava numa camada de cache em
    banco). Aqui o zip vai para o `disk_cache` como o DFP, mas com `max_age` curto: 12 h no ano
    corrente (o arquivo cresce o ano todo) e 7 dias no anterior, em vez dos 30 dias do DFP. Também há
    dedupe por protocolo, que o Lume não fazia. `latin1` do `cvm_dfp` virou `pub(crate)` e é reaproveitado.
  - Verificado: `./dev.sh check` limpo, 11 testes unitários (zips em memória) e `live_dividend_notices`
    ao vivo: Banco do Brasil (CVM 1023) devolve 18 avisos nos dois anos, o último de 2026-09-09, todos
    com link do `rad.cvm.gov.br`. Sem E2E, porque não há tela.
  - **Para o consumidor**: o valor, a data de pagamento e a data-com ficam só no PDF do link, que precisa
    de checagem do `%PDF-` (a CVM às vezes devolve uma página HTML de erro para versões antigas). O
    `resolve_cvm_code` da Sessão 94 saiu junto com a Fase 20; hoje o código CVM vem de
    `fetch_stock_bolsai_fundamentals(ticker).cvm_code` (exige a chave da bolsai).
  - **Consumidor (Sessão 110)**: alerta, sem criar sugestão, sem migration e sem ler o PDF (escolhas do
    dono do projeto). Comando `list_cvm_dividend_notices(portfolio_id)` em `commands/cvm_notice.rs`: ativos
    `acao_br`/`bdr` com posição aberta (`quantity_held_by`), código CVM pelo bolsai, avisos dos últimos 90
    dias do mais novo ao mais antigo. Falha por ativo (ex.: sem chave) vai no campo `error` e não derruba
    os outros. Tela: card `CvmNoticesCard.tsx` no topo de "Manage suggestions" (aba Dividends), com
    "Open PDF" via `openUrl`. Fora de escopo: valor/datas do aviso, FII (outro fluxo), widget na Home.
- **Fragilidade que veio junto**: no Lume, CDI e IPCA mensais (`fetch_monthly_series`) ficaram com 15 s
  e sem nova tentativa, e um 502 passageiro do BCB já derrubou o CDI. No Anchor, `bcb_sgs::fetch_monthly`
  usa o timeout padrão e também não tenta de novo (só a Selic diária tem janela de 9 anos, 45 s e 3
  tentativas). **Resolvido na Sessão 109**: `fetch_with_retry` em `bcb_sgs.rs` serve a janela diária
  (45 s) e a série mensal (30 s), com 3 tentativas e 2 s de espera; 404 não é tentado de novo.
- **O que o mobile faz**: o app Flutter **nunca usou o Lume**; busca Yahoo e CoinGecko direto
  (`yahoo_quote_service.dart`, `coingecko_quote_service.dart`). Se o Anchor virar núcleo único de dados,
  o mobile é um consumidor a decidir.

#### G. Achados técnicos por fonte (do `ARCHITECTURE.md` e do `PHASE.md` do Lume)

Valem para qualquer reescrita ou nova fonte; vários já aparecem nos comentários do `finance_core`.
- **BCB SGS**: série **diária** sem `dataInicial` devolve HTTP 406 ("janela de no máximo 10 anos");
  pagina em janelas de 9 anos desde 1994 (janelas antes do início da série voltam vazias). O BCB oscila
  (mesma janela em 0,4 s, 30 s, 502 ou HTML com status 200). Catálogo travado (CDI 4391, IPCA 433, Meta
  Selic 432), sem código especulativo. Valores de referência: CDI 481 pontos desde 1986-08, IPCA 559
  desde ~1979, Selic 10.058 pontos desde 1999-03-05.
- **Yahoo**: 5 capacidades por ticker (cotação, SMA 50/100/200 + CAGR 5/10, média de dividendos 5 anos,
  histórico de 10 anos, pagamentos). Sufixo `.SA` só na B3. PETR4 como referência: 2.491 pontos de
  histórico, 34 pagamentos. Dividendo sem histórico (growth stock) é um terceiro estado ("sem dado"),
  não erro, e nunca apaga o valor já guardado.
- **CVM**: não é API por identificador, é **um zip por ano** (~870 companhias no DFP). Empresas por
  **código CVM**, fundos por **CNPJ de 14 dígitos** (o `geral` devolve o CNPJ pontuado, com 18
  caracteres; sem normalizar o insert quebrava). Cache de zip em disco. `fii_properties` tem N linhas
  por fundo e o refresh apaga e reinsere o conjunto numa transação, para imóvel que saiu do relatório
  não ficar "fantasma". HGLG11 hoje se chama PÁTRIA LOG (era CSHG Logística). Referência: VALE3 (CVM
  4170) ROE 6,25 %, alíquota efetiva 55,75 %, payout 5 anos 61,38 %.
- **bolsai**: única fonte com chave. O campo `roe` **mistura lucro trimestral com TTM** conforme a
  empresa; o Lume expõe como veio e manda quem quer ROE confiável usar o da CVM. Expõe o `cvm_code`
  (PETR4 → 9512) para encadear com a CVM.
- **SEC EDGAR**: resolução ticker→CIK com cache; limite de ~9 req/s com um único timestamp global, **não
  seguro sob concorrência** (aceito no MVP). Banco (JPM) dá 404 em `dcf-fundamentals` de propósito: a
  taxonomia não traz EBIT, estoque e contas a receber/pagar do jeito não financeiro. Referência: AAPL
  LPA 7,46, VPA 4,99, ROE 151,9 %, payout 5 anos 15,08 %. Fallback `NetIncomeLoss` → `ProfitLoss`
  (Simon Property).
- **Cripto**: ETH tem 4 indicadores via DefiLlama/ultrasound.money/CoinGecko mais 4 via **CoinMetrics
  Community** (sem chave, 1000 req/10 min por IP, histórico desde 2015-08-08, `page_size` com teto de
  10000, `CapRealUSD` bloqueado no tier grátis, por isso o `RealizedCap` é derivado de
  `CapMrktCurUSD`/`CapMVRVCur`). **Staking Yield** continua sem fonte grátis (a beaconcha.in passou a
  exigir chave em 2026). A API serve o valor bruto; a classificação verde/neutro/vermelho é regra do
  Anchor. Resolução símbolo→`coin_id` da CoinGecko por busca exata, desempatando pelo menor
  `market_cap_rank`, com cache próprio.
- **B3 Taxa Swap (curva DI)**: usa o sistema legado "Pesquisa por Pregão"
  (`pesquisapregao/download?filelist=TS{YYMMDD}.ex_,`); o UP2DATA novo ficou atrás de Cloudflare desde
  dez/2025 (testado e descartado). Zip externo → blob auto-extraível → zip interno com `TaxaSwap.txt`
  (largura fixa, latin-1), filtra a curva `PRE`. A lib de referência (`pyettj`) desliga a verificação
  de TLS; o Lume e o Anchor **não**. Dia sem pregão devolve zip de 22 bytes = "sem dado". Curva de
  2026-09-15: 272 vértices, o de 1 dia (13,90 %) bate com a série BCB 1178.
- **Índices B3**: catálogo travado em IFIX (base 2010), SMLL e IDIV (base 2005). IFIX tem 3.885 pontos
  desde 2010-12-30.
- **Metais**: XAU/XAG/XPT/XPD → GC=F/SI=F/PL=F/PA=F, preço sempre em onça troy, sem conversão.
- **Regras de IR**: não há fonte legível por máquina, então a tabela é **curada à mão e versionada**.
  Lei nova = **entrada nova com a data nova, nunca editar a antiga** (o consumidor apura meses passados
  com a regra da época). 11 regras na primeira versão; as vindas de fonte secundária levam "verify on
  planalto.gov.br" no campo `source`.
- **Padrão de testabilidade**: o catálogo de indicadores guarda o `fetch` como closure que resolve o
  atributo do módulo na hora da chamada; importar a função direto congela a referência e o `patch` dos
  testes fica inerte.

#### H. Opções: catálogo, preço EOD e gregas (portado na Sessão 109; base da Fase 18)

**Portado (Sessão 109)**, paridade completa só na camada de dados (sem tela, tabela, comando Tauri nem
`AssetClass`; escolha do dono do projeto):
- `finance_core/black_scholes.rs` (preço, gregas, IV por Newton com bisseção, `libm::erf`),
  `finance_core/b3_options.rs` (Séries Autorizadas + COTAHIST, cache global de 24 h em memória) e
  `finance_core/option_greeks.rs` (taxa livre de risco da curva DI por dias corridos, spot do chamador).
  Fachada `fetch_option_series(raiz)` e `fetch_option_greeks(série, ativo-objeto)` em `finance_api/client.rs`.
  `disk_cache` ganhou `get_or_download_with` (timeout por chamada: 300 s para o COTAHIST de ~93 MB).
- **Diferenças em relação ao Lume**: (1) sem tabelas `option_series`/`option_eod_quotes`: os dois arquivos
  ficam no `disk_cache` (24 h) e o mercado parseado em memória (`TtlCache`, `stale` se a B3 cair);
  (2) `TPREG == "01"` explícito no COTAHIST; (3) o COTAHIST do ano anterior só entra **em janeiro**
  (juntos, vale o negócio mais recente); fora de janeiro a falha do arquivo corrente sobe, porque servir o
  ano anterior daria preço de meses atrás como atual (aconteceu num teste ao vivo); (4) a busca da curva DI
  recua também em falha de provider (no máximo 2 falhas), porque o arquivo do dia mais recente costuma
  vir quebrado ou lento, e a resposta informa a data da curva usada; (5) `dias_corridos` vem direto de
  `b3_taxa_swap::di_curve` (a fachada `fetch_di_futures_curve` o descarta); (6) mapa de raízes só com a
  exceção `EMBR3` → `EMBJ`.
- **Achados ao vivo**: (a) o `fileId` das Séries Autorizadas **mudou** (o antigo passou a redirecionar
  para a página de erro da B3, com status 200): o novo é `8AA8D0CDA134A78701A146F166082C07`, achado em
  `.../mercado-a-vista/opcoes/series-autorizadas/` (página que leva ~70 s); o `disk_cache` guardaria o HTML
  como zip por 24 h, então `ensure_zip` confere o `PK` e apaga o arquivo ruim; (b) o layout das séries não
  mudou (77.002 linhas `02` e 2.282 `03`); (c) o COTAHIST de 2026 tem 93,5 MB; (d) mercado inteiro:
  77.002 séries e 204.153 últimos preços; PETR: 3.658 séries, 2.804 com negócio; (e) a B3 oscila muito:
  timeouts e 520 em páginas e arquivos que minutos depois baixam em 0,2 s; (f) uma call ATM de 16 meses
  (PETRB620) negociada abaixo do piso do modelo sem dividendos (juros de 14 %) não tem IV: dá
  `InvalidInput`, igual ao 422 do Lume, e a mensagem agora diz isso em vez de só "abaixo do intrínseco".
- Verificado: `./dev.sh check` limpo; 6 testes de Black-Scholes (Hull, paridade, round-trip de IV), 14 de
  `b3_options` e 13 de `option_greeks`; ao vivo, `live_options_series` e `live_option_greeks`:
  PETRK611 (call de PETR4, strike 59,91, 33 dias, spot 59,68) → IV 40,9 %, delta 0,55, gamma 0,054,
  theta −0,055/dia, vega 0,071, taxa 13,56 % (curva de 2026-10-16). Mesma ordem de grandeza do exemplo
  do Lume (PETRL522: IV 46,5 %, taxa 13,59 %).

- **Fontes (todas grátis, da B3)**: *Séries Autorizadas* (arquivo estático pipe-delimited em
  `b3.com.br/lumis/portal/file/fileDownload.jsp?fileId=...`) dá o universo de séries (ativo-objeto,
  call/put, strike, vencimento, estilo); *COTAHIST anual* (`bvmf.bmfbovespa.com.br/InstDados/SerHist/
  COTAHIST_A{yyyy}.ZIP`) dá o último preço negociado. **Não existe COTAHIST diário** (a URL `_D` devolve
  página de erro). O anual de 2026 tem ~80 MB comprimidos, ~700 MB descomprimidos e ~2,8 milhões de
  linhas de 245 bytes; 86 % são opção (`TPMERC` 070 = compra, 080 = venda) e toda linha de opção é um
  negócio real (`TOTNEG` nunca 0). Primeira chamada leva ~2 min.
- **Não há fonte grátis para bid/ask, open interest nem IV.** Yahoo não tem chain de opções B3 (testado
  em PETR4/VALE3/BOVA11), UP2DATA está atrás de Cloudflare e a bolsai não cobre derivativos.
- **Formato**: o arquivo de séries tem dois tipos de linha: "02" (ação/ETF, ~85 mil das ~87 mil) e "03"
  (opção de índice em pontos, IBOVESPA/SMALL CAP, liquidação em R$/ponto, colunas totalmente
  diferentes). O tipo "03" ficou **fora de escopo**: a exposição a índice já vem das opções de BOVA11.
- **Raiz da opção**: sempre prefixo exato do ticker da série (zero exceções em ~85 mil linhas), mas
  **nem sempre** é o ticker menos o dígito (Embraer: `EMBR3` → raiz `EMBJ`). Sem mapa genérico, tratado
  caso a caso (PENDING P2 do Lume). Ao portar, trazer o mapa.
- **Modelo de dados**: cache **global**, não por identificador (o arquivo é o mercado inteiro). Frescor
  por `MAX(fetched_at)` da tabela toda, TTL de 24 h; `option_series` apaga e reinsere (série
  desregistrada some); `option_eod_quotes` é **1 linha por série** (o plano previa histórico, mas só o
  último preço interessa e o parser já calcula o máximo em memória). Sem cache em disco do zip.
  `/series` devolve `data: []` com 200 para ativo-objeto desconhecido (nunca 404) e `last_price: null`
  para série registrada e nunca negociada.
- **Gregas** (`black_scholes`, sem tabela, recomputadas a cada chamada porque o preço do ativo-objeto
  muda a cada tick): matemática pura, `erf` para a CDF normal, sem numpy/scipy; IV por Newton-Raphson
  (vega como derivada) com fallback para bisseção, devolvendo `None` explícito quando o preço de
  mercado está abaixo do valor intrínseco (sinal de preço velho, não bug). Validada contra o caso do
  Hull (S=K=100, T=1, r=5 %, σ=20 % → call ≈ 10,4506, put ≈ 5,5735, delta ≈ 0,6368, gamma ≈ 0,018762,
  vega ≈ 0,3752, theta ≈ −0,017573/dia) e por round-trip de IV. **Estilo americano não é
  distinguido** (só aproximação europeia, sem fonte para validar exercício antecipado).
- **Decisão confirmada com o dono do projeto**: o **preço do ativo-objeto vem de quem chama**
  (`underlying_ticker`), não é adivinhado, porque a raiz não diz a classe (ON/PN/PNA…) e uma grega com o
  papel errado seria erro silencioso. A resposta expõe `last_trade_date`, porque o preço usado pode ter
  semanas em série ilíquida.
- **Taxa livre de risco**: curva DI interpolada linearmente por **dias corridos** (evita calendário de
  feriados; `T` da opção usa a mesma base), sem extrapolar fora do intervalo (usa o vértice mais
  próximo) e recuando dia a dia até 10 dias em fim de semana/feriado. Exemplo: `PETRL522` (call PETR4
  dez/2026, strike 49,91, spot 49,06) → delta 0,576, gamma 0,034, theta −0,033/dia, vega 0,097, IV
  46,49 %, taxa 13,59 %.
- **Erros**: 404 série desconhecida ou nunca negociada, 422 opção vencida ou preço abaixo do intrínseco,
  502 fonte fora do ar.

#### I. Câmbio multi-moeda (portado na Sessão 109; base das Fases 19 e 30)

**Portado (Sessão 109)**, escopo "dados + tela de consulta"; moeda base e conversão na carteira
(Fase 19) seguem fora.
- `finance_core/fx.rs`: as 17 moedas do `currency_catalog.py` do Lume e `pair_info(base, quote)`, que
  valida o par antes de qualquer rede (moeda fora do catálogo ou par igual → `NoData`) e devolve o
  código (`EURUSD`), o ticker (`EURUSD=X`) e o nome (`EUR/USD`). Cotação e histórico saem de
  `yahoo::quote`/`price_history` sem mudança (`Instrument::global("EURUSD=X")`).
- Fachada `fetch_fx_quote`/`fetch_fx_price_history` em `finance_api/client.rs`; coletor
  `finance_api/fx.rs::collect_pair` grava em `stock_quotes`/`stock_price_history` com o código do par
  como ticker, `exchange = "FX"` e `currency` = moeda cotada (**sem migration**); comando Tauri
  `run_fx_collector(base, quote)`, com o mesmo lock dos outros coletores.
- Tela: `FxLookupSection` numa `view = "fx"` do Research ("Or see currency exchange rates" no estado
  vazio), com dois selects de moeda. **Não virou `AssetClass`** (que também define as classes de ativo
  da carteira), igual aos juros. O `PriceHistoryChart` ganhou `decimals` opcional porque 1,1244 não cabe
  em 0 casas.
- Verificado: `./dev.sh check` limpo; 6 testes unitários passando; `live_fx_pairs` contra o Yahoo real
  (EUR/USD 1,1244, 2.603 pontos de histórico, cross GBP/JPY ok). CI do push `0249fc2` verde (Frontend,
  Rust e E2E); o caso novo do `lookup.e2e.mjs` (USD→BRL, rede real) passou sem skip. **Não verificado**:
  só o clique manual na tela.
- Diferenças em relação ao Lume: o TTL de 5 min/1 h não foi portado (como nos metais, a tela lê a
  última linha gravada e só coleta se não houver nenhuma; "Refresh data" força); a resposta não carrega
  `cached`/`stale`.

- **Uma fonte só, o Yahoo**, pelo ticker `{BASE}{QUOTE}=X` (`EURUSD=X`, `USDBRL=X`, `ARSUSD=X`, e até
  cross entre moedas não-BRL como `GBPJPY=X`, 208,5 JPY). A hipótese do roadmap (BCB PTAX para BRL e
  outra fonte para cross) **não se confirmou**. Ticker desconhecido devolve 404 real. Histórico de
  10 anos com o mesmo formato do de ações (`eurusd`: 2.601 pontos desde 2016-09-15).
- **Catálogo por moeda, não por par**: a conversão é mecânica (`"eurusd"` → `"EURUSD=X"`), então
  catalogar pares seria contra o pedido de "moedas quaisquer". Whitelist de **17 códigos ISO 4217**
  (majors G10-ish, BRL, CNY e Mercosul/LatAm: ARS, UYU, PYG, CLP, COP, PEN, MXN); par válido se as duas
  metades estão no catálogo e são diferentes (até 17×16 = 272 pares). Validação **antes** de qualquer
  chamada ao Yahoo (`UnknownCurrencyPairError` → 404 sem rede).
- **Limitação deliberada**: moedas de valor muito baixo (VND) fazem o Yahoo arredondar a cotação para
  `0,0` na precisão que ele reporta; por isso ficaram fora da lista curada.
- Sem coluna `name` (base e cotada saem do `pair_code` por split); reaproveita os TTLs de cotação
  (5 min) e de histórico (1 h).

#### J. Visão original do blueprint e modelo de negócio (do `CONTEXT.md` e do `ROADMAP.md` do Lume)

- **Origem**: o blueprint `blueprint_plataforma_opensource.md.docx` descrevia 3 camadas: (A) ponte para
  desenvolvedores (APIs e SDKs: pagamentos unificados, fiscal, WhatsApp, finanças/Open Finance), (B)
  workspace web do empreendedor, (C) módulo B3 e mercado financeiro. O Lume começou pela fatia
  financeira de A e C por já ter um consumidor real (o Anchor). Fases 2 a 4 do Lume (fiscal SEFAZ,
  WhatsApp/Meta, Workspace) nunca começaram e estão fora do escopo do Anchor (seção E).
- **Visão do PRD**: "democratizar o acesso a dados financeiros confiáveis (mercado brasileiro e
  internacional, cripto, macroeconomia) para desenvolvedores e pequenos empreendedores brasileiros, via
  uma API única, simples e open-source". Problema declarado: cada projeto financeiro reinventa
  integração, retry, fallback e normalização de dezenas de APIs.
- **Open-Core**: versão community gratuita (infra própria, MIT) vs. nuvem hospedada pay-as-you-go
  (margem sobre micro-transações). Exemplos do blueprint: R$ 0,15 por nota fiscal emitida, repasse do
  custo Meta mais margem por mensagem, micro-taxa sobre webhooks e baixa automática de boleto/Pix. Como
  cobrar a Finance API (por chamada, por volume, tier grátis generoso) **ficou em aberto**; a Sessão 18
  respondeu em parte: cobra-se hospedagem, conveniência e dado premium, nunca o uso local.
- **Licença**: MIT (mesma do TruthID/Anchor); AGPL foi considerada para o open-core e descartada.
- **Ideias de expansão nunca sequenciadas**: Open Finance (extratos), Unified Payment API (Asaas,
  Mercado Pago, Pagar.me, Stripe) e SDK do catálogo de fontes (Python/Node). Pela régua de escopo,
  Open Finance e custódia B3 ficam fora do núcleo de dados.
- **Pontos de debate da Sessão 18 que dependiam do Lume continuar existindo** (hoje superados, mas
  guardados): a "instância local compartilhada" com o Warden (handshake em `/health`, lockfile com
  porta+PID+versão, ciclo de vida do daemon, regra de versão mínima, diretório de dados comum e
  autenticação local só por loopback) e a decisão de "manter o Lume separado do Anchor porque o Warden
  compartilha a instância". A BYO-key por keyring só faz sentido no modo local; no servidor e no
  hospedado a chave viria de env ou secret manager.
- **Docs públicas**: o Lume tinha um site Next.js + Fumadocs com a referência gerada do `/openapi.json`
  (publicado no GitHub Pages, sem domínio próprio, busca embutida Orama). Se a API headless da Fase 26
  virar produto, precisa de algo equivalente.
- **Auth**: lista estática em `API_KEYS` (header `X-API-Key`), sem tabela nem rotação; chave por
  consumidor, limite por plano e entitlement ficariam para o tier hospedado.
- **Quem usaria cada ideia nova** (da nota `lume-ideias.md`): macro e calendário → Anchor (reajuste de
  imóvel, desconto de DCF) e Warden (lembretes); documentos regulatórios → Warden (alertas) e Anchor
  (preço-teto); commodities → módulo de commodities do Anchor (Suzano, Klabin, Vale); cadastros → Anchor
  (custódia por titular, empresa não listada) e o site do açaí; on-chain → classe cripto sem RPC direto;
  clima e astronomia, geo, dados abertos do governo (possível fonte de leads de freelance), CVEs e
  releases (homelab, servidor de Minecraft), calendários e status de serviços → Warden e health check
  dos providers. Fontes grátis boas: BCB, CVM e IBGE; o premium hospedado seria por latência e
  cobertura. Pontos em aberto da nota: se o nome "centro de informações" aguenta a abertura, e quais
  ideias têm consumidor além do Anchor (a régua exige pelo menos dois).

#### K. Desmontagem: opções avaliadas, riscos e questões abertas (do `DESMONTAGEM.md` do Lume)

- **Opções**: A, mover o `api/` Python para o repo do Anchor (menor risco, preserva os 41 testes, mas
  mantém dois runtimes); B, reescrever em Rust num crate (um runtime só, mas ~7 mil linhas Python, 38
  tabelas e parsers CVM/SEC para portar, e perde o ecossistema Python se o backtesting da Fase 22 o
  quiser); **C, híbrido faseado (escolhida na Sessão 23 do Lume)**: A primeiro, depois Rust por domínio
  com teste de paridade. O Anchor decidiu **pular o passo A** e ir direto para Rust (Fase 26).
- **Riscos que o Lume listou** (nove): divergência de linguagem; quatro domínios sem consumidor; cache
  duplicado; dev e CI acoplados ao Lume; Settings Local/Remote e fallback do Warden sem sentido; docs
  públicas; auth e hospedagem; lógica de negócio sutil acumulada nos testes; brainstorms em suspenso.
  A decisão da Fase 26 resolveu os de linguagem, cache duplicado, dev/CI acoplados e Settings
  Local/Remote com fallback. **Seguem abertos**: docs públicas, auth e hospedagem, a revisão dos 41
  testes do Lume (portei os de cada domínio, mas não conferi um a um) e os brainstorms (seção B).
- **Questões abertas**: preservar o `DataProvider` (existe, com um provider por categoria); renomeação
  EasyBusiness → Lume (sem sentido se o repo for descartado); aviso "em desmontagem" no README do Lume
  (**não feito**, aguarda confirmação do dono); arquivar ou apagar o repo (decisão exclusiva do dono,
  depois de validar o Anchor sozinho). O repo mudou de lugar no GitHub para `Arven-Labs/lume`.
- **Inventário que o Lume fez** (para conferir cobertura): 13 routers, ~14 services, ~25 módulos de
  fontes, 13 modelos (38 tabelas), 10 migrations, 41 arquivos de teste com fontes mockadas, 13 hosts
  externos (api.bcb.gov.br, query1.finance.yahoo.com, dados.cvm.gov.br, data.sec.gov e www.sec.gov,
  api.coingecko.com, api.llama.fi, community-api.coinmetrics.io, ultrasound.money, api.alternative.me,
  www.b3.com.br, sistemaswebb3-listados.b3.com.br, bvmf.bmfbovespa.com.br, api.usebolsai.com). TTLs:
  1 h padrão, cotação de ação/cripto 5 min, fundamentos 24 h, opções 24 h. Respostas sempre com
  `source`, `cached`, `stale` e `fetched_at`.

#### L. Diretrizes e lições de processo do Lume (do `GUIDELINES.md`, `PENDING.md` e das sessões)

- **Diretrizes**: código novo em inglês (strings, nomes, rotas); comentários podem ser em português;
  docs de `project/`, README e commits em português. Repo público desde o primeiro commit: nunca
  commitar chave, string de conexão ou credencial (`.env` ignorado, com `.env.example`). Não copiar a
  fonte Python 1:1, reimplementar usando o cliente antigo como referência de comportamento.
  **Confiabilidade antes de cobertura**: poucas fontes bem tratadas (retry, timeout, normalização, teste
  contra a API real) valem mais que muitas frágeis.
- **PENDING P1**: `fiis/{cnpj}/properties` devolve `data: []` tanto para CNPJ desconhecido quanto para
  FII de papel sem imóvel, e a CVM não dá como distinguir. Decisão consciente: tratar `[]` como "sem
  imóvel", nunca como erro. **PENDING P2**: raiz de opção da Embraer (seção H).
- **Bugs de integração que custaram caro** (Sessão 7 do Lume, ao trocar o coletor Python por HTTP):
  capturar o erro genérico dentro do loop por ticker fazia o coletor terminar com "Updated 0 quote(s)"
  e código 0 com a API derrubada, **mascarando indisponibilidade total como "sem dado"** (só o 404 pode
  ser engolido por ticker); metais e índices B3 usam código minúsculo (`xau`, `ifix`) e o Anchor sempre
  trabalhou em maiúsculo; os códigos dos indicadores do ETH usam hífen (`tvl-trend`) e o Anchor usa
  underscore (`tvl_trend`, chave de `indicator_thresholds`).
- **Ambiente**: rodar `pytest` contra o Postgres do `docker compose` cria as tabelas sem
  `alembic_version` e o próximo `up` quebra com `DuplicateTable` (faltou um banco de teste separado);
  Docker sem `CAP_NET_ADMIN` não cria rede bridge; o `docker-compose.yml` do `docs` não pode ter
  `user: "1000:1000"` porque o volume nomeado de `node_modules` nasce root; um `.env.example` ainda
  tinha credenciais do nome antigo (`easybusiness`) e escapou do grep por não ter extensão de doc.
- **Fumadocs (3 bugs achados só rodando)**: `createOpenAPIPage()` não pode estar num módulo alcançável
  por Server Component (isolar em arquivo `"use client"`); faltava `openapi.preloadOpenAPIPage(page)` no
  `page.tsx`; o schema OpenAPI para o deploy estático sai de `app.openapi()` sem banco nem API rodando.

**Estado**: registro completo do `project/` do Lume (arquivos lidos: `OVERVIEW`, `CONTEXT`, `GUIDELINES`,
`INDEX`, `PENDING`, `ARCHITECTURE`, `PHASE`, `ROADMAP`, `DESMONTAGEM` e as sessões 1 a 24). Sessão 108.
Na Sessão 109 a série mensal do BCB ganhou o retry da Selic e o câmbio, os avisos de dividendos da CVM e
as opções com gregas foram portados: **não sobrou nenhum domínio de dados só do Lume**. Resta a decisão
do dono do projeto sobre arquivar ou apagar o repo do Lume e, no que é produto, os consumidores
(Fases 18, 19 e o provento esperado).
