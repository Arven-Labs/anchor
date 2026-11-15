# 0003: Câmbio digitado pelo usuário no IR de exterior, por transação (mobile primeiro)

**Data:** 2026-11-15

## Contexto
O IR de exterior (Lei 14.754/2023) converte cada compra, venda e provento para reais pelo câmbio do dia. A lei manda a PTAX do Banco Central; o app usa a PTAX, com o câmbio de mercado (Yahoo) de reserva, e o ano avisa quando é aproximado (`_rateToBrl`, `mobile/lib/domain/foreign_tax.dart`). O usuário não tinha como corrigir uma taxa: PTAX faltando, taxa aproximada ou o câmbio real da corretora. Pedido do dono na Sessão 351.

Fatos lidos em 2026-11-15:
- Não existia override de câmbio no mobile nem no desktop. O desktop decide a taxa em `ForeignFx::rate` e `convert` (`commands/tax.rs`, `domain/tax/foreign.rs`).
- O mobile e o desktop são espelhos nos CSVs (16 colunas de transações, 13 do informe do exterior). O CSV de transações do mobile já não carrega `foreign_tax_withheld` nem `foreign_tax_paid_on`.
- O mobile usa sqflite com migrações em cadeia por `oldVersion`; não há downgrade.

## Decisão
- **Coluna na transação:** `fx_rate_override REAL` em `portfolio_transactions` (schema v27), em reais por 1 unidade da moeda do ativo. Nula = câmbio automático. A taxa vale **só para aquela transação**.
- **Uma taxa por transação**, usada também na conversão do imposto retido dela. A lei usa a cotação de compra para o retido e a de venda para o fato; o usuário digita uma só, e o aviso do ano pede para conferir.
- **Precedência:** taxa digitada, depois PTAX, depois mercado. Em ativo BRL vale 1 e a taxa digitada é ignorada.
- **Fonte própria:** `RateSource.manual`. O ano ganha o aviso `fxManual`, cada item mostra "(digitado)", e o CSV do informe marca `fx_approximate=true`. O app **não confere** a taxa digitada.
- **Validação:** o repositório recusa (`FormatException`) taxa que não seja finita e positiva; o formulário bloqueia o salvar e mostra o erro; o domínio ignora taxa inválida e cai na PTAX.
- **Só o mobile agora.** Desktop e CSVs ficam no P17.

## Alternativas
- **Tabela de câmbio manual por dia e moeda:** uma correção valeria para todas as operações do dia. Descartada pelo dono: tabela nova, tela própria de edição e, no desktop, a `ptax_daily` poderia sobrescrever o valor no refresh. A coluna é migração trivial.
- **Tratar a taxa digitada como exata (sem aviso):** o app não a valida, então marcar como exata esconderia o risco no informe. Descartada.
- **Duas taxas por transação (fato e imposto retido):** mais fiel à lei, mais campos para o usuário errar. Descartada por ora; reabrir se aparecer caso real.
- **Desktop junto:** mantém paridade, mas são vários PRs em código de imposto nos dois lados e estoura o limite de etapas. Descartada nesta rodada.

## Consequências
- Uma taxa errada digitada muda o imposto estimado. A defesa é o aviso do ano, o rótulo em cada item e o CSV marcado como aproximado, não validação.
- Corrigir um dia com várias operações exige editar cada transação.
- **A taxa digitada se perde ao exportar e importar o CSV de transações** (como o imposto retido já se perde), e o CSV do informe só carrega o booleano `fx_approximate`. Mudar as colunas de um lado sem o outro quebra a troca de arquivos com o desktop; por isso é o P17.
- O mobile e o desktop passam a divergir: o mesmo conjunto de transações pode dar informes diferentes até o P17.
- Migrações de teste com fixtures mínimas quebram quando uma migração nova altera uma tabela que a fixture não criou (aconteceu em 8 fixtures da v15 a v25).
- **Custo de reverter:** médio. Tirar o campo e a precedência é simples, mas o sqflite não volta de versão: a coluna fica no banco (inofensiva se ninguém a ler).
- **Não verificado:** o formulário num aparelho real; a exibição no histórico (sem teste); se a taxa digitada deve ter limite máximo (hoje só finita e positiva).
