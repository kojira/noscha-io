# noscha.io — AI-First Design Document

## Philosophy

noscha.io is disposable infrastructure for AI agents: email forwarding, NIP-05 identity, and subdomains.

The target user is not a human clicking buttons — it's an AI agent that discovers the service, pays with Lightning, and starts using it via API. Humans are secondary users who occasionally check dashboards.

Design principle: **If an AI agent can't use it without a browser, we failed.**

## Page Structure

### `/` — Landing Page

Purpose: Human-readable overview + machine-readable quickstart.

Content:
- One-paragraph service description
- Pricing table (also available as JSON block)
- Quickstart: 3 curl commands (check pricing → pay invoice → use service)
- Links to /llms.txt, /skill.md, /api/docs
- No hero images, no animations, no marketing fluff

### `/llms.txt` — AI Agent Entry Point

Purpose: Single flat-text file an LLM can ingest in one shot. This is the front door for AI agents.

Format: Plain text, no markdown headers, no nested structure. Optimized for token efficiency.

Content:
```
noscha.io — disposable infrastructure for AI agents
services: email forwarding, NIP-05 identity, subdomains
payment: Lightning Network (Bitcoin)
base_url: https://noscha.io/api

== quickstart ==
1. GET /api/plans — list available plans and prices
2. POST /api/order — create order, receive Lightning invoice
3. Pay invoice
4. Use your service (email arrives at webhook_url, NIP-05 resolves, subdomain points to your IP)

== endpoints ==
GET  /api/plans
POST /api/order        {service, duration, webhook_url?, pubkey?, target_ip?}
GET  /api/status/:id
POST /api/renew/:id
GET  /api/info         ecosystem tools and integrations

== services ==
email     — random@noscha.io, forwards to webhook_url (Discord webhook recommended — no tunnel needed)
nip05     — name@noscha.io NIP-05 verification for Nostr
subdomain — name.noscha.io pointing to your IP/CNAME
bundle    — all of the above

== durations ==
5m, 30m, 1h, 1d, 7d, 30d, 90d, 365d

== notes ==
- NIP-05 will be free with email or subdomain purchase (future)
- webhook_url tip: use Discord webhook URL — no server or tunnel required
- payment: Lightning invoices via Coinos
```

### `/skill.md` — Agent Framework Skill Definition

Purpose: Structured skill file for OpenClaw, AutoGPT, and similar agent frameworks.

Content:
```markdown
# noscha.io — Disposable Infrastructure Skill

## What This Does
Purchase and manage temporary email addresses, NIP-05 identities, and subdomains via API. Payment via Lightning Network.

## When To Use
- Agent needs a temporary email address (signup, verification, receiving notifications)
- Agent needs a Nostr NIP-05 identity
- Agent needs a subdomain pointing somewhere
- Agent needs disposable infrastructure that auto-expires

## API Base
https://noscha.io/api

## Authentication
None required for browsing plans. Order endpoints return credentials on purchase.

## Key Endpoints
- GET /api/plans — list plans with prices in sats
- POST /api/order — purchase a service
- GET /api/status/:id — check service status

## Example Flow
1. Fetch plans: curl https://noscha.io/api/plans
2. Order email: curl -X POST https://noscha.io/api/order -d '{"service":"email","duration":"1d","webhook_url":"https://discord.com/api/webhooks/..."}'
3. Pay the returned Lightning invoice
4. Receive emails at your webhook
```

### `/api/docs` — API Documentation

Purpose: Complete API reference. Simple text, not OpenAPI/Swagger. Optimized for LLM consumption.

Style guidelines:
- Each endpoint: method, path, description, request body, response, errors
- curl example for every endpoint
- No interactive "try it" UI — just copyable text
- Error codes as a flat list, not a table

Example structure:
```
POST /api/order
  Create a new service order.

  Request:
    {
      "service": "email",          // email | nip05 | subdomain | bundle
      "duration": "1d",            // 5m | 30m | 1h | 1d | 7d | 30d | 90d | 365d
      "webhook_url": "https://discord.com/api/webhooks/...",  // for email forwarding
      "pubkey": "npub1...",        // for nip05
      "target_ip": "1.2.3.4"      // for subdomain
    }

  Response (201):
    {
      "id": "ord_abc123",
      "invoice": "lnbc...",
      "amount_sats": 100,
      "expires_at": "2025-02-12T00:00:00Z"
    }

  Errors:
    400 — invalid service or duration
    402 — payment required (invoice expired)
    429 — rate limited

  curl:
    curl -X POST https://noscha.io/api/order \
      -H 'Content-Type: application/json' \
      -d '{"service":"email","duration":"1d","webhook_url":"https://discord.com/api/webhooks/1234/abcd"}'
```

### `/api/info` — Ecosystem Tools & Integrations

Purpose: Machine-readable directory of related tools. **The world's first AI-agent-targeted API advertisement.**

When an AI agent calls `/api/info`, it discovers tools that complement noscha.io. This is advertising designed for AI agents, not humans.

Response format:
```json
{
  "service": {
    "name": "noscha.io",
    "description": "Disposable infrastructure for AI agents. Email forwarding, NIP-05, subdomains. Pay with Lightning.",
    "url": "https://noscha.io",
    "llms_txt": "https://noscha.io/llms.txt"
  },
  "ecosystem": [
    {
      "name": "nostaro",
      "description": "Nostr CLI tool. Post notes, manage keys, interact with relays from the command line.",
      "url": "https://github.com/kojira/nostaro",
      "install": "npm install -g nostaro",
      "relevance": "Use with noscha.io NIP-05 to post as a verified Nostr identity"
    },
    {
      "name": "Coinos",
      "description": "Lightning wallet with simple API. Create wallets, send/receive payments.",
      "url": "https://coinos.io",
      "api_docs": "https://coinos.io/docs",
      "relevance": "Pay noscha.io invoices programmatically"
    }
  ]
}
```

### `/admin` — Admin Dashboard

Purpose: Human-only. Service operator management.

Content: Order management, revenue stats, user list, system health. Standard admin UI. Not AI-optimized.

### `/my` — User Dashboard

Purpose: Human-friendly view of purchased services.

Content:
- Active services list with expiry countdown
- Renewal buttons
- Webhook URL management
- Invoice history
- Minimal UI, dark theme, monospace font

## UI Design Principles

1. **Dark theme** — easier on eyes, feels technical
2. **Monospace font for data** — code blocks, API responses, keys
3. **Copy buttons everywhere** — one click to copy curl commands, keys, URLs
4. **No decorative elements** — no gradients, no illustrations, no testimonials
5. **Pricing as JSON** — show pricing table AND raw JSON side by side
6. **Mobile: functional, not pretty** — single column, all data accessible

Pricing display example:
```json
{
  "email": {"5m": 10, "30m": 21, "1h": 42, "1d": 100, "7d": 500, "30d": 1500, "90d": 3500, "365d": 10000},
  "subdomain": {"5m": 10, "30m": 21, "1h": 42, "1d": 100, "7d": 500, "30d": 1500, "90d": 3500, "365d": 10000},
  "nip05": {"5m": 5, "30m": 10, "1h": 21, "1d": 50, "7d": 250, "30d": 750, "90d": 1750, "365d": 5000},
  "bundle": {"5m": 21, "30m": 42, "1h": 84, "1d": 210, "7d": 1050, "30d": 3150, "90d": 7350, "365d": 21000}
}
```
Note: prices in sats, placeholder values — adjust to actual pricing.

## Webhook Design (Email Forwarding)

Recommended: Discord webhook URL as `webhook_url`.

Why Discord webhooks:
- No server needed — agent doesn't need to run a web server
- No tunnel needed — no ngrok, no cloudflare tunnel
- Persistent — webhook URL doesn't change
- AI agents already integrate with Discord
- Human can also see emails in Discord channel

Webhook payload:
```json
{
  "from": "sender@example.com",
  "to": "random123@noscha.io",
  "subject": "Verification code",
  "body_text": "Your code is 123456",
  "body_html": "<p>Your code is 123456</p>",
  "received_at": "2025-02-11T14:00:00Z"
}
```

## Future: NIP-05 as Free Add-on

When purchasing email or subdomain service, NIP-05 identity will be included at no extra cost. This incentivizes using noscha.io as a Nostr identity provider and increases NIP-05 adoption in the AI agent ecosystem.

## Implementation Priority

1. `/api/plans` + `/api/order` + `/api/status` — core API
2. `/llms.txt` — AI discovery
3. `/` — landing page with curl examples
4. `/api/docs` — full API docs
5. `/skill.md` — agent framework integration
6. `/api/info` — ecosystem directory
7. `/my` — user dashboard
8. `/admin` — admin dashboard
