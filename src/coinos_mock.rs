use crate::coinos::CoinosInvoiceResponse;
use worker::*;

/// Mock invoice creation for development/testing.
/// When MOCK_PAYMENT=true, returns a fake bolt11 and marks as immediately payable.
pub async fn create_mock_invoice(
    amount_sats: u64,
    _webhook_url: &str,
    _order_secret: &str,
) -> Result<CoinosInvoiceResponse> {
    Ok(CoinosInvoiceResponse {
        id: Some(format!("mock_inv_{}", generate_mock_id())),
        amount: amount_sats,
        text: format!("lnbc{}n1mock_invoice_for_testing", amount_sats),
        hash: Some(format!("mock_hash_{}", generate_mock_id())),
    })
}

/// Check if mock payment mode is enabled via environment variable
pub fn is_mock_enabled(env: &Env) -> bool {
    env.var("MOCK_PAYMENT")
        .map(|v| v.to_string() == "true")
        .unwrap_or(false)
}

fn generate_mock_id() -> String {
    let now = js_sys::Date::now() as u64;
    format!("{:x}", now)
}
