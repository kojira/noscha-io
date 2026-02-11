#!/bin/bash
# noscha.io E2E test - robust version

BASE="https://noscha.io"
UA="User-Agent: noscha-e2e-test/1.0"
RESEND_KEY=$(cat /Users/kojira/.openclaw/workspace/data/secrets/resend_api_key.txt)
WEBHOOK_URL="https://discord.com/api/webhooks/1471130882935886059/0Aq31m2twrCdI8xIgXr9nQwtfMj7MiU-xuyGqcwcl-udiDW11wlaTWQan1wt8M8VCKw_"
TIMESTAMP=$(date +%s)
TEST_USER="e2e${TIMESTAMP: -8}"

PASS=0
FAIL=0
RESULTS=""
CF_PID=""
CF2_PID=""
WH_PID=""
WH2_PID=""

cleanup() {
  [ -n "$CF_PID" ] && kill $CF_PID 2>/dev/null
  [ -n "$CF2_PID" ] && kill $CF2_PID 2>/dev/null
  [ -n "$WH_PID" ] && kill $WH_PID 2>/dev/null
  [ -n "$WH2_PID" ] && kill $WH2_PID 2>/dev/null
}
trap cleanup EXIT

report() {
  if [ "$2" = "PASS" ]; then
    RESULTS="${RESULTS}✅ $1\n"
    PASS=$((PASS+1))
  else
    RESULTS="${RESULTS}❌ $1\n"
    FAIL=$((FAIL+1))
  fi
  echo "  [$2] $1"
}

wh_post() {
  curl -s -X POST "$WEBHOOK_URL" -H "Content-Type: application/json" -d "{\"content\":\"$1\"}" >/dev/null 2>&1
}

start_tunnel() {
  local port=$1 logfile=$2 pidfile=$3
  rm -f "$logfile"
  cloudflared tunnel --url "http://127.0.0.1:$port" --no-autoupdate 2>"$logfile" &
  echo "$!" > "$pidfile"
  # Wait for URL (up to 60s)
  for i in $(seq 1 60); do
    if grep -qE 'https://.*trycloudflare\.com' "$logfile" 2>/dev/null; then
      return 0
    fi
    sleep 1
  done
  return 1
}

get_tunnel_url() {
  grep -oE 'https://[a-z0-9-]+\.trycloudflare\.com' "$1" 2>/dev/null | head -1
}

# ============ 1. API正常系 ============
echo "=== 1. API正常系 ==="

R=$(curl -s -o /dev/null -w "%{http_code}" -H "$UA" "$BASE/health")
[ "$R" = "200" ] && report "GET /health" "PASS" || report "GET /health" "FAIL"

R=$(curl -s -H "$UA" "$BASE/api/info")
echo "$R" | jq -e '.name' >/dev/null 2>&1 && report "GET /api/info" "PASS" || report "GET /api/info" "FAIL"

R=$(curl -s -H "$UA" "$BASE/api/pricing")
echo "$R" | jq -e '.plans' >/dev/null 2>&1 && report "GET /api/pricing" "PASS" || report "GET /api/pricing" "FAIL"

R=$(curl -s -H "$UA" "$BASE/api/check/$TEST_USER")
echo "$R" | jq -e '.available == true' >/dev/null 2>&1 && report "GET /api/check (available)" "PASS" || report "GET /api/check (available)" "FAIL"

R=$(curl -s -o /dev/null -w "%{http_code}" -H "$UA" "$BASE/api/docs")
[ "$R" = "200" ] && report "GET /api/docs" "PASS" || report "GET /api/docs" "FAIL"

R=$(curl -s -o /dev/null -w "%{http_code}" -H "$UA" "$BASE/skill.md")
[ "$R" = "200" ] && report "GET /skill.md" "PASS" || report "GET /skill.md" "FAIL"

R=$(curl -s -o /dev/null -w "%{http_code}" -H "$UA" "$BASE/llms.txt")
[ "$R" = "200" ] && report "GET /llms.txt" "PASS" || report "GET /llms.txt" "FAIL"

wh_post "1️⃣ API正常系完了 (${PASS}pass)"

# ============ 2. バリデーション ============
echo "=== 2. バリデーション ==="

R=$(curl -s -H "$UA" "$BASE/api/check/admin")
echo "$R" | jq -e '.available == false' >/dev/null 2>&1 && report "Reserved: admin" "PASS" || report "Reserved: admin" "FAIL"

R=$(curl -s -H "$UA" "$BASE/api/check/www")
echo "$R" | jq -e '.available == false' >/dev/null 2>&1 && report "Reserved: www" "PASS" || report "Reserved: www" "FAIL"

R=$(curl -s -H "$UA" "$BASE/api/check/a")
echo "$R" | jq -e '.available == false' >/dev/null 2>&1 && report "Too short" "PASS" || report "Too short" "FAIL"

R=$(curl -s -H "$UA" "$BASE/api/check/TestUpper")
echo "$R" | jq -e '.available == false' >/dev/null 2>&1 && report "Uppercase invalid" "PASS" || report "Uppercase invalid" "FAIL"

R=$(curl -s -H "$UA" "$BASE/api/check/-test")
echo "$R" | jq -e '.available == false' >/dev/null 2>&1 && report "Leading hyphen" "PASS" || report "Leading hyphen" "FAIL"

wh_post "2️⃣ バリデーション完了"

# ============ 3. 注文フロー ============
echo "=== 3. 注文フロー ==="

# Webhook receiver
cat > /tmp/noscha_wh.py << 'EOF'
import http.server, json, sys
class H(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        n = int(self.headers.get('Content-Length', 0))
        body = json.loads(self.rfile.read(n)) if n else {}
        if body.get('event') == 'webhook_challenge':
            with open('/tmp/noscha_challenge.txt', 'w') as f:
                f.write(body.get('challenge_url', ''))
        self.send_response(200)
        self.end_headers()
        self.wfile.write(b'ok')
    def do_GET(self):
        self.send_response(200)
        self.end_headers()
        self.wfile.write(b'ok')
    def log_message(self, *a): pass
http.server.HTTPServer(('127.0.0.1', int(sys.argv[1])), H).serve_forever()
EOF

rm -f /tmp/noscha_challenge.txt
python3 /tmp/noscha_wh.py 18932 &
WH_PID=$!
echo "Webhook PID: $WH_PID"
sleep 1

# Start tunnel
echo "Starting tunnel..."
start_tunnel 18932 /tmp/noscha_cf1.log /tmp/noscha_cf1.pid
CF_PID=$(cat /tmp/noscha_cf1.pid)
TUNNEL_URL=$(get_tunnel_url /tmp/noscha_cf1.log)
echo "Tunnel: $TUNNEL_URL (CF PID: $CF_PID)"

MGMT_TOKEN=""
ORDER_ID=""

if [ -z "$TUNNEL_URL" ]; then
  report "Cloudflared tunnel" "FAIL"
  wh_post "3️⃣ トンネル起動失敗 ❌"
else
  report "Cloudflared tunnel" "PASS"

  # Test tunnel
  curl -s -o /dev/null --max-time 10 "$TUNNEL_URL/" || true
  sleep 2

  PUBKEY="a1b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1"

  ORDER_RESP=$(curl -s -H "$UA" -H "Content-Type: application/json" -X POST "$BASE/api/order" -d "{
    \"username\": \"$TEST_USER\",
    \"plan\": \"5m\",
    \"webhook_url\": \"$TUNNEL_URL\",
    \"services\": {
      \"nip05\": { \"pubkey\": \"$PUBKEY\" },
      \"email\": { \"forward_to\": \"test@example.com\" }
    }
  }")
  echo "Order: $ORDER_RESP"

  ORDER_ID=$(echo "$ORDER_RESP" | jq -r '.order_id // empty')
  ORDER_STATUS=$(echo "$ORDER_RESP" | jq -r '.status // empty')

  if [ -n "$ORDER_ID" ]; then
    report "POST /api/order" "PASS"

    # Wait for challenge
    echo "Waiting for challenge..."
    for i in $(seq 1 20); do
      [ -s /tmp/noscha_challenge.txt ] && break
      sleep 1
    done

    CHALLENGE_URL=$(cat /tmp/noscha_challenge.txt 2>/dev/null)
    echo "Challenge: $CHALLENGE_URL"

    if [ -n "$CHALLENGE_URL" ]; then
      report "Webhook challenge" "PASS"

      CONFIRM_RESP=$(curl -s -H "$UA" "$CHALLENGE_URL")
      echo "Confirm: $CONFIRM_RESP"

      MGMT_TOKEN=$(echo "$CONFIRM_RESP" | jq -r '.management_token // empty')
      CONFIRM_STATUS=$(echo "$CONFIRM_RESP" | jq -r '.status // empty')

      if [ -n "$MGMT_TOKEN" ] && [ "$CONFIRM_STATUS" = "provisioned" ]; then
        report "Confirm + provision" "PASS"
      else
        report "Confirm + provision" "FAIL"
      fi
    else
      report "Webhook challenge" "FAIL"
      report "Confirm + provision" "FAIL"
    fi
  else
    report "POST /api/order" "FAIL"
    report "Webhook challenge" "FAIL"
    report "Confirm + provision" "FAIL"
  fi
fi

# Stop tunnel 1
[ -n "$CF_PID" ] && kill $CF_PID 2>/dev/null && CF_PID=""
[ -n "$WH_PID" ] && kill $WH_PID 2>/dev/null && WH_PID=""

wh_post "3️⃣ 注文フロー完了"

# ============ 4. NIP-05 ============
echo "=== 4. NIP-05 ==="
if [ -n "$MGMT_TOKEN" ]; then
  NIP05=$(curl -s -H "$UA" "$BASE/.well-known/nostr.json?name=$TEST_USER")
  echo "NIP-05: $NIP05"
  echo "$NIP05" | jq -e ".names.\"$TEST_USER\"" >/dev/null 2>&1 && report "NIP-05 lookup" "PASS" || report "NIP-05 lookup" "FAIL"
else
  report "NIP-05 lookup (skipped)" "FAIL"
fi
wh_post "4️⃣ NIP-05完了"

# ============ 5. メール転送 ============
echo "=== 5. メール転送 ==="
if [ -n "$MGMT_TOKEN" ]; then
  MAIL=$(curl -s -X POST "https://api.resend.com/emails" \
    -H "Authorization: Bearer $RESEND_KEY" \
    -H "Content-Type: application/json" \
    -d "{\"from\":\"test@noscha.io\",\"to\":\"${TEST_USER}@noscha.io\",\"subject\":\"E2E ${TIMESTAMP}\",\"text\":\"test\"}")
  echo "Resend: $MAIL"
  echo "$MAIL" | jq -e '.id' >/dev/null 2>&1 && report "Email via Resend" "PASS" || report "Email via Resend" "FAIL"
else
  report "Email via Resend (skipped)" "FAIL"
fi
wh_post "5️⃣ メール転送完了"

# ============ 7. 不正アクセス ============
echo "=== 7. 不正アクセス ==="

R=$(curl -s -o /dev/null -w "%{http_code}" -H "$UA" "$BASE/my/invalid_token_xxx")
[ "$R" = "404" ] && report "Invalid token → 404" "PASS" || report "Invalid token → 404 (got $R)" "FAIL"

R=$(curl -s -o /dev/null -w "%{http_code}" -H "$UA" -H "Content-Type: application/json" -X POST "$BASE/api/order" -d '{"username":"validuser","plan":"99y","webhook_url":"https://example.com"}')
[ "$R" = "400" ] && report "Invalid plan → 400" "PASS" || report "Invalid plan → 400 (got $R)" "FAIL"

if [ -n "$MGMT_TOKEN" ]; then
  R=$(curl -s -o /dev/null -w "%{http_code}" -H "$UA" -H "Content-Type: application/json" -X POST "$BASE/api/order" -d "{\"username\":\"$TEST_USER\",\"plan\":\"5m\",\"webhook_url\":\"https://example.com\"}")
  [ "$R" = "409" ] && report "Taken username → 409" "PASS" || report "Taken username → 409 (got $R)" "FAIL"
fi

R=$(curl -s -o /dev/null -w "%{http_code}" -H "$UA" "$BASE/api/admin/rentals")
([ "$R" = "401" ] || [ "$R" = "403" ]) && report "Admin no auth" "PASS" || report "Admin no auth (got $R)" "FAIL"

wh_post "7️⃣ 不正アクセス完了"

# ============ 8. 延長テスト ============
echo "=== 8. 延長テスト ==="
if [ -n "$MGMT_TOKEN" ]; then
  rm -f /tmp/noscha_challenge.txt
  python3 /tmp/noscha_wh.py 18933 &
  WH2_PID=$!
  sleep 1

  start_tunnel 18933 /tmp/noscha_cf2.log /tmp/noscha_cf2.pid
  CF2_PID=$(cat /tmp/noscha_cf2.pid)
  T2_URL=$(get_tunnel_url /tmp/noscha_cf2.log)
  echo "Tunnel2: $T2_URL (CF2 PID: $CF2_PID)"

  if [ -n "$T2_URL" ]; then
    curl -s -o /dev/null --max-time 10 "$T2_URL/" || true
    sleep 2

    RENEW=$(curl -s -H "$UA" -H "Content-Type: application/json" -X POST "$BASE/api/renew" -d "{
      \"management_token\": \"$MGMT_TOKEN\",
      \"plan\": \"5m\",
      \"webhook_url\": \"$T2_URL\"
    }")
    echo "Renew: $RENEW"
    RO=$(echo "$RENEW" | jq -r '.order_id // empty')

    if [ -n "$RO" ]; then
      report "POST /api/renew" "PASS"
      for i in $(seq 1 20); do
        [ -s /tmp/noscha_challenge.txt ] && break
        sleep 1
      done
      RC=$(cat /tmp/noscha_challenge.txt 2>/dev/null)
      if [ -n "$RC" ]; then
        RCONF=$(curl -s -H "$UA" "$RC")
        echo "Renew confirm: $RCONF"
        RS=$(echo "$RCONF" | jq -r '.status // empty')
        [ "$RS" = "provisioned" ] && report "Renewal confirmed" "PASS" || report "Renewal confirmed" "FAIL"
      else
        report "Renewal confirmed (no challenge)" "FAIL"
      fi
    else
      report "POST /api/renew" "FAIL"
      report "Renewal confirmed" "FAIL"
    fi
  else
    report "POST /api/renew (no tunnel)" "FAIL"
    report "Renewal confirmed" "FAIL"
  fi

  [ -n "$CF2_PID" ] && kill $CF2_PID 2>/dev/null && CF2_PID=""
  [ -n "$WH2_PID" ] && kill $WH2_PID 2>/dev/null && WH2_PID=""
else
  report "POST /api/renew (skipped)" "FAIL"
  report "Renewal confirmed (skipped)" "FAIL"
fi
wh_post "8️⃣ 延長テスト完了"

# ============ 6. 期限切れ（5分+余裕待ち） ============
echo "=== 6. 期限切れテスト ==="
wh_post "⏳ 期限切れ: 5分+余裕待機中..."

# After renewal, expiry is extended. Need to wait for the renewed expiry.
# 5m plan renewed = original 5m + 5m extension, but we wait from now.
# Actually the renewal extends from current expiry, so total could be up to ~10 min from initial order.
# But the renewal was just confirmed, so the new expiry is ~5 min from now.
sleep 330

NIP05_AFTER=$(curl -s -H "$UA" "$BASE/.well-known/nostr.json?name=$TEST_USER")
echo "NIP-05 after: $NIP05_AFTER"
if echo "$NIP05_AFTER" | jq -e ".names.\"$TEST_USER\"" >/dev/null 2>&1; then
  report "NIP-05 gone after expiry" "FAIL"
else
  report "NIP-05 gone after expiry" "PASS"
fi

if [ -n "$MGMT_TOKEN" ]; then
  MY=$(curl -s -H "$UA" "$BASE/my/$MGMT_TOKEN")
  echo "$MY" | grep -qi "expir" && report "My page shows expired" "PASS" || report "My page shows expired" "FAIL"
fi

R=$(curl -s -H "$UA" "$BASE/api/check/$TEST_USER")
echo "$R" | jq -e '.available == true' >/dev/null 2>&1 && report "Username available after expiry" "PASS" || report "Username available after expiry" "FAIL"

wh_post "6️⃣ 期限切れテスト完了"

# ============ 結果 ============
echo ""
echo "================================"
echo "RESULTS: ${PASS} passed, ${FAIL} failed"
echo -e "$RESULTS"
echo "================================"

SUMMARY=$(printf "🧪 **noscha.io E2Eテスト結果**\n\n%b\n**合計: %d PASS / %d FAIL**" "$RESULTS" "$PASS" "$FAIL")
curl -s -X POST "$WEBHOOK_URL" -H "Content-Type: application/json" -d "{\"content\":$(echo "$SUMMARY" | jq -Rs .)}"

echo "Done!"
