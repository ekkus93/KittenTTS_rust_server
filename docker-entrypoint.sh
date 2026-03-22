#!/bin/sh
set -eu

# The container runs as elkitten (USER directive in Dockerfile).
# mkdir -p here is a safety net for fresh volume mounts; no chown is needed
# because directories created by elkitten are automatically owned by elkitten.
mkdir -p \
    /home/elkitten/.cache/huggingface \
    /home/elkitten/.config/pulse

exec "$@"
