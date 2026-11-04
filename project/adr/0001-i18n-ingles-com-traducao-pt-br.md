# 0001: Mobile em inglês, com tradução pt-BR por i18n

**Data:** 2026-11-01

## Contexto
O `GUIDELINES.md` manda a UI em inglês (o repositório é público) e o desktop já foi traduzido na Fase 6.1. O mobile nasceu em português e nunca foi alinhado: 52 arquivos de `mobile/lib` e 67 de `mobile/test` têm texto com acento, 18 das 20 telas têm texto em português, e `formatMoney` é fixo em pt-BR (`R$ 1.234,56`, sem `intl`). A divergência vinha sendo registrada como pendência desde a Sessão 271. Na Sessão 277 o dono escolheu resolvê-la de vez: **foco 100% em inglês, com tradução para português**.

## Decisão
- Inglês é o idioma-fonte e pt-BR é uma tradução, via `flutter_localizations` + `intl` e arquivos `.arb` (`mobile/lib/l10n/app_en.arb` é o modelo, `app_pt.arb` a tradução). Nenhuma string de UI fica fixa no código.
- A camada de domínio **não depende do Flutter**: funções puras devolvem códigos tipados (enum ou valor), e a tela os traduz com `AppLocalizations`.
- O idioma é o do aparelho; qualquer idioma que não seja português cai em inglês. Números e datas seguem o locale (`1,234.56` em inglês, `1.234,56` em pt-BR). **Emenda de 2026-11-04 (Sessão 290):** o app ganhou um seletor de idioma (Configurações, pela engrenagem da Início) com três opções: seguir o aparelho (padrão, o comportamento acima), English e Português. A escolha fica em `app_settings` (chave `language`).
- Siglas e nomes próprios brasileiros (DARF, DIRPF, CVM, B3) não são traduzidos.
- Migração em série de PRs: infraestrutura + ADR + aba "Cripto" como piloto; depois um PR por área (Início e Carteira, Finanças, Impostos, Custódias e importações); por fim, limpeza e um teste que falha com string de UI fixa. Cada PR entrega inglês e pt-BR e testa os dois.

## Alternativas descartadas
- **Só um ADR aceitando o mobile em português:** é barato, mas deixa a UI de um repositório público em português e contradiz o `GUIDELINES.md`. O dono pediu inglês como foco.
- **Traduzir para inglês sem infraestrutura:** perde o português, que é o idioma do uso diário e do domínio (imposto de renda, CVM).
- **Passar `AppLocalizations` para o domínio:** muda menos assinaturas, mas acopla o domínio ao Flutter e deixa os testes de domínio dependentes de idioma.
- **Seletor de idioma dentro do app:** descartado na decisão original (mais código e mais um fluxo para testar, sem necessidade hoje). **Revisto em 2026-11-04**: o dono pediu o seletor. Custo real: uma linha em `app_settings`, um `InheritedWidget` e uma tela, sem dependência nova nem mudança de schema.

## Consequências
- Dependência nova: `intl` (e `flutter_localizations`, do SDK), aprovada pelo dono. O `flutter gen-l10n` roda no build e pode pedir ajuste no `ci-mobile.yml`.
- Durante a série, as áreas ainda não migradas continuam em português mesmo num aparelho em inglês (estado misto, aceito para uso pessoal).
- Mudar `formatMoney` e `formatBoundary` para seguir o locale pode alterar números em telas de imposto se um chamador for esquecido; por isso os chamadores das áreas não migradas passam `pt` explicitamente até a sua vez.
- Domínio devolvendo códigos muda assinaturas públicas e os testes de domínio, que passam a afirmar códigos em vez de frases.
- Frases com números usam placeholders e plurais do `.arb`, nunca concatenação.
