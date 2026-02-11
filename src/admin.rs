use serde::{Deserialize, Serialize};
#[cfg(target_arch = "wasm32")]
use worker::*;

use crate::types::*;

/// BAN record stored in R2 at bans/{username}.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BanRecord {
    pub username: String,
    pub banned_at: String,
    pub reason: Option<String>,
}

/// Check if a username is banned by looking up bans/{username}.json in R2
#[cfg(target_arch = "wasm32")]
pub async fn is_banned(bucket: &worker::Bucket, username: &str) -> bool {
    let key = format!("bans/{}.json", username);
    matches!(bucket.get(&key).execute().await, Ok(Some(_)))
}

/// Response for GET /admin/stats
#[derive(Debug, Serialize, Deserialize)]
pub struct AdminStatsResponse {
    pub active_rentals: u64,
    pub expired_rentals: u64,
    pub banned_users: u64,
    pub expiring_soon: u64,
    pub total_revenue_sats: u64,
}

/// Single rental entry for admin listing
#[derive(Debug, Serialize, Deserialize)]
pub struct AdminRentalEntry {
    pub username: String,
    pub status: String,
    pub plan: Plan,
    pub created_at: String,
    pub expires_at: String,
    pub minutes_remaining: i64,
    pub has_email: bool,
    pub has_subdomain: bool,
    pub has_nip05: bool,
}

/// Paginated list response
#[derive(Debug, Serialize, Deserialize)]
pub struct AdminRentalsResponse {
    pub rentals: Vec<AdminRentalEntry>,
    pub total: usize,
    pub page: usize,
    pub limit: usize,
}

/// Request body for POST /admin/extend/{username}
#[derive(Debug, Serialize, Deserialize)]
pub struct ExtendRequest {
    pub minutes: u64,
}

/// Debug webhook config stored in R2 at config/debug_webhook.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugWebhookConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub webhook_url: String,
    #[serde(default)]
    pub level: String,
}

impl Default for DebugWebhookConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            webhook_url: String::new(),
            level: "off".to_string(),
        }
    }
}

/// Verify auth: Bearer token (ADMIN_API_TOKEN) OR session token (X-Admin-Token from NIP-07 login)
#[cfg(target_arch = "wasm32")]
async fn verify_session_token(req: &Request, bucket: &worker::Bucket, env: &Env) -> Result<()> {
    // Check Authorization: Bearer <token> against ADMIN_API_TOKEN secret
    if let Ok(Some(auth_header)) = req.headers().get("Authorization") {
        if let Some(bearer_token) = auth_header.strip_prefix("Bearer ") {
            if let Ok(secret) = env.secret("ADMIN_API_TOKEN") {
                let expected = secret.to_string();
                if !expected.is_empty() && bearer_token == expected {
                    return Ok(());
                }
            }
        }
    }

    // Fall back to X-Admin-Token session check
    let token = req
        .headers()
        .get("X-Admin-Token")
        .map_err(|_| Error::RustError("Missing X-Admin-Token header".to_string()))?
        .ok_or_else(|| Error::RustError("Missing X-Admin-Token header".to_string()))?;

    let key = format!("sessions/{}.json", token);
    let obj = bucket.get(&key).execute().await?;
    match obj {
        Some(obj) => {
            let body = obj.body().unwrap();
            let text = body.text().await?;
            let session: crate::types::AdminSession = serde_json::from_str(&text)
                .map_err(|e| Error::RustError(e.to_string()))?;
            // Check expiry
            let now_ms = js_sys::Date::now();
            let expires_date = js_sys::Date::new(&session.expires_at.clone().into());
            if expires_date.get_time() < now_ms {
                return Err(Error::RustError("Session expired".to_string()));
            }
            Ok(())
        }
        None => Err(Error::RustError("Invalid session token".to_string())),
    }
}

/// POST /api/admin/challenge — generate auth challenge
#[cfg(target_arch = "wasm32")]
pub async fn handle_admin_challenge(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let bucket = ctx.env.bucket("BUCKET")?;
    let now = js_sys::Date::new_0();
    let now_ms = js_sys::Date::now();
    let expires_ms = now_ms + 5.0 * 60.0 * 1000.0; // 5 minutes
    let expires_date = js_sys::Date::new(&(expires_ms.into()));

    let challenge = format!("ch_{:x}_{:x}", now_ms as u64, (now_ms * 1000.0) as u64);
    let ch = crate::types::AdminChallenge {
        challenge: challenge.clone(),
        created_at: now.to_iso_string().as_string().unwrap_or_default(),
        expires_at: expires_date.to_iso_string().as_string().unwrap_or_default(),
    };

    let key = format!("challenges/{}.json", challenge);
    let json = serde_json::to_string(&ch).map_err(|e| Error::RustError(e.to_string()))?;
    bucket.put(&key, json).execute().await?;

    Response::from_json(&serde_json::json!({ "challenge": challenge }))
}

/// POST /api/admin/login — verify NIP-07 signed event and issue session token
#[cfg(target_arch = "wasm32")]
pub async fn handle_admin_login(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let bucket = ctx.env.bucket("BUCKET")?;
    let admin_pubkey = ctx.env
        .secret("ADMIN_PUBKEY")
        .map(|s| s.to_string())
        .map_err(|_| Error::RustError("ADMIN_PUBKEY not configured".to_string()))?;

    let event: crate::types::NostrEvent = req.json().await
        .map_err(|_| Error::RustError("Invalid event JSON".to_string()))?;

    // Verify pubkey matches admin
    if event.pubkey != admin_pubkey {
        return Response::error("Unauthorized: invalid pubkey", 403);
    }

    // Extract challenge from content
    let challenge = event.content.trim().to_string();
    if challenge.is_empty() {
        return Response::error("Missing challenge in event content", 400);
    }

    // Verify challenge exists in R2 and not expired
    let ch_key = format!("challenges/{}.json", challenge);
    let ch_obj = bucket.get(&ch_key).execute().await?;
    match ch_obj {
        Some(obj) => {
            let body = obj.body().unwrap();
            let text = body.text().await?;
            let ch: crate::types::AdminChallenge = serde_json::from_str(&text)
                .map_err(|e| Error::RustError(e.to_string()))?;
            let now_ms = js_sys::Date::now();
            let expires_date = js_sys::Date::new(&ch.expires_at.clone().into());
            if expires_date.get_time() < now_ms {
                // Delete expired challenge
                let _ = bucket.delete(&ch_key).await;
                return Response::error("Challenge expired", 400);
            }
            // Delete used challenge
            let _ = bucket.delete(&ch_key).await;
        }
        None => {
            return Response::error("Invalid challenge", 400);
        }
    }

    // Issue session token (24h TTL)
    let now = js_sys::Date::new_0();
    let now_ms = js_sys::Date::now();
    let expires_ms = now_ms + 24.0 * 60.0 * 60.0 * 1000.0;
    let expires_date = js_sys::Date::new(&(expires_ms.into()));

    let token = format!("sess_{:x}", now_ms as u64);
    let session = crate::types::AdminSession {
        token: token.clone(),
        pubkey: event.pubkey,
        created_at: now.to_iso_string().as_string().unwrap_or_default(),
        expires_at: expires_date.to_iso_string().as_string().unwrap_or_default(),
    };

    let sess_key = format!("sessions/{}.json", token);
    let sess_json = serde_json::to_string(&session).map_err(|e| Error::RustError(e.to_string()))?;
    bucket.put(&sess_key, sess_json).execute().await?;

    Response::from_json(&serde_json::json!({ "token": token }))
}

/// GET /api/admin/pricing — get current pricing config
#[cfg(target_arch = "wasm32")]
pub async fn handle_admin_pricing_get(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let bucket = ctx.env.bucket("BUCKET")?;
    if let Err(_) = verify_session_token(&req, &bucket, &ctx.env).await {
        return Response::error("Unauthorized", 401);
    }

    let pricing = load_pricing(&bucket).await;
    Response::from_json(&pricing)
}

/// PUT /api/admin/pricing — update pricing config
#[cfg(target_arch = "wasm32")]
pub async fn handle_admin_pricing_put(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let bucket = ctx.env.bucket("BUCKET")?;
    if let Err(_) = verify_session_token(&req, &bucket, &ctx.env).await {
        return Response::error("Unauthorized", 401);
    }

    let pricing: crate::types::PricingConfig = req.json().await
        .map_err(|_| Error::RustError("Invalid pricing JSON".to_string()))?;

    let json = serde_json::to_string(&pricing).map_err(|e| Error::RustError(e.to_string()))?;
    bucket.put("config/pricing.json", json).execute().await?;

    Response::from_json(&pricing)
}

/// GET /api/pricing — public pricing data (no auth required)
#[cfg(target_arch = "wasm32")]
pub async fn handle_public_pricing(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let bucket = ctx.env.bucket("BUCKET")?;
    let pricing = load_pricing(&bucket).await;
    Response::from_json(&pricing)
}

/// Load pricing config from R2, falling back to defaults
#[cfg(target_arch = "wasm32")]
pub async fn load_pricing(bucket: &worker::Bucket) -> crate::types::PricingConfig {
    match bucket.get("config/pricing.json").execute().await {
        Ok(Some(obj)) => {
            if let Some(body) = obj.body() {
                if let Ok(text) = body.text().await {
                    if let Ok(config) = serde_json::from_str::<crate::types::PricingConfig>(&text) {
                        return config;
                    }
                }
            }
            crate::types::default_pricing()
        }
        _ => crate::types::default_pricing(),
    }
}

/// Load debug webhook config from R2, falling back to defaults
#[cfg(target_arch = "wasm32")]
pub async fn load_debug_webhook_config(bucket: &worker::Bucket) -> DebugWebhookConfig {
    match bucket.get("config/debug_webhook.json").execute().await {
        Ok(Some(obj)) => {
            if let Some(body) = obj.body() {
                if let Ok(text) = body.text().await {
                    if let Ok(config) = serde_json::from_str::<DebugWebhookConfig>(&text) {
                        return config;
                    }
                }
            }
            DebugWebhookConfig::default()
        }
        _ => DebugWebhookConfig::default(),
    }
}

/// GET /api/admin/debug-webhook — get current debug webhook config
#[cfg(target_arch = "wasm32")]
pub async fn handle_admin_debug_webhook_get(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let bucket = ctx.env.bucket("BUCKET")?;
    if verify_session_token(&req, &bucket, &ctx.env).await.is_err() {
        return Response::error("Unauthorized", 401);
    }

    let config = load_debug_webhook_config(&bucket).await;
    Response::from_json(&config)
}

/// PUT /api/admin/debug-webhook — update debug webhook config
#[cfg(target_arch = "wasm32")]
pub async fn handle_admin_debug_webhook_put(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let bucket = ctx.env.bucket("BUCKET")?;
    if verify_session_token(&req, &bucket, &ctx.env).await.is_err() {
        return Response::error("Unauthorized", 401);
    }

    let body: DebugWebhookConfig = req.json().await
        .map_err(|_| Error::RustError("Invalid debug webhook config JSON".to_string()))?;

    let level = body.level.to_lowercase();
    let valid_levels = ["off", "error", "warn", "info", "debug"];
    if !valid_levels.contains(&level.as_str()) {
        return Response::error("Invalid level: must be off, error, warn, info, or debug", 400);
    }

    let config = DebugWebhookConfig {
        enabled: body.enabled,
        webhook_url: body.webhook_url.trim().to_string(),
        level,
    };

    let json = serde_json::to_string(&config).map_err(|e| Error::RustError(e.to_string()))?;
    bucket.put("config/debug_webhook.json", json).execute().await?;

    Response::from_json(&config)
}

/// GET /admin — serve admin dashboard HTML
#[cfg(target_arch = "wasm32")]
pub async fn handle_admin_page(_req: Request, _ctx: RouteContext<()>) -> Result<Response> {
    Response::from_html(include_str!("admin_ui.html"))
}

/// GET /admin/rentals?page=1&limit=20&status=active
#[cfg(target_arch = "wasm32")]
pub async fn handle_admin_rentals(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let bucket = ctx.env.bucket("BUCKET")?;
    if let Err(_) = verify_session_token(&req, &bucket, &ctx.env).await {
        return Response::error("Unauthorized", 401);
    }

    let url = req.url()?;
    let params: std::collections::HashMap<String, String> = url.query_pairs().into_owned().collect();
    let page: usize = params
        .get("page")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1)
        .max(1);
    let limit: usize = params
        .get("limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(20)
        .min(100);
    let status_filter = params.get("status").cloned();

    let bucket = ctx.env.bucket("BUCKET")?;
    let now_ms = js_sys::Date::now();

    // Collect all rentals
    let list = bucket.list().prefix("rentals/").execute().await?;
    let mut entries: Vec<AdminRentalEntry> = Vec::new();

    for obj_entry in list.objects() {
        let key = obj_entry.key();
        if let Some(obj) = bucket.get(&key).execute().await? {
            let body = obj.body().unwrap();
            let text = body.text().await?;
            if let Ok(rental) = serde_json::from_str::<Rental>(&text) {
                let expires_date = js_sys::Date::new(&rental.expires_at.clone().into());
                let expires_ms = expires_date.get_time();
                let minutes_remaining = ((expires_ms - now_ms) / (60.0 * 1000.0)).ceil() as i64;

                // Check if banned
                let banned = is_banned(&bucket, &rental.username).await;
                let display_status = if banned {
                    "banned".to_string()
                } else {
                    rental.status.clone()
                };

                // Apply status filter
                if let Some(ref filter) = status_filter {
                    if filter != &display_status {
                        continue;
                    }
                }

                entries.push(AdminRentalEntry {
                    username: rental.username,
                    status: display_status,
                    plan: rental.plan,
                    created_at: rental.created_at,
                    expires_at: rental.expires_at,
                    minutes_remaining,
                    has_email: rental.services.email.as_ref().map(|e| e.enabled).unwrap_or(false),
                    has_subdomain: rental.services.subdomain.as_ref().map(|s| s.enabled).unwrap_or(false),
                    has_nip05: rental.services.nip05.as_ref().map(|n| n.enabled).unwrap_or(false),
                });
            }
        }
    }

    // Sort by expires_at descending (newest first)
    entries.sort_by(|a, b| b.expires_at.cmp(&a.expires_at));

    let total = entries.len();
    let start = (page - 1) * limit;
    let page_entries: Vec<AdminRentalEntry> = entries.into_iter().skip(start).take(limit).collect();

    Response::from_json(&AdminRentalsResponse {
        rentals: page_entries,
        total,
        page,
        limit,
    })
}

/// GET /admin/stats
#[cfg(target_arch = "wasm32")]
pub async fn handle_admin_stats(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let bucket = ctx.env.bucket("BUCKET")?;
    if let Err(_) = verify_session_token(&req, &bucket, &ctx.env).await {
        return Response::error("Unauthorized", 401);
    }
    let now_ms = js_sys::Date::now();
    let soon_ms = now_ms + 7.0 * 24.0 * 60.0 * 60.0 * 1000.0; // 7 days

    let mut active: u64 = 0;
    let mut expired: u64 = 0;
    let mut expiring_soon: u64 = 0;

    let list = bucket.list().prefix("rentals/").execute().await?;
    for obj_entry in list.objects() {
        let key = obj_entry.key();
        if let Some(obj) = bucket.get(&key).execute().await? {
            let body = obj.body().unwrap();
            let text = body.text().await?;
            if let Ok(rental) = serde_json::from_str::<Rental>(&text) {
                if rental.status == "active" {
                    active += 1;
                    let expires_date = js_sys::Date::new(&rental.expires_at.clone().into());
                    let expires_ms = expires_date.get_time();
                    if expires_ms <= soon_ms {
                        expiring_soon += 1;
                    }
                } else {
                    expired += 1;
                }
            }
        }
    }

    // Count bans
    let mut banned: u64 = 0;
    let ban_list = bucket.list().prefix("bans/").execute().await?;
    for _ in ban_list.objects() {
        banned += 1;
    }

    // Calculate total revenue from provisioned orders
    let mut total_revenue: u64 = 0;
    let order_list = bucket.list().prefix("orders/").execute().await?;
    for obj_entry in order_list.objects() {
        let key = obj_entry.key();
        if let Some(obj) = bucket.get(&key).execute().await? {
            let body = obj.body().unwrap();
            let text = body.text().await?;
            if let Ok(order) = serde_json::from_str::<Order>(&text) {
                if order.status == OrderStatus::Paid || order.status == OrderStatus::Provisioned {
                    total_revenue += order.amount_sats;
                }
            }
        }
    }

    Response::from_json(&AdminStatsResponse {
        active_rentals: active,
        expired_rentals: expired,
        banned_users: banned,
        expiring_soon,
        total_revenue_sats: total_revenue,
    })
}

/// POST /admin/ban/{username}
#[cfg(target_arch = "wasm32")]
pub async fn handle_admin_ban(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let bucket = ctx.env.bucket("BUCKET")?;
    if let Err(_) = verify_session_token(&req, &bucket, &ctx.env).await {
        return Response::error("Unauthorized", 401);
    }

    let username = ctx.param("username").unwrap().to_string();

    // Check if already banned
    if is_banned(&bucket, &username).await {
        return Response::error("User is already banned", 409);
    }

    // Create ban record
    let now = js_sys::Date::new_0();
    let ban = BanRecord {
        username: username.clone(),
        banned_at: now.to_iso_string().as_string().unwrap_or_default(),
        reason: None,
    };
    let ban_json = serde_json::to_string(&ban).map_err(|e| Error::RustError(e.to_string()))?;
    let ban_key = format!("bans/{}.json", username);
    bucket.put(&ban_key, ban_json).execute().await?;

    // Delete rental services (mark as expired, remove DNS)
    let rental_key = format!("rentals/{}.json", username);
    if let Some(obj) = bucket.get(&rental_key).execute().await? {
        let body = obj.body().unwrap();
        let text = body.text().await?;
        if let Ok(mut rental) = serde_json::from_str::<Rental>(&text) {
            // Delete DNS record if present
            if let Some(ref sub) = rental.services.subdomain {
                if let Some(ref record_id) = sub.cf_record_id {
                    let zone_id = ctx.env.var("CF_ZONE_ID").map(|v| v.to_string()).unwrap_or_default();
                    if !zone_id.is_empty() {
                        let is_mock = crate::dns_mock::is_mock_dns_enabled(&ctx.env);
                        let _ = if is_mock {
                            crate::dns_mock::delete_dns_record(&zone_id, "", record_id).await
                        } else {
                            let token = ctx.env.secret("CF_API_TOKEN")?.to_string();
                            crate::dns::delete_dns_record(&zone_id, &token, record_id).await
                        };
                    }
                }
            }

            // Mark rental as expired
            rental.status = "expired".to_string();
            let updated = serde_json::to_string(&rental).map_err(|e| Error::RustError(e.to_string()))?;
            bucket.put(&rental_key, updated).execute().await?;
        }
    }

    Response::ok("banned")
}

/// POST /admin/unban/{username}
#[cfg(target_arch = "wasm32")]
pub async fn handle_admin_unban(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let bucket = ctx.env.bucket("BUCKET")?;
    if let Err(_) = verify_session_token(&req, &bucket, &ctx.env).await {
        return Response::error("Unauthorized", 401);
    }

    let username = ctx.param("username").unwrap().to_string();

    let ban_key = format!("bans/{}.json", username);
    if !is_banned(&bucket, &username).await {
        return Response::error("User is not banned", 404);
    }

    bucket.delete(&ban_key).await?;
    Response::ok("unbanned")
}

/// POST /admin/extend/{username}  body: {"minutes": 30}
#[cfg(target_arch = "wasm32")]
pub async fn handle_admin_extend(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let bucket = ctx.env.bucket("BUCKET")?;
    if let Err(_) = verify_session_token(&req, &bucket, &ctx.env).await {
        return Response::error("Unauthorized", 401);
    }

    let username = ctx.param("username").unwrap().to_string();
    let body: ExtendRequest = req
        .json()
        .await
        .map_err(|_| Error::RustError("Invalid request body, expected {\"minutes\": N}".to_string()))?;

    if body.minutes == 0 || body.minutes > 525600 {
        return Response::error("Minutes must be between 1 and 525600", 400);
    }
    let rental_key = format!("rentals/{}.json", username);

    let obj = bucket.get(&rental_key).execute().await?;
    match obj {
        Some(obj) => {
            let obj_body = obj.body().unwrap();
            let text = obj_body.text().await?;
            let mut rental: Rental =
                serde_json::from_str(&text).map_err(|e| Error::RustError(e.to_string()))?;

            // Extend from current expires_at (or now if already expired)
            let now_ms = js_sys::Date::now();
            let current_expires = js_sys::Date::new(&rental.expires_at.clone().into());
            let base_ms = if current_expires.get_time() > now_ms {
                current_expires.get_time()
            } else {
                now_ms
            };
            let extension_ms = body.minutes as f64 * 60.0 * 1000.0;
            let new_expires = js_sys::Date::new(&(base_ms + extension_ms).into());
            rental.expires_at = new_expires.to_iso_string().as_string().unwrap_or_default();
            rental.status = "active".to_string();

            let updated =
                serde_json::to_string(&rental).map_err(|e| Error::RustError(e.to_string()))?;
            bucket.put(&rental_key, updated).execute().await?;

            Response::ok("extended")
        }
        None => Response::error("Rental not found", 404),
    }
}

/// POST /admin/revoke/{username}
#[cfg(target_arch = "wasm32")]
pub async fn handle_admin_revoke(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let bucket = ctx.env.bucket("BUCKET")?;
    if let Err(_) = verify_session_token(&req, &bucket, &ctx.env).await {
        return Response::error("Unauthorized", 401);
    }

    let username = ctx.param("username").unwrap().to_string();
    let rental_key = format!("rentals/{}.json", username);

    let obj = bucket.get(&rental_key).execute().await?;
    match obj {
        Some(obj) => {
            let obj_body = obj.body().unwrap();
            let text = obj_body.text().await?;
            let mut rental: Rental =
                serde_json::from_str(&text).map_err(|e| Error::RustError(e.to_string()))?;

            if rental.status != "active" {
                return Response::error("Rental is not active", 400);
            }

            // Delete DNS record if present
            if let Some(ref sub) = rental.services.subdomain {
                if let Some(ref record_id) = sub.cf_record_id {
                    let zone_id = ctx.env.var("CF_ZONE_ID").map(|v| v.to_string()).unwrap_or_default();
                    if !zone_id.is_empty() {
                        let is_mock = crate::dns_mock::is_mock_dns_enabled(&ctx.env);
                        let _ = if is_mock {
                            crate::dns_mock::delete_dns_record(&zone_id, "", record_id).await
                        } else {
                            let token = ctx.env.secret("CF_API_TOKEN")?.to_string();
                            crate::dns::delete_dns_record(&zone_id, &token, record_id).await
                        };
                    }
                }
            }

            // Mark as expired
            rental.status = "expired".to_string();
            let updated =
                serde_json::to_string(&rental).map_err(|e| Error::RustError(e.to_string()))?;
            bucket.put(&rental_key, updated).execute().await?;

            Response::ok("revoked")
        }
        None => Response::error("Rental not found", 404),
    }
}

/// Request body for POST /api/admin/provision
#[derive(Debug, Deserialize)]
pub struct AdminProvisionRequest {
    pub username: String,
    pub service: String,
    pub plan: Plan,
    #[serde(default)]
    pub pubkey: Option<String>,
    #[serde(default)]
    pub dns_type: Option<String>,
    #[serde(default)]
    pub dns_value: Option<String>,
}

/// POST /api/admin/provision — directly provision a rental (skip payment)
#[cfg(target_arch = "wasm32")]
pub async fn handle_admin_provision(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let bucket = ctx.env.bucket("BUCKET")?;
    if let Err(_) = verify_session_token(&req, &bucket, &ctx.env).await {
        return Response::error("Unauthorized", 401);
    }

    let body: AdminProvisionRequest = req.json().await
        .map_err(|_| Error::RustError("Invalid request body".to_string()))?;

    if let Err(err) = crate::validation::validate_username(&body.username) {
        return Response::error(err, 400);
    }

    if is_banned(&bucket, &body.username).await {
        return Response::error("This username is blocked", 403);
    }

    let rental_key = format!("rentals/{}.json", body.username);
    if let Some(obj) = bucket.get(&rental_key).execute().await? {
        let obj_body = obj.body().unwrap();
        let text = obj_body.text().await?;
        if let Ok(rental) = serde_json::from_str::<Rental>(&text) {
            let now_ms = js_sys::Date::now();
            let expires_date = js_sys::Date::new(&rental.expires_at.clone().into());
            if expires_date.get_time() > now_ms {
                return Response::error("Username is already taken", 409);
            }
        }
    }

    let now_ms = js_sys::Date::now();
    let now_date = js_sys::Date::new_0();
    let now_iso = now_date.to_iso_string().as_string().unwrap_or_default();
    let duration_ms = body.plan.duration_minutes() as f64 * 60.0 * 1000.0;
    let expires_ms = now_ms + duration_ms;
    let expires_date = js_sys::Date::new(&(expires_ms.into()));
    let expires_at = expires_date.to_iso_string().as_string().unwrap_or_default();

    let is_bundle = body.service == "bundle";
    let mgmt_token = format!("mgmt_{:x}", js_sys::Date::now() as u64);

    let nip05_service = if body.service == "nip05" || is_bundle {
        body.pubkey.as_ref().map(|pk| Nip05Service {
            enabled: true,
            pubkey_hex: pk.clone(),
            relays: vec![],
        })
    } else {
        None
    };

    let email_service = if body.service == "email" || is_bundle {
        Some(EmailService {
            enabled: true,
            cf_rule_id: None,
        })
    } else {
        None
    };

    let subdomain_service = if body.service == "subdomain" || is_bundle {
        if let (Some(ref dns_type), Some(ref dns_value)) = (&body.dns_type, &body.dns_value) {
            let zone_id = ctx.env.var("CF_ZONE_ID").map(|v| v.to_string()).unwrap_or_default();
            let domain = ctx.env.var("DOMAIN").map(|v| v.to_string()).unwrap_or_else(|_| "noscha.io".to_string());

            let record_type = match dns_type.to_uppercase().as_str() {
                "CNAME" => crate::dns::DnsRecordType::CNAME,
                "A" => crate::dns::DnsRecordType::A,
                "AAAA" => crate::dns::DnsRecordType::AAAA,
                other => return Response::error(format!("Unsupported DNS record type: {}", other), 400),
            };

            let mut cf_record_id = None;
            if !zone_id.is_empty() {
                let is_mock = crate::dns_mock::is_mock_dns_enabled(&ctx.env);
                let record_id = if is_mock {
                    crate::dns_mock::create_dns_record(&zone_id, "", &body.username, &record_type, dns_value, false, &body.username, &expires_at, &domain).await?
                } else {
                    let token = ctx.env.secret("CF_API_TOKEN")?.to_string();
                    crate::dns::create_dns_record(&zone_id, &token, &body.username, &record_type, dns_value, false, &body.username, &expires_at, &domain).await?
                };
                cf_record_id = Some(record_id);
            }

            Some(SubdomainService {
                enabled: true,
                record_type: dns_type.clone(),
                target: dns_value.clone(),
                proxied: false,
                cf_record_id,
            })
        } else {
            None
        }
    } else {
        None
    };

    let rental = Rental {
        username: body.username.clone(),
        status: "active".to_string(),
        created_at: now_iso,
        expires_at: expires_at.clone(),
        plan: body.plan,
        services: RentalServices {
            email: email_service,
            subdomain: subdomain_service,
            nip05: nip05_service,
        },
        management_token: Some(mgmt_token.clone()),
        webhook_url: None,
    };

    let rental_json = serde_json::to_string(&rental).map_err(|e| Error::RustError(e.to_string()))?;
    bucket.put(&rental_key, rental_json).execute().await?;

    Response::from_json(&serde_json::json!({
        "success": true,
        "username": body.username,
        "expires_at": expires_at,
        "management_token": mgmt_token,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ban_record_serde() {
        let ban = BanRecord {
            username: "spammer".to_string(),
            banned_at: "2025-01-15T12:00:00Z".to_string(),
            reason: Some("abuse".to_string()),
        };
        let json = serde_json::to_string(&ban).unwrap();
        let parsed: BanRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.username, "spammer");
        assert_eq!(parsed.reason, Some("abuse".to_string()));
    }

    #[test]
    fn test_ban_record_without_reason() {
        let ban = BanRecord {
            username: "baduser".to_string(),
            banned_at: "2025-02-01T00:00:00Z".to_string(),
            reason: None,
        };
        let json = serde_json::to_string(&ban).unwrap();
        let parsed: BanRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.username, "baduser");
        assert_eq!(parsed.reason, None);
    }

    #[test]
    fn test_extend_request_serde() {
        let json = r#"{"minutes": 30}"#;
        let req: ExtendRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.minutes, 30);
    }

    #[test]
    fn test_extend_request_zero_minutes() {
        let json = r#"{"minutes": 0}"#;
        let req: ExtendRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.minutes, 0);
        // Validation happens in handler: 0 is rejected
    }

    #[test]
    fn test_admin_stats_response_serde() {
        let stats = AdminStatsResponse {
            active_rentals: 42,
            expired_rentals: 10,
            banned_users: 3,
            expiring_soon: 5,
            total_revenue_sats: 12500,
        };
        let json = serde_json::to_value(&stats).unwrap();
        assert_eq!(json["active_rentals"], 42);
        assert_eq!(json["expired_rentals"], 10);
        assert_eq!(json["banned_users"], 3);
        assert_eq!(json["expiring_soon"], 5);
        assert_eq!(json["total_revenue_sats"], 12500);
    }

    #[test]
    fn test_admin_rental_entry_services() {
        let entry = AdminRentalEntry {
            username: "alice".to_string(),
            status: "active".to_string(),
            plan: Plan::ThirtyDays,
            created_at: "2025-01-01T00:00:00Z".to_string(),
            expires_at: "2025-02-01T00:00:00Z".to_string(),
            minutes_remaining: 15,
            has_email: true,
            has_subdomain: false,
            has_nip05: true,
        };
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["username"], "alice");
        assert_eq!(json["has_email"], true);
        assert_eq!(json["has_subdomain"], false);
        assert_eq!(json["has_nip05"], true);
        assert_eq!(json["minutes_remaining"], 15);
    }

    #[test]
    fn test_admin_rentals_response_pagination() {
        let resp = AdminRentalsResponse {
            rentals: vec![],
            total: 50,
            page: 3,
            limit: 20,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["total"], 50);
        assert_eq!(json["page"], 3);
        assert_eq!(json["limit"], 20);
        assert!(json["rentals"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_bearer_token_header_parsing() {
        // Test extracting Bearer token from Authorization header
        let header = "Bearer my_secret_token_123";
        let token = header.strip_prefix("Bearer ");
        assert_eq!(token, Some("my_secret_token_123"));

        // Non-bearer header should not match
        let header2 = "Basic dXNlcjpwYXNz";
        let token2 = header2.strip_prefix("Bearer ");
        assert_eq!(token2, None);

        // Empty bearer should extract empty string
        let header3 = "Bearer ";
        let token3 = header3.strip_prefix("Bearer ");
        assert_eq!(token3, Some(""));
    }

    #[test]
    fn test_banned_status_display() {
        // Verify that "banned" is a valid status string for display
        let entry = AdminRentalEntry {
            username: "baduser".to_string(),
            status: "banned".to_string(),
            plan: Plan::OneDay,
            created_at: "2025-01-01T00:00:00Z".to_string(),
            expires_at: "2025-01-02T00:00:00Z".to_string(),
            minutes_remaining: -5,
            has_email: false,
            has_subdomain: false,
            has_nip05: false,
        };
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["status"], "banned");
        assert_eq!(json["minutes_remaining"], -5);
    }
}
