#!/bin/sh
set -eu

install -d -o elkitten -g elkitten /home/elkitten/.cache
install -d -o elkitten -g elkitten /home/elkitten/.cache/huggingface
install -d -o elkitten -g elkitten /home/elkitten/.config
install -d -o elkitten -g elkitten /home/elkitten/.config/pulse

exec gosu elkitten "$@"