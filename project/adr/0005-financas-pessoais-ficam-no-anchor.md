# 0005: Finanças pessoais ficam no anchor; Firefly III não entra por baixo

**Data:** 2026-11-17

## Contexto
O relatório de reuso do workspace (2026-11-13) perguntou se as Finanças pessoais deveriam ir para uma ferramenta pronta (Firefly III), estimando "~17 mil linhas". Essa conta estava errada: incluía `finance_core` e `finance_api`, que são a camada de dados de mercado (Yahoo, CVM, BCB, CoinGecko), não Finanças pessoais.

Fatos lidos em 2026-11-17 (mapeamento só de leitura):
- As Finanças pessoais são ~4,8 mil linhas de Rust (`entity/`, `commands/`, `domain/`) e ~3,9 mil de TSX (`desktop/src/finances/`) no desktop, mais ~2,6 mil de telas e 686 de domínio no mobile (`mobile/lib/screens/financas_*`, `mobile/lib/domain/`).
- Nove entidades: `bank_account`, `general_transaction`, `general_transaction_category`, `liability`, `liability_installment`, `borrower`, `loan`, `loan_repayment`, `import_pending`.
- Não há trait de repositório: os comandos usam SeaORM direto (variantes `*_in<C: ConnectionTrait>`). No mobile, `GeneralFinanceRepository` é uma classe concreta sobre sqflite.
- Não há sync de Finanças entre desktop e celular: cada um tem seu banco.
- Acoplamento com a carteira (Fase 12 do `PHASE.md`, entidade paralela): `get_net_worth_summary`, `liability.linked_asset_id`, `bank_account.mirrored_in_portfolio` e `mirrored_cash_deduction`.

## Decisão
As Finanças pessoais ficam no anchor, com a interface e o funcionamento atuais. Não se coloca o Firefly III como backend, nem no desktop nem no mobile.

## Alternativas
- **Firefly III como backend total:** cobriria só contas, lançamentos, categorias e transferências (o CRUD, a parte barata). Dívidas Price/SAC ligadas a ativo, empréstimos concedidos, rendimento do porquinho, patrimônio líquido sem dupla contagem do caixa espelhado e importação de extrato do Inter (OFX/CSV com fila de revisão) continuariam no anchor. Exigiria servidor PHP com banco e uma porta de repositório para as 9 entidades. No mobile, que hoje é local, o celular passaria a depender de um servidor acessível. Descartada: custo alto para pouca manutenção poupada.
- **Piloto híbrido no desktop** (Firefly guarda contas, lançamentos e categorias; o anchor guarda o resto): mesmos custos, mais a fronteira entre os dois que o patrimônio líquido atravessaria. Descartada.
- **Export ou integração opcional com o Firefly:** não avaliada a fundo. Se um dia houver demanda, entra como módulo opcional (`padrao/modularidade.md`), sem mexer no núcleo.

## Consequências
- Nenhum código muda. A manutenção das Finanças continua nossa.
- O relatório `docs/pesquisa/reuso/anchor.md` do workspace deve ser corrigido (tamanho real e veredito).
- **Custo de reverter:** baixo, a decisão só registra um "não". Reabrir se a manutenção das Finanças passar a pesar mais que o resto do app.
- **Não verificado:** licença do Firefly III (acredito que seja AGPL, não confirmei); se ele modela dívida com cronograma de amortização; se importa OFX do Inter. A decisão não depende deles, pois o servidor e o mobile local já a sustentam, mas precisam ser conferidos antes de reabrir.
