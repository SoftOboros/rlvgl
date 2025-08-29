#!/bin/bash
# run the contaner with environment secrets.
docker run -d \
  -e SCCACHE_BUCKET="${SCCACHE_BUCKET:?Need SCCACHE_BUCKET set}" \
  -e AWS_REGION="${AWS_REGION:?Need AWS_REGION set}" \
  -e AWS_ACCESS_KEY_ID="${AWS_ACCESS_KEY_ID:?Need AWS_ACCESS_KEY_ID set}" \
  -e AWS_SECRET_ACCESS_KEY="${AWS_SECRET_ACCESS_KEY:?Need AWS_SECRET_ACCESS_KEY set}" \
  -e SCCACHE_REGION="${AWS_REGION:?Need AWS_REGION set}" \
  -v codex_state:/root/.codex \
  -p 127.0.0.1:1455:1455 \
  --name rlvgl-builder \
  iraa/rlvgl:latest tail -f /dev/null
# Get a copy of .ssh files from outside repo and populate them on the container once 
# running.  Create these and put them in ../ssh from the repo root
docker cp ~/ssh/known_hosts      rlvgl-builder:home/ubuntu/.ssh/known_hosts
docker cp ~/ssh/id_rsa           rlvgl-builder:home/ubuntu/.ssh/id_rsa
docker cp ~/ssh/id_rsa.pub       rlvgl-builder:home/ubuntu/.ssh/id_rsa.pub
docker cp ~/ssh/config_container rlvgl-builder:home/ubuntu/.ssh/config
docker cp ~/ssh/known_hosts      rlvgl-builder:home/rlvgl/.ssh/known_hosts
docker cp ~/ssh/id_rsa           rlvgl-builder:home/rlvgl/.ssh/id_rsa
docker cp ~/ssh/id_rsa.pub       rlvgl-builder:home/rlvgl/.ssh/id_rsa.pub
docker cp ~/ssh/config_container rlvgl-builder:home/rlvgl/.ssh/config
# Make sure rlvgl user has access to it's creentials.s
docker exec -u 0 rlvgl-builder chown -R "rlvgl":"rlvgl" \
                                /home/rlvgl/.ssh/known_hosts \
                                /home/rlvgl/.ssh/id_rsa \
                                /home/rlvgl/.ssh/id_rsa.pub \
                                /home/rlvgl/.ssh/config
# Execute commands to checkout repo, submodules, add ssh origin, assign it and execute bash
docker exec rlvgl-builder git clone https://github.com/SoftOboros/rlvgl.git /opt/rlvgl
docker exec rlvgl-builder git submodule update --init --depth=3
docker exec rlvgl-builder git remote add origin-ssh git@github.com:SoftOboros/rlvgl
docker exec rlvgl-builder git pull --set-upstream origin-ssh main
docker exec -it rlvgl-builder bash
