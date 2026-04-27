#!/bin/sh
set -e
cd "$(dirname "$0")"
podman build -t speconn-rust:build -f Containerfile.build .
echo "speconn-rust: build OK"
