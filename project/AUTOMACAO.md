# Verificação por código e automação de fluxos

> Origem: Sessão 156 do Anchor (2026-10-18). O dono do projeto disse: "prefiro seguir por código, não gosto de fazer essas coisas manualmente,
> não tem como automatizar isso?", e confirmou que o que ele não quer fazer à mão é basicamente **(1) conferir telas** e **(2) disparar passos
> de um fluxo**. Este documento é o processo que saiu daí, escrito para valer em qualquer projeto dele (o Warden usa o mesmo; ver a seção 6).
> Cada regra abaixo veio de algo que aconteceu de verdade; o "porquê" está junto.

## 1. Regras de ouro

1. **Não devolver trabalho ao dono.** Nada de fechar uma fatia com "abra o app e confira a tela" ou "rode a estimativa na sua conta". Se o passo é
   conferir, o Claude confere por código; se é disparar, o fluxo dispara sozinho.
2. **O que é manual por natureza fica dito com clareza, uma vez**, como limite, não como tarefa nem como opção da lista de próximos passos
   (exemplo: o saldo que o banco mostra só o dono sabe, então a conciliação por saldo segue manual).
3. **Respeitar as regras do projeto onde se trabalha** (CI em vez de build local no Warden, plan mode a cada item novo, código em inglês, editar com
   Edit/Write para o diff ficar visível). Este processo se encaixa nelas, não as substitui.

## 2. Parte A — Conferir as telas por código

**Ideia**: o E2E que já roda no CI passa a **fotografar** as telas-chave; o CI sobe as fotos como artefato **sempre** (não só em falha); o Claude
baixa o artefato e **olha cada imagem** (a ferramenta de leitura de arquivos mostra PNG). Testes de unidade e de fluxo não pegam defeito visual;
as capturas pegaram quatro de uma vez (seção 2.3).

### 2.1 Como montar
- Um helper `snap("nome")` nos specs, que grava `artifacts/screens/<nome>.png`, com **o nome do spec ou da tela** e **nunca derruba o teste** (`try/catch`,
  só `console.warn`). No Anchor: `desktop/e2e/specs/helpers.mjs` (WebdriverIO + tauri-driver: `browser.saveScreenshot`). Em Playwright:
  `page.screenshot({ path })`; em Flutter, um teste de integração/golden que salva a imagem.
- Chamar `snap()` **depois** da tela estar pronta (depois do `waitForText`/`waitFor` do que importa), uma vez por estado interessante: a tela cheia, o popup
  aberto, o estado de erro, o estado vazio.
- **Rolar o elemento para a vista antes de fotografar** (`scrollIntoView({ block: "center" })`): a janela do E2E é 1280×800 e o que fica abaixo da dobra sai
  de fora (o widget da Home saiu cortado na primeira captura).
- O workflow do CI sobe a pasta com `actions/upload-artifact@v4`, `if: always()`, `retention-days: 14`, `if-no-files-found: ignore`.
- A pasta de artefatos fica no `.gitignore`.

### 2.2 Como olhar
1. Dar `git push` e esperar o CI (`gh run watch <id> --exit-status`).
2. Baixar para um diretório **novo e vazio** do scratchpad: `gh run download <run-id> -n e2e-artifacts -D <dir>`.
3. Abrir cada PNG com a ferramenta de leitura e **procurar de verdade**: número estranho (`-0,00`, `NaN`, `undefined`), texto que descreve a coisa errada,
   texto em idioma errado ou jargão interno (nomes de campo, ids), elemento invisível ou sem contraste no tema, texto cortado, botão fora da área, contador
   sem sentido. Anotar o defeito, corrigir, fotografar de novo e conferir a nova captura.
4. Registrar no `SESSIONS.md` o que a conferência achou e que foi corrigido.

### 2.3 O que as capturas acharam no Anchor (e nenhum teste pegava)
- `BRL -0,00` na Home: zero negativo vindo de um `-0.0`; corrigido normalizando `|valor| < 0,005` para 0 no formatador.
- Texto errado de contexto: o popup de uma **estimativa** dizia "exatamente o que o arquivo da B3 explica"; passou a dizer "esta estimativa".
- Jargão interno na tela: o contador mostrava "line 2 · porquinho_estimate" (linha do arquivo e nome da origem); passou a dizer "estimate".
- Caixas de seleção nativas **pretas sobre fundo escuro**, quase invisíveis; trocadas por uma caixa própria com borda e ✓.

### 2.4 Cuidados
- **Só dados inventados** nas capturas (as dos E2E já são): o artefato do CI pode ser visto por quem tem acesso ao repositório e o repositório pode ser público.
  Nunca fotografar dados reais do dono.
- Não depender da captura para decidir o teste: a captura é para o olho; a asserção continua sendo por texto/DOM.

## 3. Parte B — Automatizar fluxos

Exemplo guia (Anchor, Sessões 155–156): estimar sozinho o rendimento do porquinho depois que o extrato é revisto. O que se aprendeu, em regras:

1. **Escolher o gatilho pelo estado dos dados, não pelo evento óbvio.** "Logo depois de importar" estava errado: o extrato importado fica numa fila e só entra no
   ledger quando aprovado, e a estimativa lê o ledger; estimar com a revisão pela metade erra. O gatilho certo foi **quando a fila esvazia** (aprovar ou descartar o
   último item). Perguntar "em que momento os dados que o passo precisa já estão completos?".
2. **Melhor esforço: o passo automático nunca falha a ação do usuário.** Erro (sem rede, série de dados faltando) vai para o log e a aprovação segue valendo.
   Um teste cobre isso.
3. **Idempotente**: rodar de novo não duplica (o dia que já tem resultado é pulado; deduplicação por chave). E **convive com a fonte exata**: quando chega o dado
   real (a B3), ele substitui a estimativa pendente do mesmo dia e, se a estimativa já foi aprovada, é pulado com aviso para não contar duas vezes.
4. **O botão manual continua de reserva**, para forçar, refazer e ver o resultado. Automático não é o fim do botão.
5. **Ligar sozinho só com um sinal confiável, e a marca precisa de três estados.** A conta virou "porquinho" na primeira transferência de porquinho aprovada (o único
   sinal confiável; uma conta corrente com transferências entre contas próprias não pode ser estimada). A marca é **nulo = ainda não decidido**, **0 = desligada de
   propósito**, **> 0 = ligada**. Com só dois estados, desligar gravando nulo era desfeito na transferência seguinte; **um teste de fluxo achou isso**.
6. **O resultado passa pela revisão antes de gravar.** Automático cria *propostas* na fila de Pending, não lançamentos. O dono aprova ou descarta; descartar não refaz.
7. **Nada de rede em teste.** A variável de ambiente que desliga a coleta em segundo plano (`ANCHOR_DISABLE_INGESTION`) também faz o passo automático ler só o
   que está gravado; o banco de teste a define. Os testes do passo semeiam os dados de que ele precisa.
8. **Mostrar o estado na tela**: um selo na conta ("Auto yield 100% CDI") e, no diálogo, uma frase de estado com botão ligar/desligar. O dono não deve precisar
   adivinhar se está ligado.
9. **Validar o modelo com dados reais antes de construir** quando o passo estima algo: aqui, o modelo foi medido fora do app contra os 23 resgates que a B3
   explica (erro mediano de 0,4% a 100% do CDI) antes de escrever uma linha do app. Se não passar, não vale automatizar.

## 4. Ritual de cada fatia (continua valendo o do projeto)
1. Ler o `project/` (INDEX → o arquivo relevante) e a memória.
2. Plan mode quando for item novo ou mexer em schema; fatia pequena e fechada pode pular, dito explicitamente.
3. Código e testes (unitário, de fluxo contra o schema real, E2E), **editando com Edit/Write**.
4. Verificação: **o CI é a verificação** nos dois projetos. No Warden, nada de build ou teste local (compila, clippy e testes rodam no GitHub Actions). No Anchor, o
   `./dev.sh check` (tsc + cargo check) enquanto desenvolve e o CI para os testes de verdade, como diz o `INDEX.md`; rodar `cargo test` no Docker da máquina foi
   feito em sessões anteriores, mas não é necessário.
5. `git push` e **ler o CI**; se falhar, ler o log (`gh run view <id> --log-failed`), corrigir e publicar de novo.
6. Se a fatia tem tela: **baixar e olhar as capturas** (seção 2.2).
7. Registrar: `SESSIONS.md` (o que foi feito, o que foi verificado e como, o que não foi), `PHASE.md`/`PENDING.md` conforme o projeto, e o resultado do CI.

## 5. O que continua manual por natureza (dito uma vez, sem virar tarefa)
- Dado que só o dono tem: o saldo do banco para a conciliação, uma senha, uma decisão de gosto.
- Rodar sobre os dados reais do dono (extratos e planilhas reais ficam só na máquina dele, fora do repositório): o Claude entrega o fluxo testado com dados
  inventados e **diz o limite**, sem pedir que o dono teste.

## 6. Onde isto está registrado
- **Anchor**: este arquivo (`project/AUTOMACAO.md`), a diretriz em `project/GUIDELINES.md`, o `project/INDEX.md`, a Sessão 156 e a 157 do `SESSIONS.md`, a entrega 11
  da Fase 35 no `PHASE.md`, e a memória `feedback_anchor_automate_not_manual`.
- **Warden** (Sessão 157 do Anchor mandou o recado à sessão do Warden): o mesmo processo, adaptado às regras dele, registrado no `project/` dele pelo protocolo dele.
