FROM node:20-bookworm-slim

# Bibliotecas do Linux que o Tauri precisa:
# - libwebkit2gtk: motor HTML que renderiza o React dentro da janela
# - libgtk-3-dev: toolkit de janelas do Linux
# - libssl-dev: criptografia (HTTPS)
# - build-essential: compilador C (dependências do Rust precisam)
# - libudev-dev e pkg-config: o `hidapi` (visualizador de wallets, Fase 36) acha a
#   libudev por pkg-config; sem eles o build do app para no build script
# - as demais: ícones, indicadores de sistema, imagens SVG
RUN apt-get update && apt-get install -y \
    build-essential \
    curl \
    file \
    libssl-dev \
    libgtk-3-dev \
    libwebkit2gtk-4.1-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    libudev-dev \
    pkg-config \
    patchelf \
    && rm -rf /var/lib/apt/lists/*

# Instala Rust dentro do container
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
ENV PATH="/root/.cargo/bin:${PATH}"
ENV RUSTUP_HOME=/root/.rustup CARGO_HOME=/root/.cargo HOME=/root

# sea-orm-cli: ferramenta de dev pra gerar migrations/entities do banco (SeaORM).
# Não é dependência do projeto (isso já está no Cargo.toml) — fica só no ambiente,
# igual o próprio cargo/rustc. Versão fixada pra bater com `sea-orm` no
# Cargo.toml do app — sem isso, um rebuild da imagem puxa a versão mais nova do
# CLI e `generate entity` passa a gerar colunas incompatíveis (ex.: `id: i64`
# em vez de `i32`), quebrando toda entity existente silenciosamente.
RUN cargo install sea-orm-cli --version 1.1.20 --locked

# node:20 já cria um usuário "node" (uid/gid 1000, mesmo do host) — é ele que
# roda o app em runtime (docker-compose.yml: user: "1000:1000"), necessário pra
# autenticação D-Bus da notificação nativa (Fase 5.3), que valida a credencial
# real do processo (SO_PEERCRED), diferente do X11 que autentica só por cookie.
# Libera leitura/execução do toolchain instalado como root pra esse usuário.
RUN chmod 755 /root && chmod -R a+rX /root/.cargo /root/.rustup

WORKDIR /app
