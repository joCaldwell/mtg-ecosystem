#!/bin/sh
set -eu
if [ "$(id -u)" = 0 ]; then
  # Mounted at runtime, so Dockerfile chown alone cannot prepare a Railway
  # volume: https://docs.railway.com/volumes/reference#permissions
  mkdir -p "${DECKBUILDER_DATA:-/data}"
  chown node:node "${DECKBUILDER_DATA:-/data}"
  chmod 700 "${DECKBUILDER_DATA:-/data}"
  exec setpriv --reuid=node --regid=node --init-groups "$@"
fi
exec "$@"
