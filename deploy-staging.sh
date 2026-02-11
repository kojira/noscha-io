#!/bin/bash
set -e
# Load .env if exists
if [ -f .env ]; then
  export $(grep -v '^#' .env | xargs)
fi
npx wrangler deploy --env staging "$@"
