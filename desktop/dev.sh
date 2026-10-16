#!/bin/bash
# ./dev.sh        → sobe o app (tauri dev) no Docker
# ./dev.sh check  → só confere se compila (tsc + cargo check --all-targets, que inclui
#                   o código de teste), sem abrir o app.
#                   Roda no mesmo container/volume do app pra reaproveitar o cache
#                   de compilação em vez de criar um segundo target/ no host.
#                   Testes de verdade (cargo test etc.) rodam no GitHub Actions.
set -e

# Volume recém-criado nasce com dono root, mas o container roda como uid 1000
# (ver docker-compose.yml). Acerta o dono antes de compilar — barato e idempotente.
docker compose run --rm --no-deps --user root desktop \
  chown 1000:1000 /app/src-tauri/target >/dev/null

case "${1:-up}" in
  check)
    docker compose run --rm --no-deps desktop sh -c \
      "npm install --no-audit --no-fund --silent && npx tsc --noEmit && cd src-tauri && cargo check --all-targets"
    ;;
  up)
    xhost +local:docker
    docker compose up
    ;;
  *)
    echo "uso: ./dev.sh [up|check]" >&2
    exit 1
    ;;
esac
