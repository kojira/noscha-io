#!/bin/bash
set -euo pipefail

# noscha.io staging E2E test
BASE_URL="https://staging.noscha.io"
TOKEN=$(cat /Users/kojira/.openclaw/workspace/data/secrets/noscha_staging_token.txt)
ADMIN_TOKEN=$(cat /Users/kojira/.openclaw/workspace/projects/noscha-io/data/secrets/noscha_admin_api_token.txt)

PASS=0
FAIL=0
RESULTS=()

assert() {
  local name="$1" expected="$2" actual="$3"
  if [ "$expected" = "$actual" ]; then
    echo "  PASS: $name"
    RESULTS+=("PASS: $name")
    PASS=$((PASS+1))
  else
    echo "  FAIL: $name (expected=$expected, actual=$actual)"
    RESULTS+=("FAIL: $name (expected=$expected, actual=$actual)")
    FAIL=$((FAIL+1))
  fi
}

assert_contains() {
  local name="$1" needle="$2" haystack="$3"
  if echo "$haystack" | grep -q "$needle"; then
    echo "  PASS: $name"
    RESULTS+=("PASS: $name")
    PASS=$((PASS+1))
  else
    echo "  FAIL: $name (not found: $needle)"
    RESULTS+=("FAIL: $name (not found: $needle)")
    FAIL=$((FAIL+1))
  fi
}

echo "=========================================="
echo " noscha.io staging E2E Tests"
echo "=========================================="
echo ""

# 1. Public endpoints
echo "[1] Public Endpoints"
CODE=$(curl -s -o /dev/null -w "%{http_code}" "$BASE_URL/health")
assert "/health returns 200" "200" "$CODE"

CODE=$(curl -s -o /dev/null -w "%{http_code}" "$BASE_URL/.well-known/nostr.json?name=_")
assert "/.well-known/nostr.json?name=_ returns 404" "404" "$CODE"
echo ""

# 2. Bearer auth
echo "[2] Bearer Authentication"
CODE=$(curl -s -o /dev/null -w "%{http_code}" "$BASE_URL/api/info")
assert "/api/info without Bearer returns 403" "403" "$CODE"

CODE=$(curl -s -o /dev/null -w "%{http_code}" -H "Authorization: Bearer $TOKEN" "$BASE_URL/api/info")
assert "/api/info with Bearer returns 200" "200" "$CODE"

CODE=$(curl -s -o /dev/null -w "%{http_code}" -H "Authorization: Bearer $TOKEN" "$BASE_URL/api/pricing")
assert "/api/pricing with Bearer returns 200" "200" "$CODE"
echo ""

# 3. Admin Bearer auth
echo "[3] Admin Bearer Authentication"
CODE=$(curl -s -o /dev/null -w "%{http_code}" "$BASE_URL/api/admin/rentals")
assert "/api/admin/rentals without Bearer returns 401" "401" "$CODE"

CODE=$(curl -s -o /dev/null -w "%{http_code}" -H "Authorization: Bearer $ADMIN_TOKEN" "$BASE_URL/api/admin/rentals")
assert "/api/admin/rentals with admin Bearer returns 200" "200" "$CODE"
echo ""

# 4. Order flow
echo "[4] Order Flow"
USERNAME="e2etest$(date +%s)"
ORDER_RESP=$(curl -s -X POST "$BASE_URL/api/order" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{\"username\":\"$USERNAME\",\"plan\":\"5m\",\"webhook_url\":\"https://example.com/webhook\",\"services\":{\"nip05\":{\"pubkey\":\"$(printf 'a%.0s' {1..64})\"}}}")

ORDER_ID=$(echo "$ORDER_RESP" | python3 -c "import json,sys; print(json.load(sys.stdin).get('order_id',''))" 2>/dev/null || echo "")
assert_contains "POST /api/order returns order_id" "ord_" "$ORDER_ID"

ORDER_STATUS=$(echo "$ORDER_RESP" | python3 -c "import json,sys; print(json.load(sys.stdin).get('status',''))" 2>/dev/null || echo "")
assert "Order status is webhook_pending" "webhook_pending" "$ORDER_STATUS"

if [ -n "$ORDER_ID" ]; then
  STATUS_CODE=$(curl -s -o /dev/null -w "%{http_code}" -H "Authorization: Bearer $TOKEN" "$BASE_URL/api/order/$ORDER_ID/status")
  assert "GET /api/order/{id}/status returns 200" "200" "$STATUS_CODE"
fi
echo ""

# 5. NIP-05
echo "[5] NIP-05 Verification"
NIP05_RESP=$(curl -s "$BASE_URL/.well-known/nostr.json?name=nonexistent_user_xyz")
assert_contains "NIP-05 unknown user returns appropriate response" "not found\|names\|{}" "$NIP05_RESP"
echo ""

# 6. Admin Plan CRUD
echo "[6] Admin Plan CRUD"
# Save original pricing
ORIGINAL_PRICING=$(curl -s -H "Authorization: Bearer $ADMIN_TOKEN" "$BASE_URL/api/admin/pricing")
PLAN_COUNT_BEFORE=$(echo "$ORIGINAL_PRICING" | python3 -c "import json,sys; print(len(json.load(sys.stdin)))")

# Add test plan
ADD_RESP=$(curl -s -X PUT "$BASE_URL/api/admin/pricing" \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"e2e_test_plan":{"_duration_minutes":1,"nip05":1,"email":1,"subdomain":1,"bundle":1}}')
assert_contains "PUT adds test plan" "e2e_test_plan" "$ADD_RESP"

# Verify via public pricing
PUB_PRICING=$(curl -s -H "Authorization: Bearer $TOKEN" "$BASE_URL/api/pricing")
assert_contains "Test plan visible in public pricing" "e2e_test_plan" "$PUB_PRICING"

# Remove test plan (PUT full pricing without it)
curl -s -X PUT "$BASE_URL/api/admin/pricing" \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d "$ORIGINAL_PRICING" > /dev/null

# Verify removal
PUB_PRICING2=$(curl -s -H "Authorization: Bearer $TOKEN" "$BASE_URL/api/pricing")
PLAN_COUNT_AFTER=$(echo "$PUB_PRICING2" | python3 -c "import json,sys; print(len(json.load(sys.stdin)))")
assert "Plan count restored after cleanup" "$PLAN_COUNT_BEFORE" "$PLAN_COUNT_AFTER"
echo ""

# 7. LLM dynamic docs
echo "[7] LLM Dynamic Documents"
LLMS_TXT=$(curl -s -H "Authorization: Bearer $TOKEN" "$BASE_URL/llms.txt")
assert_contains "/llms.txt contains pricing" "sats\|Pricing\|pricing" "$LLMS_TXT"

SKILL_MD=$(curl -s -H "Authorization: Bearer $TOKEN" "$BASE_URL/skill.md")
assert_contains "/skill.md contains pricing" "sats\|Pricing\|pricing" "$SKILL_MD"
echo ""

# Summary
echo "=========================================="
echo " SUMMARY: $PASS passed, $FAIL failed"
echo "=========================================="
for r in "${RESULTS[@]}"; do
  echo "  $r"
done

exit $FAIL
