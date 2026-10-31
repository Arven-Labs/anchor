# Anchor — Estado do Projeto

> Este projeto usa a mesma estrutura de arquivos do TruthID (outro projeto do mesmo dono): o estado
> foi dividido em arquivos menores dentro desta pasta (`project/`), em vez de um único
> `PROJECT_STATE.md` monolítico na raiz.
> **Estado atual**: a última entrada de `SESSIONS.md` (o log é cronológico, a mais recente fica no
> fim). O que sobra por fazer está em `ROADMAP.md` e no "Pendente" dessa última entrada. Este
> arquivo deixou de carregar um resumo da última sessão (parou na Sessão 106 e duplicava o log).

---

## Como Usar Este Arquivo

Leia o arquivo relevante para o que você precisa:

| Para saber sobre | Leia |
|---|---|
| **Como a IA e o dono trabalham juntos** (fluxo por branch + PR, plano, testes, checklist de fechamento de sessão) — fica no `AGENTS.md` do workspace que contém este repo | **`../AGENTS.md`** (relativo à raiz do repo) |
| Diretrizes de código, segurança e ensino; ritmo do projeto | `GUIDELINES.md` |
| Visão geral, stack, status das fases, ambiente de desenvolvimento, arquitetura de código | `OVERVIEW.md` |
| **Todas as fases detalhadas (0 a 10)** | **`PHASE.md`** |
| Decisões de arquitetura em aberto | `ARCHITECTURE.md` |
| Roadmap, evoluções planejadas | `ROADMAP.md` |
| Log completo de sessões de trabalho | `SESSIONS.md` |
| **Conferir telas por código (capturas no CI) e automatizar fluxos** — o processo, as regras e o que cada uma evitou | **`AUTOMACAO.md`** |

**Ao começar uma sessão**: diga ao Claude Code "leia os arquivos em `project/` e me ajude a continuar". Antes de qualquer tarefa, confira PR aberto ou branch já mesclada sobrando (`AGENTS.md` do workspace, §14, item 5)
**Ao validar código**: `cd desktop && ./dev.sh check` (tsc + cargo check, leve) enquanto desenvolve; depois dê push da branch e confira o CI no GitHub Actions (`gh run list`), em vez de rodar os testes localmente. Ver `OVERVIEW.md`, "CI no GitHub Actions"
**Ao terminar uma sessão**: o trabalho vai em branch + Pull Request (a `main` é protegida; só o dono mescla). Atualize o Log de Sessões em `SESSIONS.md` e marque etapas concluídas em `PHASE.md` **dentro do próprio PR de código**, com o resultado real do CI, e feche com a checklist da §14 do `AGENTS.md` do workspace (git, PRs, CI, registro, mesclar ou não, o que ficou em aberto)
**Ao tomar uma decisão**: registre em `ARCHITECTURE.md`
**Ao mudar de máquina**: sincronize via git
