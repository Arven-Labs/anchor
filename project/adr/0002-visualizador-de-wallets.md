# 0002: Visualizador de wallets como módulo opcional e isolado no Anchor, só leitura

**Data:** 2026-11-13

## Contexto
O dono quer ver o conteúdo de wallets de hardware (Ledger, Trezor) e hot wallets, com atualização, sem que isso toque na carteira de investimentos (posições, transações, resultado, impostos). Na Sessão 340 a pergunta era onde isso vive: no Anchor, no TruthID ou num projeto próprio. O dono escolheu conectar o dispositivo de verdade (não só cadastrar endereço) e começar por Ethereum e Base, depois Bitcoin.

Fatos lidos em 2026-11-13:
- O TruthID é autenticação por wallet. Seus `ledger.rs` (403 linhas) e `trezor.rs` (302) falam com o dispositivo via `hidapi` e `trezor_client`, mas para assinar login, e só em Ethereum (`m/44'/60'/conta'/0/0`). O `ledger.rs` também contém código de assinatura de transação.
- O rotki (AGPLv3, local-first) rastreia endereços e chaves públicas estendidas só para leitura; a documentação encontrada não confirma conexão com Ledger nem Trezor.
- O `padrao/modularidade.md` manda: módulo depende do núcleo, núcleo nunca depende de módulo, módulo não depende de módulo, e o degrau mais barato (ligar e desligar por configuração) é o preferido.

## Decisão
- O visualizador é um **módulo opcional dentro do Anchor**, ligável e desligável por configuração, com o código no binário (degrau "a" da escada de modularidade).
- **Isolamento de dados:** tabelas e migrações próprias; nenhuma chave estrangeira, consulta ou escrita entre o módulo e as tabelas de investimento. Sem o módulo, o núcleo funciona e a interface mostra "não instalado".
- **Só leitura, sem assinar:** o módulo pede ao dispositivo apenas endereço ou chave pública. Ele **não contém código de assinar** mensagem ou transação, nem de pedir ou guardar chave privada. Essa é uma garantia de construção, não uma promessa de uso.
- **Código do dispositivo escrito de novo, menor.** O Anchor não importa nem copia o `ledger.rs` e o `trezor.rs` do TruthID (que carregam assinatura e são de outro projeto); usa-os como referência. As bibliotecas `hidapi` e `trezor-client` são dependências novas e passam, cada uma, pela avaliação de `padrao/dependencias-novas.md` e pela aprovação do dono antes de entrar.
- **Ordem:** Ethereum e Base primeiro (mesmo endereço nas duas redes, leitura por RPC público); Bitcoin depois (chave pública estendida, indexador e outro app na Ledger). Outras redes ficam para depois.
- Primeira entrega: tracer bullet com **um endereço Ethereum, só leitura, sem hardware**, antes de conectar o dispositivo.

## Alternativas
- **Usar pronto (rotki, Ledger Live, Trezor Suite, Rabby):** custo zero e nada a manter. Descartada: o rotki não confirma conexão com dispositivo, e ficaria fora do ecossistema e do modelo de dados do Anchor.
- **No TruthID:** ele já fala com a Ledger. Descartada: o propósito dele é identidade e chaves; consulta de saldo e provedores de RPC diluem o escopo e ampliam a superfície de segurança de quem guarda a identidade. Lá o hardware serve para assinar; aqui, para ler.
- **Projeto próprio:** isolamento total. Descartada: repo, CI e release novos sem uma dor que os justifique; o isolamento necessário se obtém dentro do Anchor pela regra de dados.
- **Só cadastrar endereço público, sem conectar o dispositivo:** o menor risco de segurança. Descartada pelo dono, que quer conectar o dispositivo. O módulo continua só leitura.

## Consequências
- Mais superfície no binário do Anchor (leitura on-chain, USB), e dois novos pacotes a aprovar. Isso toca chaves (AGENTS.md §4): qualquer mudança que adicione assinatura ou chave privada **contradiz este ADR** e pede confirmação explícita e novo ADR.
- O isolamento só vale se for testado: um teste deve falhar se o módulo referenciar tabelas de investimento.
- Bitcoin é mais trabalho de dispositivo que Ethereum, e o código do TruthID não ajuda nele.
- **Custo de reverter:** fácil a médio. Se o isolamento for real, remover o módulo é apagar código, tabelas próprias e a opção de configuração.
- **Não verificado:** o comportamento da Ledger e do Trezor reais do dono; o `trezor-client` sem o Trezor Bridge; a saúde, a licença e o peso de `hidapi` e `trezor-client`.

## Adendo (2026-11-13, Fase 36.3): o Trezor e uma dependência que sabe assinar
**Contexto.** A garantia "o módulo não contém código de assinar" foi escrita pensando no nosso código. A Ledger (Fase 36.2) cumpre isso por inteiro: o transporte é nosso e só monta `GET_ADDRESS`. Para o Trezor, a escolha prática é o `trezor-client` (CC0, mantido pela própria Trezor), e a crate **contém** funções de assinar (`ethereum_sign_tx`, `ethereum_sign_message` e outras). Escrever o protocolo protobuf do Trezor nós mesmos seria código demais para manter.

**Decisão** (dono, 2026-11-13). Aceitar o `trezor-client` 0.1.6 com a garantia reescrita: **o código do Anchor não chama nenhuma função de assinar**; a dependência pode tê-las. Ela entra com:
- só a feature `ethereum`, sem `bitcoin`, `solana` e o resto;
- busca do aparelho só por USB (`WebUsbTransport`), sem o ping UDP ao emulador que o `find_devices` da crate faz;
- teste-guarda de texto com os nomes de assinar nas duas grafias (`ethereum_sign`, `sign_tx`, `ethereumsign`, `signtx`...);
- **teste em execução**: um transporte falso registra cada mensagem que o Anchor manda ao aparelho, e o teste reprova qualquer mensagem fora de `Initialize`, `EthereumGetAddress`, `ButtonAck` e `PassphraseAck`;
- PIN e passphrase digitados no aparelho; a crate responde ao pedido de passphrase com "digite no aparelho", e o Anchor nunca os vê.

**Consequências.** A superfície que *poderia* assinar está no binário, inerte. A proteção contra alguém passar a usá-la são os dois testes, e quem os desligar contradiz este ADR. O transporte USB da crate lê sem limite de tempo, então a leitura tem um teto do lado async e uma trava que recusa leituras simultâneas. O Trezor One com PIN (matriz digitada no computador) não é suportado; a tela pede para desbloquear no Trezor Suite antes, o que não foi testado. **Reverter:** apagar o `trezor.rs` e a dependência; a Ledger não depende dela.
