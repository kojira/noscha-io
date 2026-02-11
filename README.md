# noscha.io

Lightning-powered disposable identity services for Nostr. Rent a username and get email forwarding, subdomain DNS, and NIP-05 verification — no KYC, no signup, instant activation.

## Features

- **Email Forwarding** — `username@noscha.io` forwards to your real email
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

3. Configure secrets (via Cloudflare dashboard or wrangler):
   ```bash
   wrangler secret put COINOS_API_TOKEN
   wrangler secret put WEBHOOK_SECRET
   wrangler secret put CF_API_TOKEN
   wrangler secret put CF_ZONE_ID
   wrangler secret put ADMIN_PUBKEY
   wrangler secret put RESEND_API_KEY
   ```

4. Configure `wrangler.toml` with your domain and R2 bucket.

## Development

```bash
# Run locally with wrangler dev
npx wrangler dev

# Build only
cargo install -q worker-build && worker-build --release
```

Set `MOCK_PAYMENT = "true"` in `[vars]` to skip real Lightning payments during development.

## Deploy

```bash
# Production
npx wrangler deploy

# Staging (with auth gate)
npx wrangler deploy --env staging
```

The staging environment requires NIP-07 authentication (controlled by `REQUIRE_AUTH = "true"`). Only the `ADMIN_PUBKEY` holder can access the staging site.

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
├── worker-entry.mjs    # Worker entry point (JS shim + auth gate)
├── wrangler.toml       # Cloudflare Workers config
├── Cargo.toml          # Rust dependencies
└── LICENSE             # MIT License
```

## License

[MIT](LICENSE) © kojira
