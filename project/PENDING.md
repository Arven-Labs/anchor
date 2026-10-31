# Pendências do Projeto

> Criado na Sessão 272 para os alertas do Dependabot (procedimento `padrao/dependencias.md`, ADR 0006 do workspace). As demais pendências do anchor continuam no fim do `SESSIONS.md` (decisões da Sessão 271 e anteriores).
> Resolvida: marcar `✅ Resolvida` com a sessão. Contagem de alertas sempre relida da API.
>
> Última atualização: 2026-10-31 (Sessões 272 e 273).

---

## Não Resolvidas

### Dependências (Dependabot)

Contagem: **110 abertos** antes (2026-10-31). **Fim da Sessão 272, relida da API (2026-10-31): 0 abertos, 101 `fixed`, 11 `dismissed`, 2 `auto_dismissed`.**
- **Os 101 `fixed`** são os alertas cujo lockfile corrigido (PRs #37, #38, #39 e #44) o GitHub só reavaliou **mais de um dia depois** do merge do #37 (a API ficou em 110 abertos, 0 `fixed`, e depois passou de 101 para 2 de uma vez). Era atraso do GitHub, não problema nosso. Antes disso a contagem intermediária passou por 106, 105 e 104 (atraso de leitura da API).
- **Os 11 `dismissed` são as dispensas feitas por você**, cada uma com motivo e comentário no GitHub: #1, #2, #6 (`serialize-javascript`), #3, #4 (`extract-zip`), #95 (`deepmerge`), #113, #114 (`minimatch`) como `not_used`; #7, #8 (`xlsx`) e #100 (`glib`) como `tolerable_risk`. **Dispensar não corrige nada**: só tira o alerta da contagem. Os `auto_dismissed` (2) foram fechados pelo GitHub, não por nós; não investiguei quais são.
- **`minimatch` (#113, #114, GHSA-g3hv-75wg-m249, médio)**: surgiu às 21:25, logo depois da reavaliação. Só a 11.0.1 corrige, e nenhum pai aceita a 11.x (`@ts-morph/common` 0.29 pede `^10`, `readdir-glob` `^10.2.2`, `@wdio/config` usa `glob` 13 → 10.x; no E2E há 4 versões no lock: 3.1.5, 5.1.9, 9.0.9, 10.2.6). Ambos `development` (`shadcn` → `ts-morph`; `wdio`/`mocha`/`webdriverio`). `overrides` para a 11.x foi descartado: quebra a API que os pais esperam. **Reabrir se `ts-morph`, `wdio` ou `mocha` passarem a aceitar `minimatch` 11.** Não li o código de cada ferramenta para confirmar que o glob vem só de configuração local.
- **Lição do processo**: o `dependabot/alerts` pode demorar **mais de um dia** para refletir lockfile corrigido, e novos advisories aparecem na reavaliação. Reler a contagem depois, em vez de abrir PR para "forçar".

| ID | Item | Onde se originou | Prioridade |
|---|---|---|---|
| P2 | **`xlsx` 0.18.5, 2 alertas (prototype pollution, ReDoS), sem patch no npm** (o SheetJS saiu do registro). Lê só planilhas que **você** escolhe (`ManualAssetDetails.tsx:226`, `ThesisSection.tsx:263`), sem entrada remota. ✅ **Dispensado** (#7, #8, `tolerable_risk`, Sessão 272, comentário no GitHub). **Reabrir se o app passar a ler planilhas de terceiros.** | Sessão 272 | 🟢 Dispensado |
| P3 | **`deepmerge` 4.3.1, 1 alerta baixo, sem patch.** Vem só de `shadcn` (ferramenta de scaffolding). ✅ **Dispensado** (#95, `not_used`, Sessão 272): `shadcn` agora é `devDependency` (PR #38). | Sessão 272 | 🟢 Dispensado |
| P4 | **`extract-zip` (2, sem patch) e `serialize-javascript` (3) em `desktop/e2e/package-lock.json`**, escopo `development`: entram por `@wdio/cli` e `mocha`, só no teste E2E. `serialize-javascript` 6.0.2 → 7.x cruza major. ✅ **`extract-zip` dispensado** (#3, #4, `not_used`, Sessão 272). ✅ **`serialize-javascript` dispensado** (#1, #2, #6, `not_used`, Sessão 272): **têm patch** (7.0.3, 7.0.5 e 7.1.3), mas o `mocha` 10 (via `@wdio/mocha-framework` 9) pede a 6.x; a linha oficial com a 7.x é o `@wdio/mocha-framework` 10 (mocha 12), um major do WebdriverIO inteiro. Só roda no E2E do CI. **Reabrir se o mocha passar a serializar dado externo, ou ao subir o wdio para 10.** | Sessão 272 | 🟢 Dispensado |
| P5 | **`glib` 0.18.5, 1 alerta médio (`VariantStrIter`), corrigido em 0.20.** Vem de `tauri → muda → gtk → glib` (GTK3); só o Tauri pode trocar. Não li o código do Tauri/GTK. ✅ **Dispensado** (#100, `tolerable_risk`, Sessão 272, comentário no GitHub). **Reabrir se o Tauri sair do GTK3 ou se o `gtk` usar o `VariantStrIter`** (não li esse código; o nosso `src-tauri/src` não usa `glib`). | Sessão 272 | 🟢 Dispensado |

### Não verificado pelo procedimento
`dist/` (se a árvore do `shadcn` é empacotada), `cargo audit`, `npm audit --omit=dev`, AES nos zips, a classificação de cada alerta individual dentro dos pacotes agrupados.

---

## Resolvidas

- **P1 — `zip` (Rust), 3 alertas, 1 crítico (Zip Cloaking)** ✅ Resolvida no código na Sessão 272 (PR #39, mesclado em 2026-10-31 18:50): `zip` 2 → 9 e `calamine` 0.31 → 0.37; o `Cargo.lock` passou de duas cópias (`zip 2.6.1` direto e `zip 4.6.1` via `calamine`) para uma, `zip 9.2.0`. Antes: os avisos eram de **extração para o disco**, que o código não faz (lê entradas para memória), então eram não alcançáveis; o AES (alerta 107) nunca foi compilado, porque o `Cargo.toml` usa `default-features = false` e o `calamine` pede o mesmo (`deflate`, conferido na API do crates.io); o `cargo tree -e features` que li estava truncado e só listou features de deflate, então **não conferi o grafo de features inteiro**. Subiu mesmo assim por tirar as cópias antigas e cobrir zip malformado vindo da CVM/B3. Mudança de API: `file_names()` do `zip` 9 devolve `Result<Cow<str>>`; ajustados `cvm_fii.rs` e `cvm_inf_diario.rs` (`filter_map(Result::ok)`). Entram `typed-path` e `nix` no lock. **Teste novo** `read_xlsx_rows_reads_text_numbers_bool_and_date` (e um de erro): `read_xlsx_rows` não tinha teste automático (só `#[ignore]` com planilha privada), e o teste passou no CI com as versões antigas (`dea0d58`) e com as novas (`929aebe`). **Não coberto**: leitura de uma planilha real da B3 (`ANCHOR_B3_SAMPLE`, privada, não rodada) e o `Data::DateTime` de planilhas com formatos de data diferentes do `numFmtId` 14. O alerta só some da API depois do merge e da reavaliação do GitHub.
- **P7 — `react-is` não declarado** ✅ Resolvida na Sessão 272 (PR #44, a mesclar): `"react-is": "^19.2.7"` em `dependencies` do `desktop/package.json`; no lock só entrou a linha na raiz e saiu o marcador `"peer": true` (nenhuma versão mudou). Prova em cópia no scratchpad: com `npm install --legacy-peer-deps --package-lock-only`, o `react-is` **saía** do lock antes (1 → 0) e **permanece** depois. CI (commit `7541ae1`, igual ao `headRefOid`): `e2e`, `frontend / build`, `gate`, `guard` e `changes` verdes; `rust`, `contracts` e `mobile` pulados. **Não rodei** o `./dev.sh check` (o `npm install` completo poderia reescrever o lock e não exercita a mudança). O `--legacy-peer-deps` do CI segue como está.
- **P6 — `shadcn` em `dependencies`** ✅ Resolvida na Sessão 272 (PR #38): movido para `devDependencies`; o lockfile só ganhou marcadores `"dev": true` e `npm ls --omit=dev --package-lock-only` não lista mais `shadcn`. CI do PR (commit `ae6f364`): `e2e`, `frontend / build`, `gate`, `guard` e `changes` verdes; `rust` pulado.

**Ainda por registrar:** os alertas de `rustls`, `vite`, `source-map-js` e os 20 pacotes puxados por `shadcn` (corrigidos no lockfile pelo PR #37, na `main` desde `fad3252`) só entram aqui quando o GitHub fechá-los. Em 2026-10-31, 20 minutos depois do merge, a API ainda mostrava **110 abertos e 0 fechados**, embora a `main` já tenha as versões corrigidas (`vite` 7.3.7, `undici` 7.30.1, `rustls` 0.23.46); o warden teve o mesmo atraso. Reler a contagem depois.
