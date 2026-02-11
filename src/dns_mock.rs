#![cfg(target_arch = "wasm32")]

use worker::*;

/// Check if mock DNS mode is enabled via environment variable
pub fn is_mock_dns_enabled(env: &Env) -> bool {
    env.var("MOCK_DNS")
        .map(|v| v.to_string() == "true")
        .unwrap_or(false)
}

fn generate_mock_id() -> String {
    let now = js_sys::Date::now() as u64;
    format!("{:x}", now)
}

/// Mock: Create a DNS record (logs operation, returns fake record ID)
pub async fn create_dns_record(
    _zone_id: &str,
    _token: &str,
    subdomain: &str,
    record_type: &crate::dns::DnsRecordType,
    content: &str,
    proxied: bool,
    username: &str,
    expires: &str,
    domain: &str,
) -> Result<String> {
    let record_id = format!("mock_dns_{}", generate_mock_id());
    console_log!(
        "[MOCK DNS] create_dns_record: {}.{} {} -> {} (proxied={}, user={}, expires={}) => {}",
        subdomain,
        domain,
        record_type,
        content,
        proxied,
        username,
        expires,
        record_id
    );
    Ok(record_id)
}

/// Mock: Delete a DNS record (logs operation)
pub async fn delete_dns_record(_zone_id: &str, _token: &str, record_id: &str) -> Result<()> {
    console_log!("[MOCK DNS] delete_dns_record: {}", record_id);
    Ok(())
}

/// Mock: Update a DNS record's content (logs operation)
#[allow(dead_code)]
pub async fn update_dns_record(
    _zone_id: &str,
    _token: &str,
    record_id: &str,
    content: &str,
) -> Result<()> {
    console_log!(
        "[MOCK DNS] update_dns_record: {} -> {}",
        record_id,
        content
    );
    Ok(())
}
