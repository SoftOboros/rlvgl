#!/bin/bash
# exec into a running container with environment secrets.
set -ea pipeline

# Optional name suffix; default to "0"
NAME_SUFFIX="${1:-0}"
NAME="rlvgl-builder_${NAME_SUFFIX}"

docker exec \
  -it \
  -e AWS_ACCESS_KEY_ID="${CODEX_ACCESS_KEY_ID:?Need AWS_ACCESS_KEY_ID set}" \
  -e AWS_SECRET_ACCESS_KEY="${CODEX_SECRET_ACCESS_KEY:?Need AWS_SECRET_ACCESS_KEY set}" \
  "${NAME}" bash
