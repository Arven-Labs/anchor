# Publicar uma release do desktop (runbook)

> Criado na Sessão 326 (P9, PR B). Cobre o `desktop/`; o APK do mobile segue o job `apk` do mesmo `build.yml` e não é tratado aqui.
> O que este runbook **não** prova está na última seção. Leia antes de confiar nele.

## Como o updater funciona (uma tela)
- O app (`desktop/src-tauri/tauri.conf.json`, `plugins.updater`) consulta `https://github.com/Arven-Labs/anchor/releases/latest/download/latest.json`.
- `latest/download` só resolve para release **publicada** (não rascunho, não pré-release). Rascunho dá 404 e o app mostra "No releases published yet".
- O `latest.json` lista, por plataforma, a URL do pacote e o conteúdo do `.sig`. O app baixa o pacote e confere o `.sig` contra a `pubkey` da config **antes** de instalar. Assinatura recusada = nada instala.
- Pacotes que o updater usa: Linux `.AppImage`, Windows `.msi`/`-setup.exe`, macOS `.app.tar.gz`. O `.deb` e o `.dmg` são só instaladores para humanos e **não** se atualizam sozinhos.

## Publicar uma versão
1. **Subir a versão** em `desktop/src-tauri/tauri.conf.json` (`version`) e, para não divergir, em `desktop/src-tauri/Cargo.toml` e `desktop/package.json`. PR normal, CI verde, merge.
2. **Ensaiar sem tag** (recomendado): `gh workflow run build.yml --ref main`, depois `gh run watch <id>`. Nada é publicado; os pacotes e os `.sig` sobem como artefato (`gh run download <id>`). Confira que cada SO tem o pacote e um `.sig` não vazio.
3. **Criar a tag igual à versão**: `git tag v<versão> && git push origin v<versão>`. O workflow recusa a tag se ela não bater com a `version` do `tauri.conf.json` (`v0.2.0` exige `"version": "0.2.0"`).
4. O workflow cria uma **release em rascunho** com os pacotes, os `.sig` e o `latest.json`. **Olhe** o rascunho: os 3 SO presentes, `latest.json` com `version` certa e as 3 plataformas.
5. **Publicar** (só você, é o ponto sem volta): `gh release edit v<versão> --draft=false`. A partir daí `latest/download/latest.json` aponta para ela e os apps instalados passam a oferecer a atualização.
6. **Registrar** em `SESSIONS.md`: a versão, o run do workflow e o que foi (ou não) verificado.

## Se a release publicada estiver errada
- Não apague a tag nem force push. Despublique voltando ao rascunho (`gh release edit v<versão> --draft=true`): o endpoint volta a dar 404 (ou aponta para a release publicada anterior) e ninguém mais recebe a oferta. Quem já atualizou não volta atrás; corrija e publique uma versão **maior**.

## Custódia da chave de assinatura
- A chave **privada** vive só nos secrets do repositório (`TAURI_SIGNING_PRIVATE_KEY` e `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`) e na cópia de segurança do dono, **fora** do repo e fora do workspace. Nunca em log, artefato, issue ou commit. O workflow só a expõe no `env` do passo `Build`.
- A chave **pública** está na `pubkey` do `tauri.conf.json` (repo público, tudo bem). ID da chave: `29CF5A240EEE7800`.
- **Perdeu a chave privada:** ninguém recebe mais atualização automática, porque os apps instalados só aceitam a chave antiga. Saída: gerar um par novo, publicar uma versão manualmente (instalador baixado) com a `pubkey` nova e orientar quem tem o app a reinstalar. Por isso a cópia de segurança importa.
- **Chave vazou (ou suspeita):** trate como incidente. Gere um par novo (`npx tauri signer generate`), troque os 2 secrets, troque a `pubkey` e publique uma versão nova **por reinstalação manual**; a chave antiga continua aceita pelos apps antigos, então até todos reinstalarem existe uma janela de risco. Despublique releases suspeitas.
- Trocar a senha ou a chave é decisão sua e entra no §4 do `AGENTS.md` (chaves e CI/deploy): peça confirmação antes de mexer nos secrets.

## O que este runbook NÃO prova
- A **instalação e o reinício reais** do app: o E2E (Sessão 322) prova baixar e verificar a assinatura contra um servidor local e o ensaio sem tag prova gerar os pacotes assinados, mas ninguém ainda instalou uma versão publicada por cima de uma antiga. Faça o primeiro ciclo com uma versão de teste e anote o resultado aqui.
- A **verificação criptográfica completa** dos `.sig` gerados no CI (conferi só que o ID da chave nos `.sig` bate com a `pubkey`).
- O macOS Intel (a matriz só tem `aarch64`) e assinatura de código do SO (Apple/Microsoft: o aviso "desenvolvedor desconhecido" continua).
- A atualização no Windows e no macOS: só a máquina do dono (Linux) é testada de ponta a ponta.

## O que a primeira release (`v0.1.0`, Sessão 338) provou
- O rascunho com tag existe com os pacotes, os `.sig`, o APK e o `latest.json` (`version` 0.1.0, 9 entradas de plataforma, todas com assinatura; o `tauri-action` gera e anexa o `latest.json`). Run 43343721100.
- O passo "Tag igual à versão do app" roda e passa num runner.
- Depois de publicar, `releases/latest/download/latest.json` responde 200 e os URLs do `.AppImage`, `.app.tar.gz` e `.msi` respondem 200.
- O campo `notes` do `latest.json` sai vazio: o app não mostra texto de novidades.
- Ainda **não provado**: a atualização de uma versão instalada para a seguinte (0.1.0 → 0.1.1). Anote o resultado aqui depois do primeiro ciclo.
