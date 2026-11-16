# 0004: Editar transação no desktop: recusa só o que piora a cobertura, swap não edita

**Data:** 2026-11-15

## Contexto
No desktop só existiam `create_transaction` e `delete_transaction`: corrigir um lançamento era apagar e relançar. O mobile já edita (`PortfolioRepository.updateTransaction`), mas só confere o formato e a amortização, sem olhar o histórico. Editar tem um risco que criar não tem: mudar uma compra antiga (quantidade, data, ativo) pode deixar descoberta uma venda posterior.

Fatos lidos em 2026-11-15:
- Nada derivado é gravado: posição, custo médio, IR e informes são recalculados a partir de `transactions`; nenhuma outra tabela referencia `transactions.id`.
- O lançamento já recusa venda sem cobertura por replay do histórico do ativo (`ensure_covered_sale`), mas só se a venda **piora** o histórico (`após && !antes`). `delete_transaction` não confere nada.
- As duas pernas de um swap cripto (venda e compra) são ligadas só pelo texto de `notes` ("Swap A → B"); não há coluna de vínculo.
- A chave de deduplicação do import (`ticker, data, tipo, quantidade, total`) é calculada a cada import sobre `transactions`.

## Decisão
- **`update_transaction`** recebe o `id` e o estado completo do lançamento (os mesmos campos de `CreateTransactionRequest`), valida com as mesmas regras do lançamento (`validate` e `ensure_asset_rules`) e grava na mesma linha. `portfolio_id` e `created_at` não mudam; mudar de carteira é recusado.
- **Integridade: recusa só o que piora.** O backend refaz o replay do ativo com a linha de hoje ("antes") e com a linha editada no lugar da antiga ("depois"), no ativo novo e no antigo se o ativo mudou, e recusa só se o "depois" tem venda descoberta e o "antes" não. A linha antiga sai do "depois" (senão contaria em dobro).
- **Perna de swap não edita**: compra ou venda com `notes` começando em "Swap " é recusada ("apague o swap e refaça"). O Ledger não mostra o botão Edit nelas.
- **Tela:** o botão Edit do Ledger abre a aba Transactions com o formulário em modo edição (`key` por linha, estados iniciais da transação); Save chama `update_transaction`, Cancel volta ao Ledger. O erro do backend aparece em `tx-error`.
- Criar, editar e apagar invalidam também `tax-assessment` e `funds-overview` no front.

## Alternativas
- **Igual ao mobile (só formato e amortização):** simples, mas permite saldo negativo ao editar uma compra. Descartada.
- **Recusar sempre que o resultado fosse ruim:** bloquearia uma edição inocente por causa de um histórico que já estava descoberto (apagar uma compra não confere nada). Descartada em favor do "antes e depois".
- **Editar as pernas do swap isoladamente:** deixaria valores e quantidades das duas pernas incoerentes. Descartada, assim como criar uma coluna de vínculo (migration por um caso de borda).
- **Dialog próprio de edição:** duplicaria a lógica de campos por tipo do formulário. Descartada pelo dono.
- **Patch parcial (só os campos mudados):** o formulário já guarda o estado completo; o estado completo cabe no mesmo `validate`.

## Consequências
- Editar uma compra, data, quantidade ou ativo muda posição, custo médio e IR. O replay protege a **cobertura de vendas**; não julga se o valor faz sentido econômico.
- **Atualização (2026-11-15, Sessão 358):** `delete_transaction` passou a usar a mesma guarda (`ensure_history_not_worse`, compartilhada com a edição): apagar uma linha que deixaria venda sem cobertura é recusado ("só o que piora"). Para apagar uma compra de que uma venda depende, apague a venda antes. Isso fecha a assimetria que existia entre editar e apagar. Um histórico que já estava descoberto continua sem bloquear o delete de outra linha.
- **Amortização posterior (atualização de 2026-11-15, Sessão 358):** a mesma guarda agora também refaz o encaixe de cada amortização do ativo (`unfunded_amortizations`, que reusa `amortization_problem`, a conta de `ensure_amortization_fits`): editar ou apagar uma compra que deixaria uma amortização posterior sem cotas, ou acima do valor da posição, é recusado. Uma amortização que já não cabia antes não bloqueia.
- **Come-cotas posterior (corrigido em 2026-11-15, Sessão 360):** é uma venda para o motor de custo, então a cobertura de venda já o protege. Não há nada derivado gravado para revalidar: `estimate_fund` (`domain/tax/fund_redemption.rs`) recalcula o imposto pago e o ganho já tributado a cada leitura, a partir de `quantity`, `unit_price` e dos lotes de compra. O `loss_credit_used` é um dado de entrada (o que a administradora informou) e é limitado ao ganho recalculado (`min(taxed_gain.max(0))`), então uma edição que reduza o ganho o corta sozinha. A nota da administradora não muda com a edição, e isso é o esperado. Não li `fund_compensation` (compensação de crédito entre fundos).
- **Linha importada:** editar muda a chave de deduplicação; reimportar o arquivo original pode recriá-la.
- O desktop e o mobile passam a ter a mesma capacidade, mas regras diferentes: o desktop confere o histórico, o mobile não.
- **Custo de reverter:** baixo. Não há migration; remover o comando, o botão e o modo edição basta, e nada fica gravado de diferente.
- **Não verificado:** a edição num app real (só o E2E a exerceu); a invalidação de `tax-assessment` não tem teste que a prove (o `QueryClient` usa o `staleTime` padrão de 0, então a aba Taxes remonta e busca de novo mesmo sem invalidar; a invalidação só importa para uma tela de impostos já montada).
