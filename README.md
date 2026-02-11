# noscha.io

Lightning-powered disposable identity services for Nostr. Rent a username and get email receiving, subdomain DNS, and NIP-05 verification — no KYC, no signup, instant activation.

## Features

- **Email Receiving** — `username@noscha.io` receives mail and sends webhook notifications; received emails are automatically deleted after 1 hour
- **Subdomain DNS** — `username.noscha.io` pointing to your server (A/AAAA/CNAME)
- **NIP-05 Verification** — `username@noscha.io` Nostr identity verification
- **Lightning Payments** — Pay with Bitcoin Lightning via [coinos](https://coinos.io)
- **Flexible Plans** — 1 day to 1 year rentals
- **Admin Dashboard** — NIP-07 authenticated admin panel
- **Auto-cleanup** — Expired rentals and DNS records cleaned up automatically

## Tech Stack

- **Runtime**: Cloudflare Workers (WebAssembly)
- **Language**: Rust (compiled to wasm32 via [worker-rs](https://github.com/cloudflare/workers-rs))
- **Storage**: Cloudflare R2 (object storage)
- **DNS**: Cloudflare API for subdomain provisioning
- **Payments**: coinos.io Lightning invoices
- **Email**: Cloudflare Email Routing + Resend API
- **Auth**: NIP-07 (Nostr browser extension signing)

## Prerequisites

- [Rust](https://rustup.rs/) with `wasm32-unknown-unknown` target
- [Node.js](https://nodejs.org/) (for wrangler CLI)
- [wrangler](https://developers.cloudflare.com/workers/wrangler/) (`npm i -g wrangler`)
- `worker-build` (`cargo install worker-build`)

## Setup

1. Clone the repository:
   ```bash
   git clone https://github.com/kojira/noscha-io.git
   cd noscha-io
   ```

2. Install the wasm target:
   ```bash
   rustup target add wasm32-unknown-unknown
   ```

3. Create a `.env` file with the required secrets (see [Secrets](#secrets) below).

4. Configure `wrangler.toml` with your domain and R2 bucket.

## Secrets

The following secrets are required. Store them in `.env` (loaded automatically by deploy scripts) and/or set via `wrangler secret put`:

| Secret | Description |
|---|---|
| `COINOS_API_TOKEN` | coinos.io API token for Lightning invoices |
| `WEBHOOK_SECRET` | Shared secret for coinos payment webhooks |
| `CF_API_TOKEN` | Cloudflare API token for DNS management |
| `CF_ZONE_ID` | Cloudflare zone ID for the domain |
| `ADMIN_PUBKEY` | Nostr public key (hex) for admin access |
| `ADMIN_API_TOKEN` | Bearer token for Admin API access |
| `STAGING_AUTH_TOKEN` | Auth token for staging environment gate |
| `DISCORD_WEBHOOK_URL` | Discord webhook for notifications |

## Environment Variables

Set in `wrangler.toml` under `[vars]`:

| Variable | Description |
|---|---|
| `DOMAIN` | Primary domain (e.g. `noscha.io`) |
| `MOCK_PAYMENT` | Set `"true"` to skip real Lightning payments (dev/test) |
| `REQUIRE_AUTH` | Set `"true"` to require NIP-07 auth for all pages (used in staging) |

## Development

```bash
# Run locally with wrangler dev
npx wrangler dev

# Build only
cargo install -q worker-build && worker-build --release
```

Set `MOCK_PAYMENT = "true"` in `[vars]` to skip real Lightning payments during development.

## Deploy

Two deploy scripts are provided. Both load `.env` automatically.

```bash
# Staging — deploys to staging.noscha.io
./deploy-staging.sh

# Production — deploys to noscha.io
./deploy-production.sh
```

### Staging Environment

The staging environment (`staging.noscha.io`) has `REQUIRE_AUTH = "true"`, requiring NIP-07 authentication to access any page. Only the `ADMIN_PUBKEY` holder (or requests with `STAGING_AUTH_TOKEN`) can access the staging site. Use staging to verify changes before production deployment.

## Admin API

The Admin API (`/api/admin/*`) supports two authentication methods:

1. **Bearer Token** — `Authorization: Bearer <ADMIN_API_TOKEN>`
2. **NIP-07 Signature** — Nostr event-based auth from the admin dashboard

Both methods require the `ADMIN_PUBKEY` to match.

## Testing

### Unit Tests

```bash
cargo test --lib
```

Currently **83 tests** covering validation, types, DNS, email, NIP-05, and admin logic.

### E2E Tests (Staging)

```bash
./tests/e2e_staging.sh
```

Runs end-to-end tests against `staging.noscha.io` — checks API endpoints, health, plans, NIP-05, and admin operations.

## LLM / AI Documentation

The following endpoints provide machine-readable documentation:

| Endpoint | Description |
|---|---|
| `/llms.txt` | Plain-text overview for LLM consumption |
| `/skill.md` | Skill description in Markdown |
| `/api/docs` | API documentation (human & machine readable) |
| `/api/info` | JSON service metadata |

## Project Structure

```
├── src/
│   ├── lib.rs          # Main router and request handlers
│   ├── types.rs        # Data types (Order, Rental, Plan, etc.)
│   ├── admin.rs        # Admin API and dashboard
│   ├── admin_ui.html   # Admin dashboard UI
│   ├── ui.rs           # Landing page renderer
│   ├── ui.html         # Landing page template
│   ├── coinos.rs       # coinos.io Lightning API client
│   ├── coinos_mock.rs  # Mock payment for dev/testing
│   ├── dns.rs          # Cloudflare DNS API client
│   ├── dns_mock.rs     # Mock DNS for dev/testing
│   ├── nip05.rs        # NIP-05 .well-known handler
│   ├── email.rs        # Email types
│   ├── email_shim.js   # Email routing handler (JS)
│   ├── resend.rs       # Resend email API
│   └── validation.rs   # Input validation
├── tests/
│   ├── e2e_staging.sh  # E2E tests for staging
│   └── e2e_test.sh     # E2E test utilities
├── deploy-staging.sh   # Deploy to staging
├── deploy-production.sh # Deploy to production
├── worker-entry.mjs    # Worker entry point (JS shim + auth gate)
├── wrangler.toml       # Cloudflare Workers config
├── Cargo.toml          # Rust dependencies
└── LICENSE             # MIT License
```

## License

[MIT](LICENSE) © kojira
