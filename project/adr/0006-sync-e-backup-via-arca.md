# 0006: Sync e backup passam a usar o arca; a Fase 8 (TruthID + Arweave) fica congelada

**Data:** 2026-11-17

## Contexto
O relatório de reuso do workspace (2026-11-13) perguntou se o sync/backup descentralizado do anchor deveria usar o arca em vez de código próprio. O dono decidiu em 2026-11-17: **sim, o anchor usará o arca**, sem prazo definido, porque a maturidade do arca ainda não é conhecida por ele.

Fatos lidos em 2026-11-17:
- **Anchor, Fase 8 (`PHASE.md`):** 8.1 (contrato `SyncRegistry` deployado em Base Sepolia), 8.2 (escrita do CID via `/truthid/v1/sign-request`, TruthID na mesma máquina), 8.6 (transporte LAN e dead-drop) estão feitas; 8.3 (pin) está parcial e **bloqueada** (o `/sign-request` do TruthID executa sempre em Base Mainnet, sem seleção de rede); 8.4 (cifra via `/sign-message`) e 8.5 (publicar e baixar com merge) **não foram feitas**. Hoje `pin_database_snapshot` e `push_pin_content` mandam os bytes do SQLite **sem cifra** para o `/pin`, e o backend de pin do TruthID é Arweave (permanente e público).
- **Arca (`arca/project/OVERVIEW.md`):** Fases 0 (descoberta) e 1 (núcleo local: cripto, manifest, restauração) feitas; 2 (sync entre dispositivos) e 3 (backup externo S3) em andamento; **4 (validação e revisão de segurança) e 5 (extração como biblioteca e segundo consumidor, o Anchor) não iniciadas**.
- O arca é produto próprio com formato próprio (ADR 0003 dele): cifra no cliente, backend que só vê bytes opacos, snapshot, restauração e frase de recuperação de 24 palavras.

## Decisão
- O anchor adotará o arca como camada de sync e backup **quando as fases 4 e 5 do arca estiverem concluídas**.
- **Até lá, a Fase 8 fica congelada:** nenhuma fatia nova (8.4, 8.5 e demais). O que já existe permanece como está, sem manutenção além de manter o build verde.
- **Não usar `pin_database_snapshot` nem `push_pin_content` com um banco real** enquanto não houver cifra: o destino é permanente e público.
- Quando o arca chegar na fase 5, abrir um ADR de adoção (backend, chave, o que entra no repositório) e decidir o destino do código da Fase 8 (remover ou manter como módulo opcional, `padrao/modularidade.md`).

## Alternativas
- **Terminar a Fase 8 (8.4 e 8.5) com TruthID + Arweave:** é código próprio de criptografia e de merge, sobre um backend permanente e irreversível, e a 8.3 depende de o TruthID resolver a rede. Descartada: duplica o que o arca faz e a cifra é área de risco (§4 do AGENTS.md).
- **Adotar o arca agora:** a fase 4 (revisão de segurança) ainda não aconteceu e o formato pode mudar até a fase 5. Descartada pelo momento, não pela direção.
- **Restic, Kopia ou Syncthing:** cobrem backup ou sync, mas não a combinação do arca (sync com cifra no cliente, backend não confiável, biblioteca Rust embutida). O relatório de reuso do arca já descartou a troca do formato.

## Consequências
- O anchor fica **sem sync entre dispositivos** por um tempo. Hoje o mobile e o desktop só trocam a carteira por CSV (ADR 0003); as Finanças não sincronizam (ADR 0005).
- O contrato em Base Sepolia (`0x9e2fd1a4146e3c40ecfef9d35a09e75e00ac30e8`) e o código de `sync_registry.rs` e `truthid.rs` ficam sem uso ativo. Remover ou manter só na adoção.
- **Custo de reverter:** baixo. Nada é apagado; basta retomar a Fase 8.
- **Não verificado:** o prazo do arca; se o `BlobStore` e o formato dele cobrem um SQLite inteiro do anchor (o anchor teria de decidir se sincroniza o arquivo ou as tabelas); o que fazer com a identidade TruthID depois, já que o arca usa frase de recuperação própria.
