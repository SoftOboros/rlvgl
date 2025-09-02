#!/bin/bash
# run the contaner with environment secrets.
set -ea pipeline
docker exec \
  -it \
  -e AWS_ACCESS_KEY_ID="${CODEX_ACCESS_KEY_ID:?Need AWS_ACCESS_KEY_ID set}" \
  -e AWS_SECRET_ACCESS_KEY="${CODEX_SECRET_ACCESS_KEY:?Need AWS_SECRET_ACCESS_KEY set}" \
  rlvgl-builder bash
