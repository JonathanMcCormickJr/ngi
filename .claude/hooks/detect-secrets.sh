#!/bin/bash
set -euo pipefail

HOOK_INPUT=$(cat)
NEW_CONTENT=$(echo "$HOOK_INPUT" | jq -r '.tool_input.new_string // .tool_input.content // empty')

[ -z "$NEW_CONTENT" ] && exit 0

SECRET_PATTERNS=(
  'AKIA[0-9A-Z]{16}'           # AWS Access Key
  'ghp_[a-zA-Z0-9]{36}'        # GitHub token
  'sk_live_[a-zA-Z0-9]{24,}'   # Stripe live key
  'api[_-]?key["\s:=]+["\x27]?[a-zA-Z0-9_-]{20,}'
  'private[_-]?key'
  'client[_-]?secret'
  'password["\s:=]+"[^\s"]{8,}"'
  'mongodb(\+srv)?://[^\s]+'
  'postgres(ql)?://[^\s]+'
)

for pattern in "${SECRET_PATTERNS[@]}"; do
  if echo "$NEW_CONTENT" | grep -qiE "$pattern"; then
    echo "Secret pattern detected: $pattern — use an environment variable instead" >&2
    exit 2
  fi
done

exit 0
