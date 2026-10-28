> **Fluxo de trabalho (IA + dono)**: branch + Pull Request, plano antes de código, testes como contrato e
> checklist de fechamento de sessão estão no `CLAUDE.md` na raiz do repositório. Este arquivo cobre as
> diretrizes do código e do ensino neste projeto; se algo aqui conflitar com o `CLAUDE.md`, pare e pergunte ao dono.

---

# Diretriz de código (IMPORTANTE — sempre seguir)

**Todo código novo deve ser escrito em inglês — sem exceção.** O repositório será público no GitHub.
- Strings visíveis ao usuário (UI, mensagens de erro, labels, placeholders): inglês
- Nomes de variáveis, funções, classes, arquivos: inglês
- Comentários no código: podem ficar em português (não são visíveis ao usuário e facilitam o aprendizado)
- Esta regra vale para qualquer linguagem que venha a ser usada (Python, TypeScript, Rust, etc. — a decidir)

---

# Diretriz de segurança (IMPORTANTE — repo público no GitHub)

O projeto vai para um repositório **público**. Cuidados a partir do primeiro commit:
- **Nunca commitar chaves de API, tokens ou credenciais.** Tudo isso vai em `.env` (ou keyring do SO), com `.env.example` no repo mostrando as variáveis esperadas sem valores reais.
- `.gitignore` deve cobrir `.env`, banco de dados local (`*.db`, `*.sqlite`), pastas de build/dist, e qualquer arquivo de credencial.
- **Nunca hardcodar dados pessoais de portfólio** (valores investidos, quantidades, saldo) em código de exemplo, testes ou fixtures — usar dados fictícios.
- Antes de cada `git push` para o remoto público, revisar `git status`/`git diff` procurando por segredos, mesmo em arquivos com nome inofensivo.
- Ao decidir o banco de dados local (Fase 1), garantir que o arquivo do banco fique fora do controle de versão por padrão.

---

# Diretriz de ensino (IMPORTANTE — ler antes de cada sessão)

O usuário não é iniciante em programação (ver Fase 2 abaixo — já desenhou sozinho uma arquitetura de coleta de dados via APIs, com bom domínio de Python). O ponto de partida é **iniciante em construir uma aplicação desktop completa**: escolha de framework de UI, empacotamento/distribuição, banco de dados local, e organização de um projeto maior que um script. **Tem pouca intimidade com Rust especificamente** (pedido explícito do usuário, Sessão 1) — Python é a linguagem de referência pra analogias.

**Regras para o Claude:**
- Ir com calma — construir aos poucos, sessão a sessão, sem pressa pra "terminar tudo de uma vez"
- Antes de decidir framework/lib/arquitetura, explicar as opções e trade-offs e esperar a decisão do usuário — nunca decidir sozinho por ele quando a decisão for de gosto/direção do projeto
- Explicar o conceito novo (de UI, empacotamento, banco de dados, etc.) antes de escrever o código que o usa
- Não assumir conhecimento prévio de frameworks de UI desktop, ORMs/SQL local, ou empacotamento de apps — mas pode assumir Python e lógica de programação em geral
- **No código Rust especificamente**: ir com calma e explicar de verdade — o quê cada trecho faz, por que a sintaxe é do jeito que é, e comparar com o equivalente em Python quando ajudar (ex: `struct` ~ `dataclass`, `Result<T, E>` ~ exceção/erro explícito em vez de try/except, ownership/borrow como conceito novo sem equivalente direto em Python)
- Perguntar se o usuário entendeu antes de avançar quando o conceito for novo
- Nunca escrever um bloco grande de código sem explicar o que faz e por quê
- **(Sessão 20, pedido explícito, vale pra tudo daqui pra frente)**: sempre que for explicar como algo funciona (arquitetura, fluxo de dado, "como a tela pega o que tá no banco", etc.), explicar em linguagem simples e prática, sem jargão técnico solto, ancorado no código real (arquivo/trecho que o usuário pode abrir e seguir) — de preferência com um passo a passo concreto usando uma feature que o usuário acabou de usar, em vez de descrição abstrata

---

# Diretriz de verificação e automação (IMPORTANTE — Sessão 156, pedido explícito do dono)

O dono do projeto **não quer conferir tela nem disparar passo à mão** ("prefiro seguir por código, não gosto de fazer essas coisas manualmente"). Processo completo, com
o porquê de cada regra, em **`AUTOMACAO.md`**. O resumo:
- **Telas**: o E2E fotografa as telas-chave (`snap()`), o CI sobe as capturas sempre como artefato, e o Claude baixa (`gh run download <id> -n e2e-artifacts`) e **olha cada
  PNG**, corrigindo o que achar. Nunca fechar uma fatia com "abra o app e confira a tela".
- **Fluxos**: o que dá para rodar sozinho roda sozinho (gatilho pelo estado dos dados, melhor esforço, idempotente, resultado como proposta na fila de revisão, botão
  manual de reserva, marca com três estados). Só fica manual o que depende de dado que só o dono tem, dito uma vez como limite.
- As capturas usam **só dados inventados** (repo público).

---

# Ritmo e Expectativa do Projeto

- **Prioridade do usuário hoje é o TruthID.** O Practice Valuation existe pra resolver uma dor real (organizar preços-teto e acompanhar cripto), mas anda em ritmo de fundo — sessões espaçadas, sem pressa de terminar rápido.
- **Filosofia de construção**: expandir aos poucos, sessão a sessão. Por enquanto o objetivo é **só funcionar** pro uso pessoal do usuário — nada de over-engineering pensando em recursos futuros antes da hora.
- **Visão de longo prazo**: virar uma ferramenta "foda" de verdade (mais fontes de dado, mais metodologias, alertas afinados, talvez sync/mobile — ver "Roadmap de Evoluções Planejadas"), mas isso é destino, não requisito do MVP.
- Ao retomar uma sessão depois de um tempo parado, é normal — não tratar como projeto abandonado, só como projeto em ritmo lento.

