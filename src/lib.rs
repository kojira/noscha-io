use serde::Serialize;
use worker::*;

mod coinos;
mod coinos_mock;
mod types;

use types::*;

#[derive(Serialize)]
struct ServiceInfo {
    name: &'static str,
    description: &'static str,
    version: &'static str,
    features: Vec<&'static str>,
    pricing: &'static str,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

/// Reserved usernames that cannot be registered
const RESERVED_USERNAMES: &[&str] = &[
    "admin", "www", "mail", "api", "ns1", "ns2", "_dmarc", "autoconfig",
    "postmaster", "abuse", "hostmaster", "webmaster", "ftp", "smtp", "imap",
    "pop", "pop3", "root", "test", "localhost", "noscha",
];

/// Validate username: 3-20 chars, lowercase alphanumeric + hyphen, no leading/trailing hyphen
fn validate_username(username: &str) -> std::result::Result<(), String> {
    if username.len() < 3 {
        return Err("Username must be at least 3 characters".to_string());
    }
    if username.len() > 20 {
        return Err("Username must be at most 20 characters".to_string());
    }
    if username.starts_with('-') || username.ends_with('-') {
        return Err("Username cannot start or end with a hyphen".to_string());
    }
    if !username
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(
            "Username can only contain lowercase letters, digits, and hyphens".to_string(),
        );
    }
    if RESERVED_USERNAMES.contains(&username) {
        return Err("This username is reserved".to_string());
    }
    Ok(())
}

/// Generate a simple order ID using timestamp
fn generate_order_id() -> String {
    let now = js_sys::Date::now() as u64;
    format!("ord_{:x}", now)
}

/// Generate a webhook secret for order verification
fn generate_webhook_secret() -> String {
    let now = js_sys::Date::now() as u64;
    format!("sec_{:x}", now)
}

/// GET /api/check/{username}
async fn handle_check_username(
    _req: Request,
    ctx: RouteContext<()>,
) -> Result<Response> {
    let username = ctx.param("username").unwrap();

    if let Err(err) = validate_username(username) {
        return Response::from_json(&CheckUsernameResponse {
            available: false,
            username: username.to_string(),
            error: Some(err),
        });
    }

    // Check R2 for existing active rental
    let bucket = ctx.env.bucket("BUCKET")?;
    let key = format!("rentals/{}.json", username);
    let existing = bucket.get(&key).execute().await?;

    let available = existing.is_none();

    Response::from_json(&CheckUsernameResponse {
        available,
        username: username.to_string(),
        error: None,
    })
}

/// POST /api/order
async fn handle_create_order(
    mut req: Request,
    ctx: RouteContext<()>,
) -> Result<Response> {
    let body: OrderRequest = req.json().await.map_err(|_| {
        Error::RustError("Invalid request body".to_string())
    })?;

    // Validate username
    if let Err(err) = validate_username(&body.username) {
        return Response::error(err, 400);
    }

    // Check availability
    let bucket = ctx.env.bucket("BUCKET")?;
    let rental_key = format!("rentals/{}.json", body.username);
    if bucket.get(&rental_key).execute().await?.is_some() {
        return Response::error("Username is already taken", 409);
    }

    let order_id = generate_order_id();
    let amount_sats = body.plan.amount_sats();
    let webhook_secret = generate_webhook_secret();
    let domain = ctx
        .env
        .var("DOMAIN")
        .map(|v| v.to_string())
        .unwrap_or_else(|_| "noscha.io".to_string());
    let webhook_url = format!("https://{}/api/webhook/coinos", domain);

    // Create invoice (mock or real)
    let is_mock = coinos_mock::is_mock_enabled(&ctx.env);
    let invoice = if is_mock {
        coinos_mock::create_mock_invoice(amount_sats, &webhook_url, &webhook_secret).await?
    } else {
        let api_token = ctx.env.secret("COINOS_API_TOKEN")?.to_string();
        coinos::create_invoice(&api_token, amount_sats, &webhook_url, &webhook_secret).await?
    };

    // Calculate expiry (15 min for invoice)
    let now = js_sys::Date::now();
    let expires_ms = now + 15.0 * 60.0 * 1000.0;
    let created_at = js_sys::Date::new_0();
    let expires_at = js_sys::Date::new(&(expires_ms.into()));

    let order = Order {
        order_id: order_id.clone(),
        username: body.username.clone(),
        plan: body.plan,
        amount_sats,
        bolt11: invoice.text.clone(),
        status: if is_mock {
            OrderStatus::Paid
        } else {
            OrderStatus::Pending
        },
        created_at: created_at.to_iso_string().as_string().unwrap_or_default(),
        expires_at: expires_at.to_iso_string().as_string().unwrap_or_default(),
        coinos_invoice_hash: invoice.hash,
        webhook_secret: Some(webhook_secret),
    };

    // Save order to R2
    let order_key = format!("orders/{}.json", order_id);
    let order_json =
        serde_json::to_string(&order).map_err(|e| Error::RustError(e.to_string()))?;
    bucket.put(&order_key, order_json).execute().await?;

    Response::from_json(&OrderResponse {
        order_id: order.order_id,
        amount_sats: order.amount_sats,
        bolt11: order.bolt11,
        expires_at: order.expires_at,
    })
}

/// GET /api/order/{order_id}/status
async fn handle_order_status(
    _req: Request,
    ctx: RouteContext<()>,
) -> Result<Response> {
    let order_id = ctx.param("order_id").unwrap();

    let bucket = ctx.env.bucket("BUCKET")?;
    let key = format!("orders/{}.json", order_id);
    let obj = bucket.get(&key).execute().await?;

    match obj {
        Some(obj) => {
            let body = obj.body().unwrap();
            let text = body.text().await?;
            let order: Order = serde_json::from_str(&text)
                .map_err(|e| Error::RustError(e.to_string()))?;

            Response::from_json(&OrderStatusResponse {
                order_id: order.order_id,
                status: order.status,
            })
        }
        None => Response::error("Order not found", 404),
    }
}

/// POST /api/webhook/coinos
async fn handle_coinos_webhook(
    mut req: Request,
    ctx: RouteContext<()>,
) -> Result<Response> {
    let payload: CoinosWebhookPayload = req.json().await.map_err(|_| {
        Error::RustError("Invalid webhook payload".to_string())
    })?;

    // Must have confirmed = true
    if payload.confirmed != Some(true) {
        return Response::ok("ignored");
    }

    let secret = match &payload.secret {
        Some(s) => s.clone(),
        None => return Response::ok("no secret"),
    };

    // Find order by webhook secret — scan recent orders
    // In production, we'd use an index. For now, we rely on the hash field.
    let bucket = ctx.env.bucket("BUCKET")?;

    let hash = match &payload.hash {
        Some(h) => h.clone(),
        None => return Response::ok("no hash"),
    };

    // List orders to find the matching one
    let list = bucket.list().prefix("orders/").execute().await?;
    for obj_entry in list.objects() {
        let key = obj_entry.key();
        if let Some(obj) = bucket.get(&key).execute().await? {
            let body = obj.body().unwrap();
            let text = body.text().await?;
            if let Ok(mut order) = serde_json::from_str::<Order>(&text) {
                // Match by webhook secret and pending status
                if order.webhook_secret.as_deref() == Some(&secret)
                    && order.status == OrderStatus::Pending
                {
                    // Update order status to paid
                    order.status = OrderStatus::Paid;
                    order.coinos_invoice_hash = Some(hash.clone());

                    let updated_json = serde_json::to_string(&order)
                        .map_err(|e| Error::RustError(e.to_string()))?;
                    bucket.put(&key, updated_json).execute().await?;

                    return Response::ok("ok");
                }
            }
        }
    }

    Response::ok("no matching order")
}

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();

    Router::new()
        .get("/", |_, _| {
            let info = ServiceInfo {
                name: "noscha.io",
                description: "Lightning Network powered disposable email, subdomain & NIP-05 service. No KYC, no signup, instant activation.",
                version: "0.1.0",
                features: vec![
                    "Disposable email forwarding ({username}@noscha.io)",
                    "Subdomain provisioning ({username}.noscha.io)",
                    "NIP-05 Nostr identity verification",
                    "Lightning Network instant payments",
                    "1-day to 1-year rentals",
                ],
                pricing: "Starting from 10 sats/day",
            };
            Response::from_json(&info)
        })
        .get("/health", |_, _| {
            let health = HealthResponse { status: "ok" };
            Response::from_json(&health)
        })
        .get_async("/api/check/:username", handle_check_username)
        .post_async("/api/order", handle_create_order)
        .get_async("/api/order/:order_id/status", handle_order_status)
        .post_async("/api/webhook/coinos", handle_coinos_webhook)
        .run(req, env)
        .await
}
