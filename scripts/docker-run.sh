#!/bin/bash
# run the container with environment secrets.
set -ea pipeline

# Optional name suffix; default to "0"
NAME_SUFFIX="${1:-0}"
NAME="rlvgl-builder_${NAME_SUFFIX}"

docker run -d \
  -e SCCACHE_BUCKET="${SCCACHE_BUCKET:?Need SCCACHE_BUCKET set}" \
  -e AWS_REGION="${AWS_REGION:?Need AWS_REGION set}" \
  -e SCCACHE_REGION="${AWS_REGION:?Need AWS_REGION set}" \
  -e RUSTFLAGS="${RUSTFLAGS:-}" \
  -v codex_state:/root/.codex \
  -p 127.0.0.1:1455:1455 \
  --name "${NAME}" \
  iraa/rlvgl:latest tail -f /dev/null || echo "Container Failed to Spawn - Name taken ?"

# Get a copy of .ssh files from outside repo and populate them on the container once running.
# Create these and put them in ~/ssh from the host.
docker cp ~/ssh/known_hosts      "${NAME}":home/ubuntu/.ssh/known_hosts
docker cp ~/ssh/id_rsa           "${NAME}":home/ubuntu/.ssh/id_rsa
docker cp ~/ssh/id_rsa.pub       "${NAME}":home/ubuntu/.ssh/id_rsa.pub
docker cp ~/ssh/config_container "${NAME}":home/ubuntu/.ssh/config
docker cp ~/ssh/known_hosts      "${NAME}":home/rlvgl/.ssh/known_hosts
docker cp ~/ssh/id_rsa           "${NAME}":home/rlvgl/.ssh/id_rsa
docker cp ~/ssh/id_rsa.pub       "${NAME}":home/rlvgl/.ssh/id_rsa.pub
docker cp ~/ssh/config_container "${NAME}":home/rlvgl/.ssh/config
docker exec "${NAME}" mkdir -p /home/rlvgl/.codex
docker cp ~/.codex/config.toml   "${NAME}":home/rlvgl/.codex/config.toml

# Make sure rlvgl user has access to its credentials.
docker exec -u 0 "${NAME}" chown -R "rlvgl":"rlvgl" \
  /home/rlvgl/.ssh/known_hosts \
  /home/rlvgl/.ssh/id_rsa \
  /home/rlvgl/.ssh/id_rsa.pub \
  /home/rlvgl/.ssh/config \
  /home/rlvgl/.codex/config.toml

# Execute commands to checkout repo, submodules, add ssh origin, assign it and execute bash
docker exec "${NAME}" git clone https://github.com/SoftOboros/rlvgl.git /opt/rlvgl \
  || echo "Already Cloned"
docker exec "${NAME}" git -C /opt/rlvgl submodule update --init --depth=3 || echo "Submodule Dirty"
docker exec "${NAME}" git -C /opt/rlvgl remote add origin-ssh git@github.com:SoftOboros/rlvgl \
  || echo "Remote Exists"
docker exec "${NAME}" git -C /opt/rlvgl pull --set-upstream origin-ssh main \
  || echo "Repo Dirty"
