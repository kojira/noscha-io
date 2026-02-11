pub mod admin;
pub mod dns;
pub mod email;
pub mod nip05;
pub mod types;
pub mod ui;
pub mod validation;

#[cfg(target_arch = "wasm32")]
mod coinos;
#[cfg(target_arch = "wasm32")]
mod coinos_mock;
#[cfg(target_arch = "wasm32")]
mod dns_mock;

#[cfg(target_arch = "wasm32")]
use serde::Serialize;
#[cfg(target_arch = "wasm32")]
use worker::*;

#[cfg(target_arch = "wasm32")]
use admin::{
    handle_admin_ban, handle_admin_extend, handle_admin_page, handle_admin_rentals,
    handle_admin_revoke, handle_admin_stats, handle_admin_unban,
};
#[cfg(target_arch = "wasm32")]
use dns::DnsRecordType;
#[cfg(target_arch = "wasm32")]
use nip05::{handle_nip05, handle_nip05_options};
#[cfg(target_arch = "wasm32")]
use types::*;
#[cfg(target_arch = "wasm32")]
use validation::validate_username;

#[cfg(target_arch = "wasm32")]
#[derive(Serialize)]
struct ServiceInfo {
    name: &'static str,
    description: &'static str,
    version: &'static str,
    features: Vec<&'static str>,
    pricing: &'static str,
}

#[cfg(target_arch = "wasm32")]
#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

/// Generate a simple order ID using timestamp
#[cfg(target_arch = "wasm32")]
fn generate_order_id() -> String {
    let now = js_sys::Date::now() as u64;
    format!("ord_{:x}", now)
}

/// Generate a webhook secret for order verification
#[cfg(target_arch = "wasm32")]
fn generate_webhook_secret() -> String {
    let now = js_sys::Date::now() as u64;
    format!("sec_{:x}", now)
}

/// GET /api/check/{username}
#[cfg(target_arch = "wasm32")]
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

    // Check if username is banned
    if admin::is_banned(&bucket, username).await {
        return Response::from_json(&CheckUsernameResponse {
            available: false,
            username: username.to_string(),
            error: Some("This username is blocked".to_string()),
        });
    }

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
#[cfg(target_arch = "wasm32")]
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

    // Validate forward_to email if email service is requested
    if let Some(ref services) = body.services {
        if let Some(ref email_req) = services.email {
            if let Err(err) = validation::validate_forward_email(&email_req.forward_to) {
                return Response::error(format!("Invalid forwarding email: {}", err), 400);
            }
        }
    }

    // Check availability
    let bucket = ctx.env.bucket("BUCKET")?;

    // Check if username is banned
    if admin::is_banned(&bucket, &body.username).await {
        return Response::error("This username is blocked", 403);
    }

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
        services_requested: body.services,
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
#[cfg(target_arch = "wasm32")]
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

/// Provision DNS for a subdomain order
#[cfg(target_arch = "wasm32")]
async fn provision_dns(
    env: &Env,
    username: &str,
    subdomain_req: &OrderSubdomainRequest,
    expires: &str,
) -> Result<Option<String>> {
    let zone_id = env.var("CF_ZONE_ID")
        .map(|v| v.to_string())
        .unwrap_or_default();
    let domain = env.var("DOMAIN")
        .map(|v| v.to_string())
        .unwrap_or_else(|_| "noscha.io".to_string());

    if zone_id.is_empty() {
        console_log!("CF_ZONE_ID not set, skipping DNS provisioning");
        return Ok(None);
    }

    let record_type = match subdomain_req.record_type.to_uppercase().as_str() {
        "CNAME" => DnsRecordType::CNAME,
        "A" => DnsRecordType::A,
        "AAAA" => DnsRecordType::AAAA,
        other => {
            return Err(Error::RustError(format!(
                "Unsupported DNS record type: {}",
                other
            )))
        }
    };

    let is_mock = dns_mock::is_mock_dns_enabled(env);

    let record_id = if is_mock {
        dns_mock::create_dns_record(
            &zone_id,
            "",
            username,
            &record_type,
            &subdomain_req.target,
            subdomain_req.proxied,
            username,
            expires,
            &domain,
        )
        .await?
    } else {
        let token = env.secret("CF_API_TOKEN")?.to_string();
        dns::create_dns_record(
            &zone_id,
            &token,
            username,
            &record_type,
            &subdomain_req.target,
            subdomain_req.proxied,
            username,
            expires,
            &domain,
        )
        .await?
    };

    Ok(Some(record_id))
}

/// POST /api/webhook/coinos
#[cfg(target_arch = "wasm32")]
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

                    // Calculate rental expiry based on plan
                    let now_ms = js_sys::Date::now();
                    let duration_ms =
                        order.plan.duration_days() as f64 * 24.0 * 60.0 * 60.0 * 1000.0;
                    let rental_expires_ms = now_ms + duration_ms;
                    let rental_expires_date =
                        js_sys::Date::new(&(rental_expires_ms.into()));
                    let rental_expires_at = rental_expires_date
                        .to_iso_string()
                        .as_string()
                        .unwrap_or_default();
                    let now_date = js_sys::Date::new_0();
                    let now_iso = now_date
                        .to_iso_string()
                        .as_string()
                        .unwrap_or_default();

                    // Provision DNS if subdomain requested
                    let mut subdomain_service: Option<SubdomainService> = None;
                    if let Some(ref services) = order.services_requested {
                        if let Some(ref sub_req) = services.subdomain {
                            let record_id = provision_dns(
                                &ctx.env,
                                &order.username,
                                sub_req,
                                &rental_expires_at,
                            )
                            .await?;
                            subdomain_service = Some(SubdomainService {
                                enabled: true,
                                record_type: sub_req.record_type.clone(),
                                target: sub_req.target.clone(),
                                proxied: sub_req.proxied,
                                cf_record_id: record_id,
                            });
                        }
                    }

                    // Build rental object
                    let email_service =
                        order.services_requested.as_ref().and_then(|s| {
                            s.email.as_ref().map(|e| EmailService {
                                enabled: true,
                                forward_to: e.forward_to.clone(),
                                cf_rule_id: None,
                            })
                        });
                    let nip05_service =
                        order.services_requested.as_ref().and_then(|s| {
                            s.nip05.as_ref().map(|n| Nip05Service {
                                enabled: true,
                                pubkey_hex: n.pubkey.clone(),
                                relays: vec![],
                            })
                        });

                    let rental = Rental {
                        username: order.username.clone(),
                        status: "active".to_string(),
                        created_at: now_iso,
                        expires_at: rental_expires_at,
                        plan: order.plan.clone(),
                        services: RentalServices {
                            email: email_service,
                            subdomain: subdomain_service,
                            nip05: nip05_service,
                        },
                    };

                    // Save rental to R2
                    let rental_key =
                        format!("rentals/{}.json", order.username);
                    let rental_json = serde_json::to_string(&rental)
                        .map_err(|e| Error::RustError(e.to_string()))?;
                    bucket
                        .put(&rental_key, rental_json)
                        .execute()
                        .await?;

                    // Update order status
                    order.status = OrderStatus::Provisioned;
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

/// Cleanup expired DNS records by scanning R2 rentals
#[cfg(target_arch = "wasm32")]
async fn cleanup_expired_dns(env: &Env) -> Result<()> {
    let bucket = env.bucket("BUCKET")?;
    let now_ms = js_sys::Date::now();

    let list = bucket.list().prefix("rentals/").execute().await?;

    for obj_entry in list.objects() {
        let key = obj_entry.key();
        if let Some(obj) = bucket.get(&key).execute().await? {
            let body = obj.body().unwrap();
            let text = body.text().await?;
            if let Ok(mut rental) = serde_json::from_str::<Rental>(&text) {
                if rental.status != "active" {
                    continue;
                }

                // Parse expires_at as JS Date to compare
                let expires_date =
                    js_sys::Date::new(&rental.expires_at.clone().into());
                let expires_ms = expires_date.get_time();

                if expires_ms > now_ms {
                    continue; // not yet expired
                }

                console_log!(
                    "Cleaning up expired rental: {}",
                    rental.username
                );

                // Delete DNS record if present
                if let Some(ref sub) = rental.services.subdomain {
                    if let Some(ref record_id) = sub.cf_record_id {
                        let zone_id = env
                            .var("CF_ZONE_ID")
                            .map(|v| v.to_string())
                            .unwrap_or_default();

                        if !zone_id.is_empty() {
                            let is_mock = dns_mock::is_mock_dns_enabled(env);
                            let result = if is_mock {
                                dns_mock::delete_dns_record(
                                    &zone_id, "", record_id,
                                )
                                .await
                            } else {
                                let token =
                                    env.secret("CF_API_TOKEN")?.to_string();
                                dns::delete_dns_record(
                                    &zone_id, &token, record_id,
                                )
                                .await
                            };

                            if let Err(e) = result {
                                console_log!(
                                    "Failed to delete DNS record {} for {}: {:?}",
                                    record_id,
                                    rental.username,
                                    e
                                );
                            }
                        }
                    }
                }

                // Mark rental as expired
                rental.status = "expired".to_string();
                let updated_json = serde_json::to_string(&rental)
                    .map_err(|e| Error::RustError(e.to_string()))?;
                bucket.put(&key, updated_json).execute().await?;
            }
        }
    }

    Ok(())
}

#[cfg(target_arch = "wasm32")]
#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();

    Router::new()
        .get("/", |_, _| {
            Response::from_html(ui::landing_page_html())
        })
        .get("/api/info", |_, _| {
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
        .get_async("/.well-known/nostr.json", handle_nip05)
        .options_async("/.well-known/nostr.json", handle_nip05_options)
        // Admin routes
        .get_async("/admin", handle_admin_page)
        .get_async("/admin/rentals", handle_admin_rentals)
        .get_async("/admin/stats", handle_admin_stats)
        .post_async("/admin/ban/:username", handle_admin_ban)
        .post_async("/admin/unban/:username", handle_admin_unban)
        .post_async("/admin/extend/:username", handle_admin_extend)
        .post_async("/admin/revoke/:username", handle_admin_revoke)
        .run(req, env)
        .await
}

#[cfg(target_arch = "wasm32")]
#[event(scheduled)]
async fn scheduled(_event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    if let Err(e) = cleanup_expired_dns(&env).await {
        console_log!("Error during cleanup: {:?}", e);
    }
}
