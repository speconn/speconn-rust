#!/bin/sh
set -e
cd "$(dirname "$0")"
PROXY=${PROXY:-http://127.0.0.1:17890}
podman build --network host \
    --build-arg HTTP_PROXY=$PROXY --build-arg HTTPS_PROXY=$PROXY \
    --build-arg http_proxy=$PROXY --build-arg https_proxy=$PROXY \
    --build-arg ALL_PROXY=$PROXY --build-arg all_proxy=$PROXY \
    -t speconn-rust:build -f Containerfile.build .
echo "speconn-rust: build OK"
