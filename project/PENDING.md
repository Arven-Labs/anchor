# Pendências do Projeto

> Criado na Sessão 272 para os alertas do Dependabot (procedimento `padrao/dependencias.md`, ADR 0006 do workspace). As demais pendências do anchor continuam no fim do `SESSIONS.md` (decisões da Sessão 271 e anteriores).
> Resolvida: marcar `✅ Resolvida` com a sessão. Contagem de alertas sempre relida da API.
>
> Última atualização: 2026-10-31 (Sessão 272).

---

## Não Resolvidas

### Dependências (Dependabot)

Contagem: **110 abertos** antes (2026-10-31). Depois do PR #37: a reler da API após o merge.

| ID | Item | Onde se originou | Prioridade |
|---|---|---|---|
| P2 | **`xlsx` 0.18.5, 2 alertas (prototype pollution, ReDoS), sem patch no npm** (o SheetJS saiu do registro). Lê só planilhas que **você** escolhe (`ManualAssetDetails.tsx:226`, `ThesisSection.tsx:263`), sem entrada remota. Candidato a dispensa `tolerable_risk` (**não dispensado**, depende da sua confirmação). | Sessão 272 | 🟡 Baixa |
| P3 | **`deepmerge` 4.3.1, 1 alerta baixo, sem patch.** Vem só de `shadcn` (ferramenta de scaffolding). Fica aberto até o PR 2 e a releitura da API; depois, candidato a `not_used`. | Sessão 272 | 🟡 Baixa |
| P4 | **`extract-zip` (2, sem patch) e `serialize-javascript` (3) em `desktop/e2e/package-lock.json`**, escopo `development`: entram por `@wdio/cli` e `mocha`, só no teste E2E. `serialize-javascript` 6.0.2 → 7.x cruza major. Candidatos a `not_used` (**não dispensados**). | Sessão 272 | 🟡 Baixa |
| P5 | **`glib` 0.18.5, 1 alerta médio (`VariantStrIter`), corrigido em 0.20.** Vem de `tauri → muda → gtk → glib` (GTK3); só o Tauri pode trocar. Não li o código do Tauri/GTK. Candidato a `tolerable_risk` (**não dispensado**). | Sessão 272 | 🟡 Baixa |
| P7 | **`react-is` não declarado:** é peer dependency do `recharts`, só existe no lock por acidente; um `npm update --legacy-peer-deps` o remove e quebra o build. Declará-lo no `package.json` é mudança de manifesto (decisão sua). | Sessão 272 | 🟠 Média |

### Não verificado pelo procedimento
`dist/` (se a árvore do `shadcn` é empacotada), `cargo audit`, `npm audit --omit=dev`, AES nos zips, a classificação de cada alerta individual dentro dos pacotes agrupados.

---

## Resolvidas

- **P1 — `zip` (Rust), 3 alertas, 1 crítico (Zip Cloaking)** ✅ Resolvida no código na Sessão 272 (PR #39, a mesclar): `zip` 2 → 9 e `calamine` 0.31 → 0.37; o `Cargo.lock` passou de duas cópias (`zip 2.6.1` direto e `zip 4.6.1` via `calamine`) para uma, `zip 9.2.0`. Antes: os avisos eram de **extração para o disco**, que o código não faz (lê entradas para memória), então eram não alcançáveis; o AES (alerta 107) nunca foi compilado, porque o `Cargo.toml` usa `default-features = false` e o `calamine` pede o mesmo (`deflate`, conferido na API do crates.io); o `cargo tree -e features` que li estava truncado e só listou features de deflate, então **não conferi o grafo de features inteiro**. Subiu mesmo assim por tirar as cópias antigas e cobrir zip malformado vindo da CVM/B3. Mudança de API: `file_names()` do `zip` 9 devolve `Result<Cow<str>>`; ajustados `cvm_fii.rs` e `cvm_inf_diario.rs` (`filter_map(Result::ok)`). Entram `typed-path` e `nix` no lock. **Teste novo** `read_xlsx_rows_reads_text_numbers_bool_and_date` (e um de erro): `read_xlsx_rows` não tinha teste automático (só `#[ignore]` com planilha privada), e o teste passou no CI com as versões antigas (`dea0d58`) e com as novas (`929aebe`). **Não coberto**: leitura de uma planilha real da B3 (`ANCHOR_B3_SAMPLE`, privada, não rodada) e o `Data::DateTime` de planilhas com formatos de data diferentes do `numFmtId` 14. O alerta só some da API depois do merge e da reavaliação do GitHub.
- **P6 — `shadcn` em `dependencies`** ✅ Resolvida na Sessão 272 (PR #38): movido para `devDependencies`; o lockfile só ganhou marcadores `"dev": true` e `npm ls --omit=dev --package-lock-only` não lista mais `shadcn`. CI do PR (commit `ae6f364`): `e2e`, `frontend / build`, `gate`, `guard` e `changes` verdes; `rust` pulado.

**Ainda por registrar:** os alertas de `rustls`, `vite`, `source-map-js` e os 20 pacotes puxados por `shadcn` (corrigidos no lockfile pelo PR #37, na `main` desde `fad3252`) só entram aqui quando o GitHub fechá-los. Em 2026-10-31, 20 minutos depois do merge, a API ainda mostrava **110 abertos e 0 fechados**, embora a `main` já tenha as versões corrigidas (`vite` 7.3.7, `undici` 7.30.1, `rustls` 0.23.46); o warden teve o mesmo atraso. Reler a contagem depois.
