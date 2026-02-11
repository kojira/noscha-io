# noscha.io — AI Agent Skill Guide

## Service Overview

noscha.io provides **disposable email forwarding, subdomain DNS, and NIP-05 Nostr identity** — all paid instantly via Lightning Network. No KYC, no signup, no accounts. Just pick a username, pay sats, and your services are live.

**Base URL:** `https://noscha.io`

## Quick Start (3 Steps)

### Step 1: Check username availability

```
GET /api/check/{username}
```

**Response:**
```json
{"available": true, "username": "alice"}
```

### Step 2: Create order (webhook verification required)

```
POST /api/order
Content-Type: application/json

{
  "username": "alice",
  "plan": "30d",
  "webhook_url": "https://your-server.com/webhook",
  "services": {
    "email": {},
    "subdomain": {"type": "CNAME", "target": "mysite.example.com", "proxied": false},
    "nip05": {"pubkey": "abc123...hex"}
  }
}
```

**Response:**
```json
{
  "order_id": "ord_18f3a...",
  "amount_sats": 6500,
  "bolt11": "",
  "expires_at": "2026-02-11T12:15:00Z",
  "status": "webhook_pending",
  "message": "Check your webhook for the challenge URL. Visit it to confirm and get an invoice."
}
```

A challenge POST is sent to your `webhook_url`:
```json
{"event": "webhook_challenge", "challenge_url": "https://noscha.io/api/order/{order_id}/confirm/{challenge}", "order_id": "ord_18f3a..."}
```

### Step 2b: Confirm webhook & get invoice

Visit the `challenge_url` from the webhook (GET request):
```
GET /api/order/{order_id}/confirm/{challenge}
```

**Response:**
```json
{
  "order_id": "ord_18f3a...",
  "amount_sats": 6500,
  "bolt11": "lnbc65000n1p...",
  "expires_at": "2026-02-11T12:15:00Z",
  "status": "pending"
}
```

You can include any combination of services (`email`, `subdomain`, `nip05`). `webhook_url` is **required** for all orders — email notifications are delivered via webhook.

### Step 3: Pay the invoice & poll for status

Pay the `bolt11` Lightning invoice, then poll:

```
GET /api/order/{order_id}/status
```

**Response (after payment):**
```json
{
  "order_id": "ord_18f3a...",
  "status": "provisioned",
  "management_token": "mgmt_18f3b..."
}
```

Save the `management_token` — it's needed for renewals and management.

## Endpoints Reference

### GET /api/check/{username}
Check if a username is available for registration.
- **username**: 1-20 chars, alphanumeric + hyphens, no leading/trailing hyphens
- Returns `{"available": bool, "username": string, "error"?: string}`

### POST /api/order
Create a new rental order. Returns a Lightning invoice.
- **Body**: `{"username": string, "plan": string, "services"?: {...}}`
- **plan**: `"1d"` | `"7d"` | `"30d"` | `"90d"` | `"365d"`
- **services.email**: `{"forward_to": "email@example.com"}`
- **services.subdomain**: `{"type": "A"|"AAAA"|"CNAME", "target": string, "proxied"?: bool}`
- **services.nip05**: `{"pubkey": "hex_pubkey"}`
- Returns `{"order_id", "amount_sats", "bolt11", "expires_at", "management_token"?}`
- Invoice expires in 15 minutes

### GET /api/order/{order_id}/status
Poll order status after payment.
- Returns `{"order_id", "status": "pending"|"paid"|"provisioned"|"expired", "management_token"?}`
- `management_token` is returned only when `status` is `"provisioned"`

### POST /api/renew
Extend an existing rental.
- **Body**: `{"management_token": string, "plan": string, "services"?: {...}}`
- Returns `{"order_id", "amount_sats", "bolt11", "expires_at"}`
- Time is added on top of current expiry (not from now)

### GET /api/pricing
Get current pricing for all plans and services.
- Returns pricing matrix: `{"1d": {"subdomain": 500, "email": 1500, "nip05": 200, "bundle": 1800}, ...}`

### GET /api/info
Service metadata.

### GET /health
Health check. Returns `{"status": "ok", "version": "..."}`.

### GET /.well-known/nostr.json?name={username}
NIP-05 verification endpoint (Nostr protocol).

### GET /api/mail/{token}
Retrieve an email from the inbox using the token received via webhook notification.
- **token**: Random UUID token from the webhook notification URL
- Returns the email data including from, to, subject, body_text, body_html, date, etc.
- Marks the email as read (sets read_at timestamp) on first access
- Returns 404 if email not found

### POST /api/mail/send/{management_token}
Send an email via Resend API from your noscha.io email address.
- **management_token**: Your rental's management token for authentication
- **Body**: `{"to": "recipient@example.com", "subject": "Subject", "body": "Email content"}`
- Rate limited to 5 emails per 24 hours per account
- From address will be `{username}@noscha.io`
- Returns `{"success": true, "message_id": "resend_message_id"}` on success

### PUT /api/settings/{management_token}
Update rental settings, currently supports setting webhook URL for email notifications.
- **management_token**: Your rental's management token for authentication
- **Body**: `{"webhook_url": "https://your-server.com/webhook"}` (or null to disable)
- When webhook_url is set, incoming emails will trigger a POST to this URL instead of forwarding via Resend
- Webhook payload: `{"event": "email_received", "from": "sender@example.com", "to": "you@noscha.io", "subject": "...", "url": "https://noscha.io/api/mail/{token}", "received_at": "2026-02-11T..."}`

## Pricing (sats, Lightning Network)

| Plan | Subdomain | Email | NIP-05 | Bundle (all 3) |
|------|-----------|-------|--------|-----------------|
| 1 day | 500 | 1,500 | 200 | 1,800 |
| 7 days | 1,000 | 2,500 | 500 | 3,300 |
| 30 days | 2,000 | 5,000 | 1,000 | 6,500 |
| 90 days | 5,000 | 12,000 | 2,500 | 16,000 |
| 365 days | 15,000 | 40,000 | 8,000 | 50,000 |

Bundle discount applies automatically when all 3 services are selected. Prices may change — always check `/api/pricing` for current rates.

## Typical Agent Workflow

1. **Decide** what services you need (email forwarding? subdomain? NIP-05?)
2. **GET /api/check/{username}** — verify availability
3. **GET /api/pricing** — confirm current pricing
4. **POST /api/order** — create order with desired services
5. **Pay** the `bolt11` invoice via any Lightning wallet/API
6. **GET /api/order/{order_id}/status** — poll until `"provisioned"` (poll every 3s, max ~5 min)
7. **Store** the `management_token` for future renewals
8. **POST /api/renew** when rental is nearing expiry

## Limitations

- Username: 1-20 characters, alphanumeric and hyphens only
- Invoice expires in 15 minutes after creation
- Lightning Network payments only (Bitcoin)
- No refunds (disposable service by design)
- DNS propagation may take up to 5 minutes after provisioning
- Email forwarding is one-to-one (one forwarding address per username)
- All services under one username share the same expiry date

## Terms of Service (Summary)

- Service is provided as-is for legitimate use
- Abuse (spam, phishing, illegal content) will result in immediate termination
- No personal data is collected beyond what's needed for the service
- Payments are final and non-refundable
- Service availability is best-effort
