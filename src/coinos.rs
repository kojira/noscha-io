#![cfg(target_arch = "wasm32")]

use serde::{Deserialize, Serialize};
use worker::*;

const USER_AGENT: &str = "Mozilla/5.0 (compatible; noscha.io/0.1)";

#[derive(Debug, Serialize)]
struct InvoiceInner {
    amount: u64,
    #[serde(rename = "type")]
    invoice_type: String,
    webhook: String,
    secret: String,
}

#[derive(Debug, Serialize)]
struct CreateInvoiceRequest {
    invoice: InvoiceInner,
}

#[derive(Debug, Deserialize)]
pub struct CoinosInvoiceResponse {
    #[serde(default)]
    #[allow(dead_code)]
    pub id: Option<String>,
    #[allow(dead_code)]
    pub amount: u64,
    /// bolt11 invoice string
    pub text: String,
    #[serde(default)]
    pub hash: Option<String>,
}

/// Create a Lightning invoice via Coinos API
pub async fn create_invoice(
    api_token: &str,
    amount_sats: u64,
    webhook_url: &str,
    order_secret: &str,
) -> Result<CoinosInvoiceResponse> {
    let body = CreateInvoiceRequest {
        invoice: InvoiceInner {
            amount: amount_sats,
            invoice_type: "lightning".to_string(),
            webhook: webhook_url.to_string(),
            secret: order_secret.to_string(),
        },
    };

    let headers = Headers::new();
    headers.set("Content-Type", "application/json")?;
    headers.set("Authorization", &format!("Bearer {}", api_token))?;
    headers.set("User-Agent", USER_AGENT)?;

    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(
            serde_json::to_string(&body)
                .map_err(|e| Error::RustError(e.to_string()))?
                .into(),
        ));

    let request = Request::new_with_init("https://coinos.io/api/invoice", &init)?;
    let mut response = Fetch::Request(request).send().await?;

    if response.status_code() != 200 {
        let text = response.text().await.unwrap_or_default();
        return Err(Error::RustError(format!(
            "Coinos API error ({}): {}",
            response.status_code(),
            text
        )));
    }

    let invoice: CoinosInvoiceResponse = response.json().await?;
    Ok(invoice)
}
