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
| P1 | **`zip` (Rust), 3 alertas, 1 crítico (GHSA de "Zip Cloaking"), corrigidos só em `zip` ≥ 9.0.5.** Duas cópias no `Cargo.lock`: `zip 2.6.1` direto (`Cargo.toml:42`) e `zip 4.6.1` via `calamine 0.31`. Código lido: `cvm_ipe.rs:62`, `cvm_fii.rs:84`, `b3_taxa_swap.rs:53` só **leem a primeira entrada** para memória/CSV, sem extrair para o disco (os avisos crítico e de symlink são de extração); `ZipWriter` só em testes. Classificação: **não alcançável** para o crítico, com ressalva de zip malformado vindo da CVM/B3 (negação de serviço). **Não verificado**: se algum zip vem com AES. Correção: PR 3, salto de major 2 → 9 e upgrade do `calamine` (§4, pede confirmação). | Sessão 272 | 🟠 Média |
| P2 | **`xlsx` 0.18.5, 2 alertas (prototype pollution, ReDoS), sem patch no npm** (o SheetJS saiu do registro). Lê só planilhas que **você** escolhe (`ManualAssetDetails.tsx:226`, `ThesisSection.tsx:263`), sem entrada remota. Candidato a dispensa `tolerable_risk` (**não dispensado**, depende da sua confirmação). | Sessão 272 | 🟡 Baixa |
| P3 | **`deepmerge` 4.3.1, 1 alerta baixo, sem patch.** Vem só de `shadcn` (ferramenta de scaffolding). Fica aberto até o PR 2 e a releitura da API; depois, candidato a `not_used`. | Sessão 272 | 🟡 Baixa |
| P4 | **`extract-zip` (2, sem patch) e `serialize-javascript` (3) em `desktop/e2e/package-lock.json`**, escopo `development`: entram por `@wdio/cli` e `mocha`, só no teste E2E. `serialize-javascript` 6.0.2 → 7.x cruza major. Candidatos a `not_used` (**não dispensados**). | Sessão 272 | 🟡 Baixa |
| P5 | **`glib` 0.18.5, 1 alerta médio (`VariantStrIter`), corrigido em 0.20.** Vem de `tauri → muda → gtk → glib` (GTK3); só o Tauri pode trocar. Não li o código do Tauri/GTK. Candidato a `tolerable_risk` (**não dispensado**). | Sessão 272 | 🟡 Baixa |
| P7 | **`react-is` não declarado:** é peer dependency do `recharts`, só existe no lock por acidente; um `npm update --legacy-peer-deps` o remove e quebra o build. Declará-lo no `package.json` é mudança de manifesto (decisão sua). | Sessão 272 | 🟠 Média |

### Não verificado pelo procedimento
`dist/` (se a árvore do `shadcn` é empacotada), `cargo audit`, `npm audit --omit=dev`, AES nos zips, a classificação de cada alerta individual dentro dos pacotes agrupados.

---

## Resolvidas

- **P6 — `shadcn` em `dependencies`** ✅ Resolvida na Sessão 272 (PR #38): movido para `devDependencies`; o lockfile só ganhou marcadores `"dev": true` e `npm ls --omit=dev --package-lock-only` não lista mais `shadcn`. CI do PR (commit `ae6f364`): `e2e`, `frontend / build`, `gate`, `guard` e `changes` verdes; `rust` pulado.

**Ainda por registrar:** os alertas de `rustls`, `vite`, `source-map-js` e os 20 pacotes puxados por `shadcn` (corrigidos no lockfile pelo PR #37, na `main` desde `fad3252`) só entram aqui quando o GitHub fechá-los. Em 2026-10-31, 20 minutos depois do merge, a API ainda mostrava **110 abertos e 0 fechados**, embora a `main` já tenha as versões corrigidas (`vite` 7.3.7, `undici` 7.30.1, `rustls` 0.23.46); o warden teve o mesmo atraso. Reler a contagem depois.
