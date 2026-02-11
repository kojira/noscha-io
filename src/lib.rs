use serde::Serialize;
use worker::*;

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
        .run(req, env)
        .await
}
