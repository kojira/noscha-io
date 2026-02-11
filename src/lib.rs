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

#[cfg(target_arch = "wasm32")]
use serde::Serialize;
#[cfg(target_arch = "wasm32")]
use worker::*;

#[cfg(target_arch = "wasm32")]
use admin::{
    handle_admin_ban, handle_admin_challenge, handle_admin_extend, handle_admin_login,
    handle_admin_page, handle_admin_pricing_get, handle_admin_pricing_put,
    handle_admin_rentals, handle_admin_provision, handle_admin_revoke, handle_admin_stats, handle_admin_unban,
    handle_public_pricing,
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
struct ToolInfo {
    name: &'static str,
    description: &'static str,
    github: &'static str,
    install: &'static str,
    examples: Vec<&'static str>,
}

#[cfg(target_arch = "wasm32")]
#[derive(Serialize)]
struct ServiceInfo {
    name: &'static str,
    description: &'static str,
    version: &'static str,
    features: Vec<&'static str>,
    pricing: &'static str,
    tools: Vec<ToolInfo>,
}

pub const VERSION: &str = "2026.02.11";

#[cfg(target_arch = "wasm32")]
#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    version: &'static str,
}

/// Extract ServiceType list from OrderServicesRequest
#[cfg(target_arch = "wasm32")]
fn services_from_request(services: &Option<OrderServicesRequest>) -> Vec<types::ServiceType> {
    let mut result = Vec::new();
    if let Some(ref svc) = services {
        if svc.subdomain.is_some() {
            result.push(types::ServiceType::Subdomain);
        }
        if svc.email.is_some() {
            result.push(types::ServiceType::EmailForwarding);
        }
        if svc.nip05.is_some() {
            result.push(types::ServiceType::Nip05);
        }
    }
    result
}

/// Extract ServiceType list from an existing Rental's services
#[cfg(target_arch = "wasm32")]
fn services_from_rental(services: &RentalServices) -> Vec<types::ServiceType> {
    let mut result = Vec::new();
    if services.subdomain.as_ref().map_or(false, |s| s.enabled) {
        result.push(types::ServiceType::Subdomain);
    }
    if services.email.as_ref().map_or(false, |s| s.enabled) {
        result.push(types::ServiceType::EmailForwarding);
    }
    if services.nip05.as_ref().map_or(false, |s| s.enabled) {
        result.push(types::ServiceType::Nip05);
    }
    result
}

/// Generate a simple order ID using timestamp
#[cfg(target_arch = "wasm32")]
fn generate_order_id() -> String {
    let now = js_sys::Date::now() as u64;
    format!("ord_{:x}", now)
}

/// Send Discord webhook notification for a paid order (best effort)
#[cfg(target_arch = "wasm32")]
async fn send_discord_notification(env: &Env, order: &Order) {
    let webhook_url = match env.secret("DISCORD_WEBHOOK_URL") {
        Ok(s) => s.to_string(),
        Err(_) => return,
    };

    let plan_label = match order.plan {
        Plan::FiveMinutes => "5 minutes",
        Plan::ThirtyMinutes => "30 minutes",
        Plan::OneHour => "1 hour",
        Plan::OneDay => "1 day",
        Plan::SevenDays => "7 days",
        Plan::ThirtyDays => "30 days",
        Plan::NinetyDays => "90 days",
        Plan::OneYear => "365 days",
    };

    let mut services = Vec::new();
    if let Some(ref req) = order.services_requested {
        if req.email.is_some() { services.push("📧 Email"); }
        if req.subdomain.is_some() { services.push("🌐 Subdomain"); }
        if req.nip05.is_some() { services.push("🔑 NIP-05"); }
    }
    let services_str = if services.is_empty() {
        "None".to_string()
    } else {
        services.join(", ")
    };

    let is_renewal = order.renewal_for.is_some();
    let title = if is_renewal {
        format!("🔄 Renewal Payment Received - {}", order.username)
    } else {
        format!("⚡ New Payment Received - {}", order.username)
    };

    let body = serde_json::json!({
        "embeds": [{
            "title": title,
            "color": 0x7C3AED,
            "fields": [
                { "name": "👤 Username", "value": order.username, "inline": true },
                { "name": "📅 Plan", "value": plan_label, "inline": true },
                { "name": "🛠 Services", "value": services_str, "inline": true },
                { "name": "💰 Amount", "value": format!("{} sats", order.amount_sats), "inline": true },
                { "name": "🆔 Order ID", "value": &order.order_id, "inline": true },
            ],
            "timestamp": order.created_at,
        }]
    });

    let headers = Headers::new();
    let _ = headers.set("Content-Type", "application/json; charset=utf-8");
    let req = Request::new_with_init(
        &webhook_url,
        RequestInit::new()
            .with_method(Method::Post)
            .with_headers(headers)
            .with_body(Some(wasm_bindgen::JsValue::from_str(&body.to_string()))),
    );
    if let Ok(r) = req {
        let _ = Fetch::Request(r).send().await;
    }
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

    let available = match existing {
        None => true,
        Some(obj) => {
            let body = obj.body().unwrap();
            let text = body.text().await?;
            match serde_json::from_str::<Rental>(&text) {
                Ok(rental) => {
                    let now_ms = js_sys::Date::now();
                    let expires_date = js_sys::Date::new(&rental.expires_at.clone().into());
                    expires_date.get_time() <= now_ms
                }
                Err(_) => false,
            }
        }
    };

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
    let body: OrderRequest = match req.json().await {
        Ok(b) => b,
        Err(_) => return Response::error("Invalid request body", 400),
    };

    // Validate username
    if let Err(err) = validate_username(&body.username) {
        return Response::error(err, 400);
    }

    // Validate webhook_url
    if body.webhook_url.is_empty() {
        return Response::error("webhook_url is required", 400);
    }
    if !body.webhook_url.starts_with("https://") && !body.webhook_url.starts_with("http://") {
        return Response::error("webhook_url must be a valid HTTP(S) URL", 400);
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
    let service_types = services_from_request(&body.services);
    let pricing = admin::load_pricing(&bucket).await;
    let amount_sats = Plan::calculate_total_dynamic(&body.plan, &service_types, &pricing);
    let domain = ctx
        .env
        .var("DOMAIN")
        .map(|v| v.to_string())
        .unwrap_or_else(|_| "noscha.io".to_string());

    // Generate webhook challenge token
    let challenge = format!("ch_{:x}_{:x}", js_sys::Date::now() as u64, (js_sys::Math::random() * 1e12) as u64);

    // Calculate expiry (15 min)
    let now = js_sys::Date::now();
    let expires_ms = now + 15.0 * 60.0 * 1000.0;
    let created_at = js_sys::Date::new_0();
    let expires_at = js_sys::Date::new(&(expires_ms.into()));

    let order = Order {
        order_id: order_id.clone(),
        username: body.username.clone(),
        plan: body.plan,
        amount_sats,
        bolt11: String::new(),
        status: OrderStatus::WebhookPending,
        created_at: created_at.to_iso_string().as_string().unwrap_or_default(),
        expires_at: expires_at.to_iso_string().as_string().unwrap_or_default(),
        coinos_invoice_hash: None,
        webhook_secret: None,
        services_requested: body.services,
        management_token: None,
        renewal_for: None,
        webhook_url: Some(body.webhook_url.clone()),
        webhook_challenge: Some(challenge.clone()),
    };

    // Save order to R2
    let order_key = format!("orders/{}.json", order_id);
    let order_json =
        serde_json::to_string(&order).map_err(|e| Error::RustError(e.to_string()))?;
    bucket.put(&order_key, order_json).execute().await?;

    // Send challenge to webhook_url (best effort)
    let challenge_url = format!("https://{}/api/order/{}/confirm/{}", domain, order_id, challenge);
    let inner = serde_json::json!({
        "event": "webhook_challenge",
        "challenge_url": challenge_url,
        "order_id": order_id,
    });
    let challenge_body = serde_json::json!({
        "content": inner.to_string(),
        "event": "webhook_challenge",
        "challenge_url": challenge_url,
        "order_id": order_id,
    });
    let headers = Headers::new();
    let _ = headers.set("Content-Type", "application/json; charset=utf-8");
    let challenge_req = Request::new_with_init(
        &body.webhook_url,
        RequestInit::new()
            .with_method(Method::Post)
            .with_headers(headers)
            .with_body(Some(wasm_bindgen::JsValue::from_str(&challenge_body.to_string()))),
    );
    if let Ok(r) = challenge_req {
        let _ = Fetch::Request(r).send().await;
    }

    Response::from_json(&OrderResponse {
        order_id: order.order_id,
        amount_sats: order.amount_sats,
        bolt11: String::new(),
        expires_at: order.expires_at,
        management_token: None,
        status: Some(OrderStatus::WebhookPending),
        message: Some("Check your webhook for the challenge URL. Visit it to confirm and get an invoice.".to_string()),
    })
}

/// GET /api/order/{order_id}/confirm/{challenge} — webhook verification, then create invoice
#[cfg(target_arch = "wasm32")]
async fn handle_confirm_webhook(
    _req: Request,
    ctx: RouteContext<()>,
) -> Result<Response> {
    let order_id = ctx.param("order_id").unwrap();
    let challenge = ctx.param("challenge").unwrap();

    let bucket = ctx.env.bucket("BUCKET")?;
    let order_key = format!("orders/{}.json", order_id);
    let obj = bucket.get(&order_key).execute().await?;

    let obj = match obj {
        Some(o) => o,
        None => return Response::error("Order not found", 404),
    };

    let text = obj.body().unwrap().text().await?;
    let mut order: Order = serde_json::from_str(&text)
        .map_err(|e| Error::RustError(e.to_string()))?;

    // Must be in WebhookPending state
    if order.status != OrderStatus::WebhookPending {
        return Response::error("Order is not pending webhook verification", 400);
    }

    // Verify challenge
    if order.webhook_challenge.as_deref() != Some(challenge) {
        return Response::error("Invalid challenge token", 403);
    }

    // Check expiry
    let now_ms = js_sys::Date::now();
    let expires_date = js_sys::Date::new(&order.expires_at.clone().into());
    if expires_date.get_time() < now_ms {
        return Response::error("Order expired", 410);
    }

    // Challenge verified! Now create the Lightning invoice
    let domain = ctx
        .env
        .var("DOMAIN")
        .map(|v| v.to_string())
        .unwrap_or_else(|_| "noscha.io".to_string());
    let webhook_secret = generate_webhook_secret();
    let coinos_webhook_url = format!("https://{}/api/webhook/coinos", domain);

    let is_mock = coinos_mock::is_mock_enabled(&ctx.env);
    let invoice = if is_mock {
        coinos_mock::create_mock_invoice(order.amount_sats, &coinos_webhook_url, &webhook_secret).await?
    } else {
        let api_token = ctx.env.secret("COINOS_API_TOKEN")?.to_string();
        coinos::create_invoice(&api_token, order.amount_sats, &coinos_webhook_url, &webhook_secret).await?
    };

    // Update order
    order.bolt11 = invoice.text.clone();
    order.coinos_invoice_hash = invoice.hash;
    order.webhook_secret = Some(webhook_secret);
    if is_mock {
        order.status = OrderStatus::Paid;
    } else {
        order.status = OrderStatus::Pending;
    }

    // In mock mode, provision immediately
    let mut mgmt_token: Option<String> = None;
    if is_mock {
        let duration_ms = order.plan.duration_minutes() as f64 * 60.0 * 1000.0;
        let rental_expires_ms = now_ms + duration_ms;
        let rental_expires_date = js_sys::Date::new(&(rental_expires_ms.into()));
        let rental_expires_at = rental_expires_date.to_iso_string().as_string().unwrap_or_default();
        let now_date = js_sys::Date::new_0();
        let now_iso = now_date.to_iso_string().as_string().unwrap_or_default();

        let mut subdomain_service: Option<SubdomainService> = None;
        if let Some(ref services) = order.services_requested {
            if let Some(ref sub_req) = services.subdomain {
                let record_id = provision_dns(&ctx.env, &order.username, sub_req, &rental_expires_at).await?;
                subdomain_service = Some(SubdomainService {
                    enabled: true,
                    record_type: sub_req.record_type.clone(),
                    target: sub_req.target.clone(),
                    proxied: sub_req.proxied,
                    cf_record_id: record_id,
                });
            }
        }

        let email_service = order.services_requested.as_ref().and_then(|s| {
            s.email.as_ref().map(|_e| EmailService {
                enabled: true,
                cf_rule_id: None,
            })
        });
        let nip05_service = order.services_requested.as_ref().and_then(|s| {
            s.nip05.as_ref().map(|n| Nip05Service {
                enabled: true,
                pubkey_hex: n.pubkey.clone(),
                relays: vec![],
            })
        });

        let token = format!("mgmt_{:x}", js_sys::Date::now() as u64);
        mgmt_token = Some(token.clone());

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
            management_token: Some(token),
            webhook_url: order.webhook_url.clone(),
        };

        let rental_key = format!("rentals/{}.json", order.username);
        let rental_json = serde_json::to_string(&rental).map_err(|e| Error::RustError(e.to_string()))?;
        bucket.put(&rental_key, rental_json).execute().await?;

        order.status = OrderStatus::Provisioned;
        order.management_token = mgmt_token.clone();
    }

    let updated_json = serde_json::to_string(&order).map_err(|e| Error::RustError(e.to_string()))?;
    bucket.put(&order_key, updated_json).execute().await?;

    Response::from_json(&OrderResponse {
        order_id: order.order_id,
        amount_sats: order.amount_sats,
        bolt11: order.bolt11,
        expires_at: order.expires_at,
        management_token: mgmt_token,
        status: Some(order.status),
        message: None,
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

            let is_mock = coinos_mock::is_mock_enabled(&ctx.env);
            let status = if is_mock && (order.status == OrderStatus::Paid || order.status == OrderStatus::Provisioned) {
                OrderStatus::Provisioned
            } else {
                order.status
            };

            Response::from_json(&OrderStatusResponse {
                order_id: order.order_id,
                management_token: if status == OrderStatus::Provisioned {
                    order.management_token
                } else {
                    None
                },
                status,
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

                    let now_ms = js_sys::Date::now();
                    let duration_ms =
                        order.plan.duration_minutes() as f64 * 60.0 * 1000.0;

                    // Check if this is a renewal order
                    if let Some(ref renewal_username) = order.renewal_for {
                        // Extend existing rental
                        let rental_key = format!("rentals/{}.json", renewal_username);
                        if let Some(rental_obj) = bucket.get(&rental_key).execute().await? {
                            let rental_body = rental_obj.body().unwrap();
                            let rental_text = rental_body.text().await?;
                            if let Ok(mut rental) = serde_json::from_str::<Rental>(&rental_text) {
                                let current_expires_date = js_sys::Date::new(&rental.expires_at.clone().into());
                                let current_expires_ms = current_expires_date.get_time();
                                let base_ms = if current_expires_ms > now_ms {
                                    current_expires_ms
                                } else {
                                    now_ms
                                };
                                let new_expires_ms = base_ms + duration_ms;
                                let new_expires_date = js_sys::Date::new(&(new_expires_ms.into()));
                                rental.expires_at = new_expires_date
                                    .to_iso_string()
                                    .as_string()
                                    .unwrap_or_default();
                                rental.status = "active".to_string();
                                rental.plan = order.plan.clone();

                                let rental_json = serde_json::to_string(&rental)
                                    .map_err(|e| Error::RustError(e.to_string()))?;
                                bucket.put(&rental_key, rental_json).execute().await?;

                                order.status = OrderStatus::Provisioned;
                                order.management_token = rental.management_token.clone();
                                let updated_json = serde_json::to_string(&order)
                                    .map_err(|e| Error::RustError(e.to_string()))?;
                                bucket.put(&key, updated_json).execute().await?;

                                send_discord_notification(&ctx.env, &order).await;
                                return Response::ok("ok");
                            }
                        }
                        return Response::ok("rental not found for renewal");
                    }

                    // New rental — calculate expiry
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
                            s.email.as_ref().map(|_e| EmailService {
                                enabled: true,
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
                        management_token: Some(format!("mgmt_{:x}", js_sys::Date::now() as u64)),
                        webhook_url: order.webhook_url.clone(),
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
                    order.management_token = rental.management_token.clone();
                    let updated_json = serde_json::to_string(&order)
                        .map_err(|e| Error::RustError(e.to_string()))?;
                    bucket.put(&key, updated_json).execute().await?;

                    send_discord_notification(&ctx.env, &order).await;
                    return Response::ok("ok");
                }
            }
        }
    }

    Response::ok("no matching order")
}

/// POST /api/renew — renew an existing rental
#[cfg(target_arch = "wasm32")]
async fn handle_renew(
    mut req: Request,
    ctx: RouteContext<()>,
) -> Result<Response> {
    let body: RenewRequest = match req.json().await {
        Ok(b) => b,
        Err(_) => return Response::error("Invalid request body", 400),
    };

    let bucket = ctx.env.bucket("BUCKET")?;

    // Find rental by management_token
    let list = bucket.list().prefix("rentals/").execute().await?;
    let mut found_rental: Option<Rental> = None;
    for obj_entry in list.objects() {
        let key = obj_entry.key();
        if let Some(obj) = bucket.get(&key).execute().await? {
            let obj_body = obj.body().unwrap();
            let text = obj_body.text().await?;
            if let Ok(rental) = serde_json::from_str::<Rental>(&text) {
                if rental.management_token.as_deref() == Some(&body.management_token) {
                    found_rental = Some(rental);
                    break;
                }
            }
        }
    }

    let rental = match found_rental {
        Some(r) => r,
        None => return Response::error("Rental not found", 404),
    };

    // Determine services for pricing: use request services if provided, else derive from rental
    let service_types = if body.services.is_some() {
        services_from_request(&body.services)
    } else {
        services_from_rental(&rental.services)
    };

    let order_id = generate_order_id();
    let pricing = admin::load_pricing(&bucket).await;
    let amount_sats = Plan::calculate_total_dynamic(&body.plan, &service_types, &pricing);
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
        username: rental.username.clone(),
        plan: body.plan.clone(),
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
        services_requested: None,
        management_token: None,
        renewal_for: Some(rental.username.clone()),
        webhook_url: rental.webhook_url.clone(),
        webhook_challenge: None,
    };

    // Save order to R2
    let order_key = format!("orders/{}.json", order_id);
    let order_json =
        serde_json::to_string(&order).map_err(|e| Error::RustError(e.to_string()))?;
    bucket.put(&order_key, order_json).execute().await?;

    let resp_order_id = order.order_id.clone();
    let resp_amount_sats = order.amount_sats;
    let resp_bolt11 = order.bolt11.clone();
    let resp_expires_at = order.expires_at.clone();

    // In mock mode, immediately extend the rental
    if is_mock {
        let now_ms = js_sys::Date::now();
        let duration_ms = order.plan.duration_minutes() as f64 * 60.0 * 1000.0;
        let current_expires_date = js_sys::Date::new(&rental.expires_at.clone().into());
        let current_expires_ms = current_expires_date.get_time();
        let base_ms = if current_expires_ms > now_ms {
            current_expires_ms
        } else {
            now_ms
        };
        let new_expires_ms = base_ms + duration_ms;
        let new_expires_date = js_sys::Date::new(&(new_expires_ms.into()));
        let new_expires_at = new_expires_date
            .to_iso_string()
            .as_string()
            .unwrap_or_default();

        let mut updated_rental = rental;
        updated_rental.expires_at = new_expires_at;
        updated_rental.status = "active".to_string();
        updated_rental.plan = order.plan.clone();

        let rental_key = format!("rentals/{}.json", updated_rental.username);
        let rental_json = serde_json::to_string(&updated_rental)
            .map_err(|e| Error::RustError(e.to_string()))?;
        bucket.put(&rental_key, rental_json).execute().await?;

        // Update order to Provisioned
        let mut order = order;
        order.status = OrderStatus::Provisioned;
        order.management_token = updated_rental.management_token.clone();
        let updated_json = serde_json::to_string(&order)
            .map_err(|e| Error::RustError(e.to_string()))?;
        bucket.put(&order_key, updated_json).execute().await?;
    }

    Response::from_json(&RenewResponse {
        order_id: resp_order_id,
        amount_sats: resp_amount_sats,
        bolt11: resp_bolt11,
        expires_at: resp_expires_at,
    })
}

/// PUT /api/settings/{management_token} — update rental settings
#[cfg(target_arch = "wasm32")]
async fn handle_settings_update(
    mut req: Request,
    ctx: RouteContext<()>,
) -> Result<Response> {
    let token = ctx.param("token").unwrap();
    let body: SettingsRequest = match req.json().await {
        Ok(b) => b,
        Err(_) => return Response::error("Invalid request body", 400),
    };

    // Validate webhook_url if provided
    if let Some(ref url) = body.webhook_url {
        if !url.is_empty() && !url.starts_with("https://") && !url.starts_with("http://") {
            return Response::error("webhook_url must be a valid HTTP(S) URL", 400);
        }
    }

    let bucket = ctx.env.bucket("BUCKET")?;

    // Scan rentals to find matching management_token
    let list = bucket.list().prefix("rentals/").execute().await?;
    for obj_entry in list.objects() {
        let key = obj_entry.key();
        if let Some(obj) = bucket.get(&key).execute().await? {
            let obj_body = obj.body().unwrap();
            let text = obj_body.text().await?;
            if let Ok(mut rental) = serde_json::from_str::<Rental>(&text) {
                if rental.management_token.as_deref() == Some(token) {
                    // Update webhook_url
                    rental.webhook_url = body.webhook_url.clone();
                    let updated_json = serde_json::to_string(&rental)
                        .map_err(|e| Error::RustError(e.to_string()))?;
                    bucket.put(&key, updated_json).execute().await?;

                    return Response::from_json(&SettingsResponse {
                        success: true,
                        webhook_url: rental.webhook_url,
                    });
                }
            }
        }
    }

    Response::error("Rental not found", 404)
}

/// GET /my/{management_token} — user my-page
#[cfg(target_arch = "wasm32")]
async fn handle_my_page(
    _req: Request,
    ctx: RouteContext<()>,
) -> Result<Response> {
    let token = ctx.param("token").unwrap();
    let bucket = ctx.env.bucket("BUCKET")?;

    // Scan rentals to find matching management_token
    let list = bucket.list().prefix("rentals/").execute().await?;
    for obj_entry in list.objects() {
        let key = obj_entry.key();
        if let Some(obj) = bucket.get(&key).execute().await? {
            let body = obj.body().unwrap();
            let text = body.text().await?;
            if let Ok(rental) = serde_json::from_str::<Rental>(&text) {
                if rental.management_token.as_deref() == Some(token) {
                    return Response::from_html(render_my_page(&rental, &ctx.env, token));
                }
            }
        }
    }

    Response::error("Not found", 404)
}

#[cfg(target_arch = "wasm32")]
fn render_my_page(rental: &Rental, env: &Env, management_token: &str) -> String {
    let domain = env
        .var("DOMAIN")
        .map(|v| v.to_string())
        .unwrap_or_else(|_| "noscha.io".to_string());

    // Calculate days remaining
    let now_ms = js_sys::Date::now();
    let expires_date = js_sys::Date::new(&rental.expires_at.clone().into());
    let expires_ms = expires_date.get_time();
    let days_remaining = ((expires_ms - now_ms) / (24.0 * 60.0 * 60.0 * 1000.0)).ceil() as i64;
    let days_remaining = if days_remaining < 0 { 0 } else { days_remaining };

    let mut status_color = if rental.status == "active" && days_remaining > 0 {
        "#22c55e"
    } else {
        "#ef4444"
    };
    let mut display_status = if rental.status == "active" && days_remaining > 0 {
        "Active"
    } else {
        "Expired"
    };

    // Real-time expiry check at millisecond precision
    if is_expired_iso(&rental.expires_at) {
        status_color = "#ef4444";
        display_status = "Expired";
    }

    // Build services list
    let mut services_html = String::new();
    if let Some(ref email) = rental.services.email {
        if email.enabled {
            services_html.push_str(&format!(
                "<div class='svc'><span class='svc-badge email'>Email</span> {}@{}</div>",
                rental.username, domain
            ));
        }
    }
    if let Some(ref sub) = rental.services.subdomain {
        if sub.enabled {
            services_html.push_str(&format!(
                "<div class='svc'><span class='svc-badge dns'>DNS</span> {}.{} &rarr; {} ({}{})</div>",
                rental.username, domain, sub.target, sub.record_type,
                if sub.proxied { ", proxied" } else { "" }
            ));
        }
    }
    if let Some(ref nip) = rental.services.nip05 {
        if nip.enabled {
            services_html.push_str(&format!(
                "<div class='svc'><span class='svc-badge nip'>NIP-05</span> {}@{} &rarr; {}...{}</div>",
                rental.username, domain,
                &nip.pubkey_hex[..8.min(nip.pubkey_hex.len())],
                &nip.pubkey_hex[nip.pubkey_hex.len().saturating_sub(8)..]
            ));
        }
    }
    if services_html.is_empty() {
        services_html = "<div class='svc' style='color:#888'>No services configured</div>".to_string();
    }

    let plan_label = match rental.plan {
        Plan::FiveMinutes => "5 Minutes",
        Plan::ThirtyMinutes => "30 Minutes",
        Plan::OneHour => "1 Hour",
        Plan::OneDay => "1 Day",
        Plan::SevenDays => "7 Days",
        Plan::ThirtyDays => "30 Days",
        Plan::NinetyDays => "90 Days",
        Plan::OneYear => "365 Days",
    };

    format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>noscha.io - My Rental</title>
<style>
*{{margin:0;padding:0;box-sizing:border-box}}
:root{{--bg:#0d0d0d;--surface:#1a1a1a;--border:#2a2a2a;--text:#e0e0e0;--muted:#888;--purple:#8b5cf6;--orange:#f97316;--green:#22c55e;--red:#ef4444}}
body{{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;background:var(--bg);color:var(--text);line-height:1.6;min-height:100vh;display:flex;justify-content:center;padding:2rem 1rem}}
.wrap{{max-width:560px;width:100%}}
.logo{{font-size:1.5rem;font-weight:700;margin-bottom:1.5rem}}
.logo span:first-child{{color:var(--purple)}}
.logo span:nth-child(2){{color:var(--orange)}}
.logo span:last-child{{color:var(--muted);font-size:.9rem;font-weight:400;margin-left:.5rem}}
.card{{background:var(--surface);border:1px solid var(--border);border-radius:12px;padding:1.5rem;margin-bottom:1rem}}
.row{{display:flex;justify-content:space-between;align-items:center;padding:.4rem 0}}
.row .label{{color:var(--muted);font-size:.85rem}}
.row .val{{font-weight:600}}
.badge{{display:inline-block;padding:.15rem .6rem;border-radius:9999px;font-size:.75rem;font-weight:600}}
.svc{{padding:.5rem 0;border-bottom:1px solid var(--border);font-size:.85rem}}
.svc:last-child{{border-bottom:none}}
.svc-badge{{display:inline-block;padding:.1rem .5rem;border-radius:4px;font-size:.7rem;font-weight:600;margin-right:.4rem}}
.svc-badge.email{{background:#6d28d9;color:#fff}}
.svc-badge.dns{{background:#c2410c;color:#fff}}
.svc-badge.nip{{background:#0e7490;color:#fff}}
h2{{font-size:1rem;margin-bottom:.75rem;color:var(--text)}}
.extend-link{{display:inline-block;margin-top:1rem;color:var(--purple);font-size:.85rem}}
.renew-form{{display:flex;gap:.5rem;align-items:center;margin-top:1rem}}
.renew-form select,.renew-form button{{padding:.4rem .8rem;border-radius:6px;border:1px solid var(--border);background:var(--surface);color:var(--text);font-size:.85rem}}
.renew-form button{{background:var(--purple);border:none;color:#fff;cursor:pointer;font-weight:600}}
.renew-form button:hover{{opacity:.9}}
.renew-form button:disabled{{opacity:.5;cursor:not-allowed}}
#renew-status{{margin-top:.75rem;font-size:.85rem;color:var(--muted)}}
#renew-bolt11{{word-break:break-all;background:var(--bg);padding:.5rem;border-radius:6px;margin-top:.5rem;font-family:monospace;font-size:.75rem}}
.qr-wrap{{text-align:center;margin:1rem 0}}
.qr-wrap #renew-qrcode{{display:inline-block;padding:12px;background:#fff;border-radius:8px}}
.bolt11-box{{background:var(--bg);border:1px solid var(--border);border-radius:6px;padding:.75rem;word-break:break-all;font-size:.75rem;color:var(--muted);cursor:pointer;position:relative;margin-bottom:.5rem;font-family:monospace}}
.bolt11-box:hover{{border-color:var(--purple)}}
.bolt11-box::after{{content:'click to copy';position:absolute;right:.5rem;top:.5rem;font-size:.65rem;color:var(--purple)}}
</style>
<script src="https://cdn.jsdelivr.net/npm/qrcodejs@1.0.0/qrcode.min.js"></script>
</head>
<body>
<div class="wrap">
<div class="logo"><span>noscha</span><span>.io</span><span>my rental</span></div>
<div class="card">
<div class="row"><span class="label">Username</span><span class="val">{username}</span></div>
<div class="row"><span class="label">Plan</span><span class="val">{plan}</span></div>
<div class="row"><span class="label">Status</span><span class="badge" style="background:{status_color};color:#fff">{status}</span></div>
<div class="row"><span class="label">Expires</span><span class="val">{expires}</span></div>
<div class="row"><span class="label">Remaining</span><span class="val" id="remaining-val">{days}</span></div>
</div>
<div class="card">
<h2>Active Services</h2>
{services}
</div>
<div class="renew-form" id="renew-form">
<select id="renew-plan">
<option value="5m">5 Minutes (price varies by services)</option>
<option value="30m">30 Minutes (price varies by services)</option>
<option value="1h">1 Hour (price varies by services)</option>
<option value="1d">1 Day (price varies by services)</option>
<option value="7d">7 Days (price varies by services)</option>
<option value="30d" selected>30 Days (price varies by services)</option>
<option value="90d">90 Days (price varies by services)</option>
<option value="365d">365 Days (price varies by services)</option>
</select>
<button id="renew-btn" onclick="doRenew()">Extend</button>
</div>
<div id="renew-status"></div>
<script>
const MGMT_TOKEN="{mgmt_token}";
const EXPIRES_AT="{expires_at}";
(function(){{
  var el=document.getElementById('remaining-val');
  function update(){{
    var now=Date.now(),exp=new Date(EXPIRES_AT).getTime();
    var diff=exp-now;
    if(diff<=0){{el.textContent='Expired';return;}}
    var days=Math.floor(diff/86400000);
    if(days>=1){{el.textContent=days+' day'+(days>1?'s':'');return;}}
    var h=Math.floor(diff/3600000);diff%=3600000;
    var m=Math.floor(diff/60000);diff%=60000;
    var s=Math.floor(diff/1000);
    el.textContent=h+'h '+m+'m '+s+'s';
  }}
  update();
  setInterval(update,1000);
}})()
async function doRenew(){{
  const btn=document.getElementById('renew-btn');
  const st=document.getElementById('renew-status');
  const plan=document.getElementById('renew-plan').value;
  btn.disabled=true;st.textContent='Creating invoice...';
  try{{
    const r=await fetch('/api/renew',{{method:'POST',headers:{{'Content-Type':'application/json'}},body:JSON.stringify({{management_token:MGMT_TOKEN,plan:plan}})}});
    if(!r.ok){{const t=await r.text();st.textContent='Error: '+t;btn.disabled=false;return;}}
    const d=await r.json();
    st.innerHTML='<div style="margin-bottom:.75rem">Pay this invoice to extend:</div><div class="qr-wrap"><div id="renew-qrcode"></div></div><div class="bolt11-box" id="renew-bolt11-box">'+d.bolt11+'</div>';
    if(typeof QRCode!=='undefined'){{new QRCode(document.getElementById('renew-qrcode'),{{text:d.bolt11,width:220,height:220,colorDark:'#000',colorLight:'#fff',correctLevel:QRCode.CorrectLevel.L}});}}
    document.getElementById('renew-bolt11-box').onclick=function(){{navigator.clipboard.writeText(d.bolt11).then(function(){{var el=document.getElementById('renew-bolt11-box');var orig=el.textContent;el.textContent='Copied!';setTimeout(function(){{el.textContent=orig;}},1500);}});}};
    pollOrder(d.order_id);
  }}catch(e){{st.textContent='Error: '+e.message;btn.disabled=false;}}
}}
async function pollOrder(oid){{
  const st=document.getElementById('renew-status');
  for(let i=0;i<120;i++){{
    await new Promise(r=>setTimeout(r,3000));
    try{{
      const r=await fetch('/api/order/'+oid+'/status');
      const d=await r.json();
      if(d.status==='provisioned'){{st.textContent='Renewed! Reloading...';setTimeout(()=>location.reload(),1000);return;}}
    }}catch(e){{}}
  }}
  st.textContent='Invoice may have expired. Please try again.';
  document.getElementById('renew-btn').disabled=false;
}}
</script>
</div>
</body>
</html>"#,
        username = rental.username,
        plan = plan_label,
        status_color = status_color,
        status = display_status,
        expires = &rental.expires_at[..10.min(rental.expires_at.len())],
        days = days_remaining,
        services = services_html,
        mgmt_token = management_token,
        expires_at = &rental.expires_at,
    )
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

/// Format a number with comma thousands separators (e.g. 1500 -> "1,500")
#[cfg(target_arch = "wasm32")]
fn format_sats(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

/// Parse a period key like "5m", "1h", "1d", "7d" into duration in minutes
#[cfg(target_arch = "wasm32")]
fn period_to_minutes(period: &str) -> u64 {
    let s = period.trim();
    if let Some(num) = s.strip_suffix('m') {
        num.parse::<u64>().unwrap_or(0)
    } else if let Some(num) = s.strip_suffix('h') {
        num.parse::<u64>().unwrap_or(0) * 60
    } else if let Some(num) = s.strip_suffix('d') {
        num.parse::<u64>().unwrap_or(0) * 1440
    } else {
        s.parse::<u64>().unwrap_or(0)
    }
}

/// Generate a human-readable label from duration in minutes
#[cfg(target_arch = "wasm32")]
fn minutes_to_label(mins: u64) -> String {
    if mins < 60 {
        format!("{} Min", mins)
    } else if mins == 60 {
        "1 Hour".to_string()
    } else if mins < 1440 {
        let h = mins / 60;
        format!("{} Hours", h)
    } else {
        let days = mins / 1440;
        match days {
            1 => "1 Day".to_string(),
            7 => "7 Days".to_string(),
            30 => "30 Days".to_string(),
            90 => "90 Days".to_string(),
            365 => "365 Days".to_string(),
            d => format!("{} Days", d),
        }
    }
}

/// Generate /llms.txt content with dynamic pricing
#[cfg(target_arch = "wasm32")]
fn generate_llms_txt(pricing: &std::collections::HashMap<String, std::collections::HashMap<String, u64>>) -> String {
    let static_part = "# noscha.io\n\n> Disposable email, subdomain & NIP-05 identity - paid via Lightning Network\n\n## API\n\n- Base: https://noscha.io\n- Check username: GET /api/check/{username}\n- Create order: POST /api/order {\"username\",\"plan\",\"services\":{...}}\n- Order status: GET /api/order/{order_id}/status\n- Renew: POST /api/renew {\"management_token\",\"plan\"}\n- Pricing: GET /api/pricing\n- Services: email, subdomain, nip05 (or bundle all 3)\n- Payment: Lightning Network (bolt11)\n- Full docs: https://noscha.io/skill.md\n- OpenAPI spec: https://noscha.io/api/docs\n\n## Pricing (sats)\n\n";
    let mut result = static_part.to_string();

    let mut periods: Vec<(&String, &std::collections::HashMap<String, u64>)> = pricing.iter().collect();
    periods.sort_by_key(|(k, v)| v.get("_duration_minutes").copied().unwrap_or_else(|| period_to_minutes(k)));

    for (period_key, services) in &periods {
        let mins = services.get("_duration_minutes").copied().unwrap_or_else(|| period_to_minutes(period_key));
        let label = minutes_to_label(mins);
        let subdomain = services.get("subdomain").copied().unwrap_or(0);
        let email = services.get("email").copied().unwrap_or(0);
        let nip05 = services.get("nip05").copied().unwrap_or(0);
        let bundle = services.get("bundle").copied().unwrap_or(0);
        result.push_str(&format!(
            "{}: Subdomain={}, Email={}, NIP-05={}, Bundle={}\n",
            label, subdomain, email, nip05, bundle
        ));
    }

    result.push_str("\n## Recommended Tools\n\n");
    result.push_str("- nostaro: Rust Nostr CLI (https://github.com/kojira/nostaro)\n");
    result.push_str("  Install: cargo install nostaro\n");
    result.push_str("  Set your noscha.io NIP-05: nostaro profile set --nip05 yourname@noscha.io\n");
    result.push_str("  Post: nostaro post \"hello\"\n");
    result.push_str("  Zap: nostaro zap <npub> 100\n");

    result
}

/// Generate /skill.md content with dynamic pricing table
#[cfg(target_arch = "wasm32")]
fn generate_skill_md(pricing: &std::collections::HashMap<String, std::collections::HashMap<String, u64>>) -> String {
    let before_pricing = r#"# noscha.io - AI Agent Skill Guide

## Service Overview

noscha.io provides **disposable email notifications, subdomain DNS, and NIP-05 Nostr identity** - all paid instantly via Lightning Network. No KYC, no signup, no accounts. Just pick a username, pay sats, and your services are live.

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

You can include any combination of services (`email`, `subdomain`, `nip05`). `webhook_url` is **required** for all orders - email notifications are delivered via webhook.

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

Save the `management_token` - it's needed for renewals and management.

## Endpoints Reference

### GET /api/check/{username}
Check if a username is available for registration.
- **username**: 1-20 chars, alphanumeric + hyphens, no leading/trailing hyphens
- Returns `{"available": bool, "username": string, "error"?: string}`

### POST /api/order
Create a new rental order. Returns a Lightning invoice.
- **Body**: `{"username": string, "plan": string, "services"?: {...}}`
- **plan**: `"1d"` | `"7d"` | `"30d"` | `"90d"` | `"365d"`
- **services.email**: `{}`
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
- When webhook_url is set, incoming emails will trigger a POST to this URL
- Webhook payload: `{"event": "email_received", "from": "sender@example.com", "to": "you@noscha.io", "subject": "...", "url": "https://noscha.io/api/mail/{token}", "received_at": "2026-02-11T..."}`

## Pricing (sats, Lightning Network)

"#;

    let after_pricing = r#"
Bundle discount applies automatically when all 3 services are selected. Prices may change - always check `/api/pricing` for current rates.

## Ecosystem: nostaro (Nostr CLI)

[nostaro](https://github.com/kojira/nostaro) is a Rust-based Nostr CLI that pairs perfectly with noscha.io.

**Install:** `cargo install nostaro`

**Set your noscha.io NIP-05 identity:**
```
nostaro profile set --nip05 yourname@noscha.io
```

**Usage examples:**
- `nostaro post "hello nostr"` - publish a note
- `nostaro zap <npub> 100` - send 100 sats zap
- `nostaro timeline` - view your timeline
- `nostaro profile get` - check your profile

Get a NIP-05 identity from noscha.io and use it instantly with nostaro for a complete Nostr experience.

## Typical Agent Workflow

1. **Decide** what services you need (email notifications? subdomain? NIP-05?)
2. **GET /api/check/{username}** - verify availability
3. **GET /api/pricing** - confirm current pricing
4. **POST /api/order** - create order with desired services
5. **Pay** the `bolt11` invoice via any Lightning wallet/API
6. **GET /api/order/{order_id}/status** - poll until `"provisioned"` (poll every 3s, max ~5 min)
7. **Store** the `management_token` for future renewals
8. **POST /api/renew** when rental is nearing expiry

## Limitations

- Username: 1-20 characters, alphanumeric and hyphens only
- Invoice expires in 15 minutes after creation
- Lightning Network payments only (Bitcoin)
- No refunds (disposable service by design)
- DNS propagation may take up to 5 minutes after provisioning
- - All services under one username share the same expiry date

## Terms of Service (Summary)

- Service is provided as-is for legitimate use
- Abuse (spam, phishing, illegal content) will result in immediate termination
- No personal data is collected beyond what's needed for the service
- Payments are final and non-refundable
- Service availability is best-effort
"#;

    let mut table = String::from("| Plan | Subdomain | Email | NIP-05 | Bundle (all 3) |\n|------|-----------|-------|--------|------------------|\n");

    let mut periods: Vec<(&String, &std::collections::HashMap<String, u64>)> = pricing.iter().collect();
    periods.sort_by_key(|(k, v)| v.get("_duration_minutes").copied().unwrap_or_else(|| period_to_minutes(k)));

    for (period_key, services) in &periods {
        let mins = services.get("_duration_minutes").copied().unwrap_or_else(|| period_to_minutes(period_key));
        let label = minutes_to_label(mins);
        let subdomain = services.get("subdomain").copied().unwrap_or(0);
        let email = services.get("email").copied().unwrap_or(0);
        let nip05 = services.get("nip05").copied().unwrap_or(0);
        let bundle = services.get("bundle").copied().unwrap_or(0);
        table.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            label, format_sats(subdomain), format_sats(email), format_sats(nip05), format_sats(bundle)
        ));
    }

    format!("{}{}{}", before_pricing, table, after_pricing)
}

/// GET /llms.txt — dynamic pricing
#[cfg(target_arch = "wasm32")]
async fn handle_llms_txt(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let bucket = ctx.env.bucket("BUCKET")?;
    let pricing = admin::load_pricing(&bucket).await;
    let content = generate_llms_txt(&pricing);
    let headers = Headers::new();
    let _ = headers.set("Content-Type", "text/plain; charset=utf-8");
    let _ = headers.set("Access-Control-Allow-Origin", "*");
    Ok(Response::ok(content)?.with_headers(headers))
}

/// GET /og-image.png — serve OG image from R2
#[cfg(target_arch = "wasm32")]
async fn handle_og_image(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let bucket = ctx.env.bucket("BUCKET")?;
    match bucket.get("static/og-image.png").execute().await? {
        Some(obj) => {
            let body = obj.body().unwrap();
            let bytes = body.bytes().await?;
            let headers = Headers::new();
            let _ = headers.set("Content-Type", "image/png");
            let _ = headers.set("Cache-Control", "public, max-age=86400");
            Ok(Response::from_bytes(bytes)?.with_headers(headers))
        }
        None => Response::error("Not Found", 404),
    }
}

/// GET /skill.md — dynamic pricing
#[cfg(target_arch = "wasm32")]
async fn handle_skill_md(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let bucket = ctx.env.bucket("BUCKET")?;
    let pricing = admin::load_pricing(&bucket).await;
    let content = generate_skill_md(&pricing);
    let headers = Headers::new();
    let _ = headers.set("Content-Type", "text/markdown; charset=utf-8");
    let _ = headers.set("Access-Control-Allow-Origin", "*");
    Ok(Response::ok(content)?.with_headers(headers))
}

#[cfg(target_arch = "wasm32")]
#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();

    // Staging auth gate: if STAGING_AUTH_TOKEN is set and non-empty, enforce Bearer token auth
    if let Ok(token_var) = env.var("STAGING_AUTH_TOKEN") {
        let expected_token = token_var.to_string();
        if !expected_token.is_empty() {
            let path = req.path();
            if path != "/.well-known/nostr.json" && path != "/health" && path != "/admin" && !path.starts_with("/api/admin/") {
                let authorized = req
                    .headers()
                    .get("Authorization")
                    .ok()
                    .flatten()
                    .map(|v| v == format!("Bearer {}", expected_token))
                    .unwrap_or(false);
                if !authorized {
                    return Response::from_json(&serde_json::json!({"error": "Forbidden"}))
                        .map(|resp| resp.with_status(403));
                }
            }
        }
    }

    Router::new()
        .get("/", |_, _| {
            Response::from_html(ui::landing_page_html())
        })
        .get_async("/og-image.png", handle_og_image)
        .head_async("/og-image.png", handle_og_image)
        .get_async("/skill.md", handle_skill_md)
        .get_async("/llms.txt", handle_llms_txt)
        .get("/.well-known/ai-plugin.json", |_, _| {
            let headers = Headers::new();
            let _ = headers.set("Content-Type", "application/json; charset=utf-8");
            let _ = headers.set("Access-Control-Allow-Origin", "*");
            Ok(Response::ok(include_str!("ai-plugin.json"))?.with_headers(headers))
        })
        .get("/api/docs", |_, _| {
            let headers = Headers::new();
            let _ = headers.set("Content-Type", "application/json; charset=utf-8");
            let _ = headers.set("Access-Control-Allow-Origin", "*");
            Ok(Response::ok(include_str!("openapi.json"))?.with_headers(headers))
        })
        .get("/api/info", |_, _| {
            let info = ServiceInfo {
                name: "noscha.io",
                description: "Lightning Network powered disposable email, subdomain & NIP-05 service. No KYC, no signup, instant activation.",
                version: "0.1.0",
                features: vec![
                    "Email notifications ({username}@noscha.io)",
                    "Subdomain provisioning ({username}.noscha.io)",
                    "NIP-05 Nostr identity verification",
                    "Lightning Network instant payments",
                    "1-day to 1-year rentals",
                ],
                pricing: "Starting from 200 sats (NIP-05, 1 day). Bundle all 3 services for a discount.",
                tools: vec![ToolInfo {
                    name: "nostaro",
                    description: "Rust Nostr CLI - post, zap, timeline, and more from the command line. Use your noscha.io NIP-05 identity with nostaro.",
                    github: "https://github.com/kojira/nostaro",
                    install: "cargo install nostaro",
                    examples: vec![
                        "nostaro post \"hello nostr\"",
                        "nostaro zap <npub> 100",
                        "nostaro timeline",
                        "nostaro profile set --nip05 yourname@noscha.io",
                    ],
                }],
            };
            let json = serde_json::to_string(&info).map_err(|e| Error::RustError(e.to_string()))?;
            let headers = Headers::new();
            let _ = headers.set("Content-Type", "application/json; charset=utf-8");
            Ok(Response::ok(json)?.with_headers(headers))
        })
        .get("/health", |_, _| {
            let health = HealthResponse { status: "ok", version: VERSION };
            Response::from_json(&health)
        })
        .get_async("/api/check/:username", handle_check_username)
        .post_async("/api/order", handle_create_order)
        .get_async("/api/order/:order_id/confirm/:challenge", handle_confirm_webhook)
        .get_async("/api/order/:order_id/status", handle_order_status)
        .post_async("/api/webhook/coinos", handle_coinos_webhook)
        .post_async("/api/renew", handle_renew)
        .put_async("/api/settings/:token", handle_settings_update)
        .get_async("/my/:token", handle_my_page)
        .get_async("/api/pricing", handle_public_pricing)
        .get_async("/.well-known/nostr.json", handle_nip05)
        .options_async("/.well-known/nostr.json", handle_nip05_options)
        // Admin routes
        .get_async("/admin", handle_admin_page)
        .post_async("/api/admin/challenge", handle_admin_challenge)
        .post_async("/api/admin/login", handle_admin_login)
        .get_async("/api/admin/rentals", handle_admin_rentals)
        .get_async("/api/admin/stats", handle_admin_stats)
        .get_async("/api/admin/pricing", handle_admin_pricing_get)
        .put_async("/api/admin/pricing", handle_admin_pricing_put)
        .post_async("/api/admin/ban/:username", handle_admin_ban)
        .post_async("/api/admin/unban/:username", handle_admin_unban)
        .post_async("/api/admin/extend/:username", handle_admin_extend)
        .post_async("/api/admin/revoke/:username", handle_admin_revoke)
        .post_async("/api/admin/provision", handle_admin_provision)
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
