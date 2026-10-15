# O que é o Practice Valuation

App desktop pessoal para acompanhar teses de investimento em ações (B3) e criptoativos.
Substitui a ideia original de planilha (ver Fase 2, histórico) por um app com banco de dados local.

**O que ele precisa fazer (visão do usuário, ainda sendo refinada):**
- Puxar o máximo de dados possível de fontes externas (fundamentos de ações BR, dados on-chain/mercado de cripto), com espaço pra ajuste manual quando necessário
- Guardar **múltiplos preços-teto/cálculos de valuation por ativo**, cada um com seu próprio conjunto de premissas (ex: duas projeções do mesmo ativo com taxas de crescimento diferentes, ambas salvas e comparáveis lado a lado)
- Cadastrar premissas por ativo (incluindo cripto) e monitorar indicadores automaticamente
- Avisar o usuário quando um ativo entrar em "zona de compra" segundo as premissas cadastradas
- Banco de dados **local** por enquanto — sync entre máquinas/nuvem é ideia pra mais adiante (ver Roadmap)

**Decidido até agora** (ver "Decisões de Arquitetura em Aberto"):
- Stack híbrida: app em **Tauri + Rust + React/TypeScript** (reaproveitando o padrão do TruthID), coleta de dados em **Python** (ver Fase 2), os dois se comunicando só através de um banco **SQLite** local compartilhado — sem API/IPC entre eles

- UI: **Tailwind CSS + shadcn/ui (Radix) + TanStack Table**, visual **arejado tipo dashboard** (não denso tipo planilha, apesar da ideia original de "funcionar como planilha" — isso ficou pro comportamento/dado, não pra densidade visual)

**Ainda não decidido**:
- Biblioteca de gráfico (pra tela de cripto/indicadores, Fase 4.3) — avaliar quando chegar lá (candidatos: Recharts, ou lightweight-charts da TradingView, mais voltada pra preço/candlestick)
- ~~Lista exata de metodologias de preço-teto~~ — entregue na Sessão 1, ver Fase 3

---

# Status Geral

```
Fase 0 — Fundamentos & Decisões de Arquitetura   [~] Em andamento (0.1–0.5 ✓, falta 0.6)
Fase 1 — Modelo de Dados (schema do banco local)  [~] Em andamento (migrations rodando normalmente a cada modelo, falta só formalizar 1.3 como concluída)
Fase 2 — Coleta de Dados (ações BR + cripto)      [~] Em andamento (cotação e dividendo médio do Bazin via Yahoo Finance — brapi removida na Sessão 10 —, LPA/VPA via bolsai, ROE e Payout (Banks) via CVM (Sessão 16 — ROE corrigido, payout novo, ambos calculados direto do DFP/DMPL) e todas as entradas contábeis do DCF via CVM funcionando ponta a ponta — nenhuma pendência de dado de ações sobrando; 4 dos 9 indicadores cripto automatizados (TVL Trend, Net Issuance, Fees vs Emissão, NVT Ratio — os 2 últimos concluídos na Sessão 21) — ver Log de Sessões; os outros 5 (MVRV, Puell, Exchange Netflow, Endereços Ativos, Staking Yield) têm bloqueio documentado, sem fonte grátis conhecida)
Fase 3 — Motor de Cálculo (preço-teto/valuation)  [x] Completa — 8 modelos de ação + score cripto (9 indicadores), todos ponta a ponta; Sessão 56 (2026-08-02) migrou a UI da tela solta "Valuation" pro dentro de Research (`StockAnalysisSection.tsx`, botões "New Valuation"/"All Saved Valuations") e separou "Calculate" (preview) de "Save" (persiste), antes uma ação só
Fase 4 — Interface Desktop                        [~] Em andamento (shadcn/ui + TanStack Table instalados, tela de valuations salvos completa incl. detalhe fino de premissas, 7 formulários + painel cripto vestidos, identidade visual dark+verde definida; Sessão 10: painel cru de tabelas de teste removido, cada formulário ganhou botão de buscar dado por ticker que preenche os campos automaticamente)
Fase 5 — Monitoramento & Alertas                  [x] Completa — cadastro (5.1), verificação periódica (5.2) e notificação nativa do SO (5.3, validada na Sessão 12)
Fase 6 — Publicação (GitHub público)               [x] Completa (6.2 README e 6.3 LICENSE concluídos na Sessão 25; 6.4 já estava satisfeita há tempos — o repo já tinha histórico público em `origin`/`gitea` bem antes, checklist só nunca tinha sido marcado; 6.1 rodada na Sessão 47 — segredos/credenciais confirmados limpos contra o repo e o histórico completo do git — e os 2 achados de hardening/consistência que sobraram (UI em português, CSP desabilitado) resolvidos na Sessão 48, mesmo dia: CSP configurada e testada sem quebrar o modo dev, UI inteira traduzida pra inglês; só 6.5 — aviso de atualização disponível — segue não iniciada, ideia solta sem prioridade)
Fase 7 — Chat de IA Integrado                      [x] Completa (7.1 a 7.10.6 — Sessão 27 (2026-07-21) fechou a fase inteira, entregando 7.10.3 (contador de tokens) e 7.10.4 (IA propõe criar valuation, com confirmação humana obrigatória) na mesma sessão; testado ponta a ponta pelo usuário no app real, incl. aprovar/descartar/reload; Claude/OpenAI seguem sem teste contra API real por falta de chave de teste — não bloqueia o fechamento da fase, é uma pendência à parte — ver Fases Detalhadas)
Fase 8 — Sync Multi-Dispositivo via TruthID+IPFS  [~] Em andamento — Sessão 23 (2026-07-16): transporte cross-device do `/sign-request` validado ponta a ponta; Sessão 31 (2026-07-28): 8.1 deployada de verdade em Base Sepolia (`0x9e2fd1a4146e3c40ecfef9d35a09e75e00ac30e8`) e 8.2 (escrita via canal delegado, same-machine) implementada e validada ponta a ponta pelo usuário no app real; Sessão 32 (2026-07-28): 8.3 (pinning via `/truthid/v1/pin`, pinando o SQLite real) implementada, pin real validado (Kubo local); Sessão 33 (2026-07-29): suspeita de rede errada CONFIRMADA — `/sign-request` do TruthID sempre executa em Base Mainnet, hardcoded, sem seleção de rede por chamada — 8.3 bloqueada até o TruthID resolver isso, só documentado por ora; Sessão 39 (2026-08-01): 8.6 (transporte cross-device LAN+dead-drop) estendido do `/sign-request` pro `/pin` também, ponta requisitante completa; Sessão 54 (2026-08-02): a UI da prova de conceito saiu do nav principal (era aba "TruthID Sync") e virou uma seção dentro de Configurações (`TruthIdSettingsSection.tsx`), sem mudança de funcionalidade; restam 8.4 (consumir `/sign-message` pra criptografia real) e 8.5 (integração fim a fim publish/pull), ambas não iniciadas; ver Fases Detalhadas
Fase 9 — Tela de Research (unificada, ex-"Stock Lookup")  [x] Completa — 9.1 a 9.5 concluídas; Sessão 43 (2026-08-02) generalizou a tela pra qualquer classe com dado automático (não só ação): ganhou seletor de classe explícito ao lado do ticker (Stock/FII), testada no app real; renomeada de "Pesquisa" pra "Research" na Sessão 48 (tradução da UI pra inglês, Fase 6.1)
Fase 10 — Remodelagem para Workspaces Colaborativos & Multi-Ativos [~] Em andamento — 10.1 (fundação Workspace/Portfolio) e 10.2 (multi-ativos, fatia Ação BR — cadastro virou busca automática por ticker, Sessão 34) até 10.6 (seletor de múltiplos Workspaces, tela de entrada) implementadas e testadas no app real (Sessões 33-38, 2026-07-29 a 07-31), incl. 10.3 (rentabilidade histórica TWR/Dietz), 10.4 (watchlists nomeadas + favoritos) e 10.5 (teses de investimento + anexos locais); só resta **10.7** (convite/RBAC multiusuário real via TruthID — bloqueada, depende de resolver a permissão descentralizada, amarrada à Fase 8); Sessões 41-46 (2026-08-02) ampliaram as classes de ativo (item 8) com **FII** — busca automática por ticker + TWR/Dietz estendida + indicadores fundamentalistas direto da **CVM** (vacância/inadimplência por imóvel, patrimônio líquido, VP da cota, priorizados sobre agregador terceiro) + análise unificada na Fase 9 (ver acima) + bugfix real (proventos de FII vinham vazios) + Pesquisa/Ativos convergentes (cadastrar por Pesquisa, CNPJ sincronizado nas duas telas), tudo **testado no app real**; Sessões 49-50 acrescentaram **ETF (B3)** na mesma fatia de cotação automática, com tela de análise própria (`EtfLookupSection.tsx` — cotação/SMA/CAGR/proventos, sem fundamentos/DCF/"preço teto", que não fazem sentido pra fundo de índice); indicador de fundo específico não deu pra fazer (CVM não tem categoria de dados abertos pra fundo de índice, achado pesquisando de verdade, nem Yahoo/bolsai têm composição/índice replicado/site); Sessão 51 acrescentou **Cripto** — primeira classe expandida sem fonte Yahoo `.SA` (cripto não é listada na B3), cotação/histórico via CoinGecko, unificada em Research (`CryptoLookupSection.tsx`, absorveu a antiga aba solta "Crypto Score") com SMA 50/100/200 e Fear & Greed Index global (`alternative.me`, componente reutilizável `CryptoFearGreedGauge`); Sessão 52 deu a todas as 4 classes um **gráfico de preço** com seletor de período (30D/12M/YTD/5Y/10Y/All, `PriceHistoryChart.tsx`, Recharts, recorte client-side) — Ação/FII/ETF passaram a alimentar `stock_price_history` automaticamente em toda busca (antes só via backfill manual), igualando o comportamento que Cripto já tinha; Sessão 53 acrescentou **BDR (B3)** — quinta classe expandida, a mais barata até agora (mesmo endpoint Yahoo `.SA`, zero coletor/migration novos), exposição default "US" editável, `BdrLookupSection.tsx` (cópia adaptada de `EtfLookupSection.tsx`); Sessão 55 acrescentou **Metal** — sexta classe expandida, segunda sem fonte Yahoo `.SA` (usa contratos futuros COMEX/NYMEX), `MetalLookupSection.tsx`; Sessão 57 revisou a unidade pra onça troy (a real do contrato, sem conversão — reverte a fatia de grama da Sessão 55) e ampliou pra **Ouro/Prata/Platina/Paládio** (`XAU`/`XAG`/`XPT`/`XPD`), indicador simples "US$/g" só de referência; demais classes (Stocks intl/Tesouro Direto/Renda Fixa/REIT/Imóvel/Empresa não listada) seguem manuais; usuário sinalizou (Sessão 37) que a UX atual (forms soltos em abas) não é a visão final, revisitar depois — ver Fases Detalhadas
Fase 11 — Rebranding (Anchor), App Mobile & Empacotamento [ ] Não iniciada — ideia registrada na Sessão 30: novo nome **Anchor**, app mobile completo em **Flutter**, empacotamento do sidecar Python (PyInstaller/Nuitka) + CI GitHub Actions gerando instaladores standalone do desktop — ver Fases Detalhadas
```

---

# Ambiente de Desenvolvimento

**Docker** — decidido na Sessão 1, mesmo padrão usado no TruthID: um único container com Node + Rust + WebKitGTK (pra abrir a janela do Tauri) e também **Python3 + venv** (pra rodar os coletores de dados chamados pelo próprio app). `docker compose up` sobe tudo, X11 do host repassado pro container pra a janela do app aparecer na tela — nada precisa ser instalado na máquina.

Diferente do TruthID (que precisava de acesso a USB pra Ledger), este projeto não mexe com hardware — o container fica mais simples e menos privilegiado (sem `privileged: true`, sem montar `/dev`).

Criado na Fase 0.5: `desktop/Dockerfile`, `desktop/docker-compose.yml`, `desktop/dev.sh` (`xhost +local:docker && docker compose up`).

**⚠️ Cuidado (achado na Sessão 1)**: a pasta do app se chama `desktop/`, igual a do TruthID — sem um `name:` explícito no topo do `docker-compose.yml`, o Compose usa o nome da pasta como nome do projeto e **colide** com as imagens/volumes do TruthID (`desktop-desktop`, `desktop_cargo-*`). Por isso o `docker-compose.yml` daqui tem `name: practice-valuation` logo na primeira linha — não remover.

**⚠️ Cuidado com espaço em disco**: a máquina roda os dois projetos (TruthID + Practice Valuation) e o disco de 32GB vive perto do limite por causa dos caches Docker do TruthID (imagens Flutter/Gradle/NDK, cache do cargo). Antes de builds Docker pesados, checar `df -h /` — na Sessão 1 o disco chegou a 100% (0 disponível) durante o setup inicial e isso **causou perda de arquivos** (os 3 arquivos de Docker recém-criados sumiram no meio de uma operação). `docker image prune -f` remove imagens órfãs com segurança (não mexe em nada usado pelo TruthID); ir além disso (limpar volumes/imagens nomeadas do TruthID) é decisão do usuário, não fazer sem perguntar.

**Limpeza da Sessão 103 (2026-10-15)**: liberados ~45 GB, autorizado pelo dono do projeto. Saíram: cache de build do Docker (13 GB), volume `anchor_cargo-target` (8,6 GB, recriado no próximo `./dev.sh`), imagem `mobile-flutter` (6 GB, os testes do Flutter agora rodam no CI), sobras do TruthID (`desktop-desktop` + volumes `desktop_cargo-*`, só cache, parados havia 3 meses) e o `desktop/src-tauri/target/` do host (8,6 GB). **Ficaram** (são dados ou estão em uso): `anchor_app-data`, `lume_db-data`, `anchor_cargo-registry`, imagens `anchor-desktop`/`lume-api`/`postgres`. O `[profile.dev]` do `Cargo.toml` passou a gerar menos debug info (`line-tables-only` no nosso código, nenhuma nas dependências), pro `target/` crescer bem menos que os ~8 GB de antes.

## CI no GitHub Actions (Sessão 103)

Testes e builds rodam no GitHub, não na máquina local (pedido do dono do projeto: os builds estavam pesando em disco e hardware). O repo é público, então o Actions é grátis e sem limite de minutos nos runners padrão. Os workflows ficam em `.github/workflows/` e os resultados em github.com/Arven-Labs/anchor/actions:

| Workflow | Quando roda | O que faz |
|---|---|---|
| `ci-desktop.yml` | push no `main`/PR que mexa em `desktop/` | `npm run build` (`tsc` + vite) e `cargo test --lib` (os `#[ignore]` ao vivo ficam de fora). Cria um sidecar vazio porque o `tauri-build` exige o arquivo do `externalBin` |
| `ci-contracts.yml` | push/PR em `contracts/` | `forge test` |
| `ci-mobile.yml` | push/PR em `mobile/` | `flutter analyze` + `flutter test` (substitui `./dev.sh flutter test` no Docker local) |
| `build.yml` | tag `v*` | instaladores Linux/Windows/macOS num release em rascunho (ainda nunca rodou) |

Todos têm `workflow_dispatch` (botão "Run workflow" na aba Actions).

**Fluxo de desenvolvimento a partir da Sessão 103:**
- Rodar o app pra testar tela continua local (`desktop/dev.sh`); não tem como sair da máquina.
- **Não rodar `cargo test`/`tsc`/`flutter test`/`forge test` localmente por padrão.** Commitar, dar push e conferir o resultado com `gh run list` / `gh run watch <id>` (ou na aba Actions). Só rodar local quando precisar iterar rápido num teste específico.
- Os testes `#[ignore]` (ao vivo contra o Lume) não rodam no CI. Continuam manuais e locais.
- Uma fase só conta como "verificada" com o CI verde no commit dela.

---

# Arquitetura de Código

Decidido na Sessão 1: mesmo sendo um projeto pessoal, vale organizar bem desde o início — "fácil manutenção" não significa construir mais funcionalidade agora, significa estruturar bem o pouco que já existe.

**Camadas no lado Rust** (convenção adotada, aplica a partir da Fase 3):
- **`commands/`** — a "cola" exposta ao React via `invoke()`. Fina: só recebe o pedido e chama a camada de baixo, não tem regra de negócio
- **`domain/`** (ou `valuation/`) — as funções puras de cálculo (Bazin, DCF, etc.) e a lógica do score cripto. Não sabem nada de banco nem de Tauri — só recebem números/dados, devolvem números/resultado. É a "função pura" já mencionada na Fase 3, só que com um lugar físico definido
- **Repository/entities (SeaORM)** — só sabe conversar com o banco

Princípio: não misturar regra de negócio com acesso a banco — o mesmo motivo pelo qual qualquer linguagem separa "service layer" de "data layer".

**Tratamento de erro (Rust)**: **`thiserror`** — um enum de erro próprio (`AppError::AssetNotFound`, `AppError::InvalidGuard`, etc.) que serializa pro React como JSON estruturado (`{ code, message }`), em vez de string solta. Decidido na Sessão 1 em vez de `anyhow` (mais genérico/dinâmico, bom pra prototipagem rápida, mas não dá pra distinguir tipos de erro na UI depois).

**Busca de dados no React**: **TanStack Query** pra chamar os comandos Tauri (`invoke()`) — cuida de cache, loading, erro e refetch de forma consistente em toda tela, em vez de cada componente reinventar isso com `useState`/`useEffect`. Decidido na Sessão 1; mesma família do TanStack Table já escolhido pra grid (Fase 0.4).

**Testes**: funções de `domain/` são puras (input → output, sem I/O) — dá pra testar sem precisar de banco nem de mock, então a prática é escrever teste unitário junto de cada função de cálculo conforme ela é escrita (não é uma decisão de infraestrutura, é só disciplina a manter).

