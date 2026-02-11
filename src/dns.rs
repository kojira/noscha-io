use serde::{Deserialize, Serialize};
#[cfg(target_arch = "wasm32")]
use worker::*;

/// Supported DNS record types for subdomain provisioning
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DnsRecordType {
    CNAME,
    A,
    AAAA,
}

impl DnsRecordType {
    pub fn as_str(&self) -> &'static str {
        match self {
            DnsRecordType::CNAME => "CNAME",
            DnsRecordType::A => "A",
            DnsRecordType::AAAA => "AAAA",
        }
    }
}

impl std::fmt::Display for DnsRecordType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Request body for creating a DNS record via Cloudflare API
#[derive(Debug, Serialize)]
pub struct CreateDnsRecordRequest {
    #[serde(rename = "type")]
    pub record_type: String,
    pub name: String,
    pub content: String,
    pub ttl: u32,
    pub proxied: bool,
    pub comment: String,
}

/// Request body for updating a DNS record via Cloudflare API
#[derive(Debug, Serialize)]
pub struct UpdateDnsRecordRequest {
    pub content: String,
}

/// Cloudflare API response wrapper
#[cfg(target_arch = "wasm32")]
#[derive(Debug, Deserialize)]
struct CfApiResponse {
    success: bool,
    result: Option<CfDnsResult>,
    errors: Option<Vec<CfApiError>>,
}

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Deserialize)]
struct CfDnsResult {
    id: String,
}

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Deserialize)]
struct CfApiError {
    message: String,
}

/// Validate a subdomain name for DNS provisioning
pub fn validate_subdomain(subdomain: &str) -> Result<(), String> {
    if subdomain.is_empty() {
        return Err("Subdomain cannot be empty".to_string());
    }
    if subdomain.len() > 63 {
        return Err("Subdomain must be at most 63 characters".to_string());
    }
    if subdomain.contains('.') {
        return Err("Subdomain cannot contain dots".to_string());
    }
    let lower = subdomain.to_lowercase();
    if lower != subdomain {
        return Err("Subdomain must be lowercase".to_string());
    }
    if subdomain.starts_with('-') || subdomain.ends_with('-') {
        return Err("Subdomain cannot start or end with a hyphen".to_string());
    }
    if !subdomain
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(
            "Subdomain can only contain lowercase letters, digits, and hyphens".to_string(),
        );
    }
    Ok(())
}

/// Build the request body for creating a DNS record
pub fn build_create_request(
    subdomain: &str,
    domain: &str,
    record_type: &DnsRecordType,
    content: &str,
    proxied: bool,
    username: &str,
    expires: &str,
) -> CreateDnsRecordRequest {
    CreateDnsRecordRequest {
        record_type: record_type.as_str().to_string(),
        name: format!("{}.{}", subdomain, domain),
        content: content.to_string(),
        ttl: 300,
        proxied,
        comment: format!("noscha rental: {}, expires: {}", username, expires),
    }
}

/// Create a DNS record via Cloudflare API
/// Returns the record ID on success
#[cfg(target_arch = "wasm32")]
pub async fn create_dns_record(
    zone_id: &str,
    token: &str,
    subdomain: &str,
    record_type: &DnsRecordType,
    content: &str,
    proxied: bool,
    username: &str,
    expires: &str,
    domain: &str,
) -> Result<String> {
    validate_subdomain(subdomain).map_err(|e| Error::RustError(e))?;

    let body = build_create_request(subdomain, domain, record_type, content, proxied, username, expires);

    let url = format!(
        "https://api.cloudflare.com/client/v4/zones/{}/dns_records",
        zone_id
    );

    let headers = Headers::new();
    headers.set("Content-Type", "application/json")?;
    headers.set("Authorization", &format!("Bearer {}", token))?;

    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(
            serde_json::to_string(&body)
                .map_err(|e| Error::RustError(e.to_string()))?
                .into(),
        ));

    let request = Request::new_with_init(&url, &init)?;
    let mut response = Fetch::Request(request).send().await?;

    if response.status_code() != 200 {
        let text = response.text().await.unwrap_or_default();
        return Err(Error::RustError(format!(
            "Cloudflare DNS API error ({}): {}",
            response.status_code(),
            text
        )));
    }

    let cf_resp: CfApiResponse = response.json().await?;
    if !cf_resp.success {
        let msg = cf_resp
            .errors
            .map(|errs| {
                errs.iter()
                    .map(|e| e.message.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_else(|| "Unknown error".to_string());
        return Err(Error::RustError(format!("Cloudflare DNS error: {}", msg)));
    }

    cf_resp
        .result
        .map(|r| r.id)
        .ok_or_else(|| Error::RustError("No record ID in response".to_string()))
}

/// Delete a DNS record via Cloudflare API
#[cfg(target_arch = "wasm32")]
pub async fn delete_dns_record(zone_id: &str, token: &str, record_id: &str) -> Result<()> {
    let url = format!(
        "https://api.cloudflare.com/client/v4/zones/{}/dns_records/{}",
        zone_id, record_id
    );

    let headers = Headers::new();
    headers.set("Authorization", &format!("Bearer {}", token))?;

    let mut init = RequestInit::new();
    init.with_method(Method::Delete).with_headers(headers);

    let request = Request::new_with_init(&url, &init)?;
    let mut response = Fetch::Request(request).send().await?;

    if response.status_code() != 200 {
        let text = response.text().await.unwrap_or_default();
        return Err(Error::RustError(format!(
            "Cloudflare DNS delete error ({}): {}",
            response.status_code(),
            text
        )));
    }

    Ok(())
}

/// Update a DNS record's content via Cloudflare API
#[cfg(target_arch = "wasm32")]
pub async fn update_dns_record(
    zone_id: &str,
    token: &str,
    record_id: &str,
    content: &str,
) -> Result<()> {
    let url = format!(
        "https://api.cloudflare.com/client/v4/zones/{}/dns_records/{}",
        zone_id, record_id
    );

    let body = UpdateDnsRecordRequest {
        content: content.to_string(),
    };

    let headers = Headers::new();
    headers.set("Content-Type", "application/json")?;
    headers.set("Authorization", &format!("Bearer {}", token))?;

    let mut init = RequestInit::new();
    init.with_method(Method::Patch)
        .with_headers(headers)
        .with_body(Some(
            serde_json::to_string(&body)
                .map_err(|e| Error::RustError(e.to_string()))?
                .into(),
        ));

    let request = Request::new_with_init(&url, &init)?;
    let mut response = Fetch::Request(request).send().await?;

    if response.status_code() != 200 {
        let text = response.text().await.unwrap_or_default();
        return Err(Error::RustError(format!(
            "Cloudflare DNS update error ({}): {}",
            response.status_code(),
            text
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_create_request_cname() {
        let req = build_create_request(
            "alice",
            "noscha.io",
            &DnsRecordType::CNAME,
            "mysite.example.com",
            false,
            "alice",
            "2025-02-14",
        );
        assert_eq!(req.record_type, "CNAME");
        assert_eq!(req.name, "alice.noscha.io");
        assert_eq!(req.content, "mysite.example.com");
        assert_eq!(req.ttl, 300);
        assert!(!req.proxied);
        assert!(req.comment.contains("alice"));
        assert!(req.comment.contains("2025-02-14"));
    }

    #[test]
    fn test_build_create_request_a() {
        let req = build_create_request(
            "bob",
            "noscha.io",
            &DnsRecordType::A,
            "93.184.216.34",
            true,
            "bob",
            "2025-06-01",
        );
        assert_eq!(req.record_type, "A");
        assert_eq!(req.name, "bob.noscha.io");
        assert_eq!(req.content, "93.184.216.34");
        assert_eq!(req.ttl, 300);
        assert!(req.proxied);
        assert!(req.comment.contains("bob"));
        assert!(req.comment.contains("2025-06-01"));
    }

    #[test]
    fn test_build_create_request_aaaa() {
        let req = build_create_request(
            "charlie",
            "noscha.io",
            &DnsRecordType::AAAA,
            "2001:db8::1",
            false,
            "charlie",
            "2025-12-31",
        );
        assert_eq!(req.record_type, "AAAA");
        assert_eq!(req.name, "charlie.noscha.io");
        assert_eq!(req.content, "2001:db8::1");
        assert_eq!(req.ttl, 300);
        assert!(!req.proxied);
        assert!(req.comment.contains("charlie"));
        assert!(req.comment.contains("2025-12-31"));
    }

    #[test]
    fn test_create_request_json_serialization() {
        let req = build_create_request(
            "test",
            "noscha.io",
            &DnsRecordType::CNAME,
            "example.com",
            false,
            "test",
            "2025-01-01",
        );
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["type"], "CNAME");
        assert_eq!(json["name"], "test.noscha.io");
        assert_eq!(json["content"], "example.com");
        assert_eq!(json["ttl"], 300);
        assert_eq!(json["proxied"], false);
    }

    #[test]
    fn test_validate_subdomain_valid() {
        assert!(validate_subdomain("alice").is_ok());
        assert!(validate_subdomain("my-site").is_ok());
        assert!(validate_subdomain("abc123").is_ok());
        assert!(validate_subdomain("a").is_ok());
        assert!(validate_subdomain("a-b-c").is_ok());
    }

    #[test]
    fn test_validate_subdomain_empty() {
        assert!(validate_subdomain("").is_err());
    }

    #[test]
    fn test_validate_subdomain_too_long() {
        let long = "a".repeat(64);
        assert!(validate_subdomain(&long).is_err());
        let ok = "a".repeat(63);
        assert!(validate_subdomain(&ok).is_ok());
    }

    #[test]
    fn test_validate_subdomain_no_dots() {
        assert!(validate_subdomain("sub.domain").is_err());
        assert!(validate_subdomain("a.b.c").is_err());
    }

    #[test]
    fn test_validate_subdomain_lowercase() {
        assert!(validate_subdomain("Alice").is_err());
        assert!(validate_subdomain("ABC").is_err());
    }

    #[test]
    fn test_validate_subdomain_no_leading_trailing_hyphen() {
        assert!(validate_subdomain("-abc").is_err());
        assert!(validate_subdomain("abc-").is_err());
    }

    #[test]
    fn test_validate_subdomain_invalid_chars() {
        assert!(validate_subdomain("a_b").is_err());
        assert!(validate_subdomain("a b").is_err());
        assert!(validate_subdomain("a@b").is_err());
    }

    #[test]
    fn test_dns_record_type_display() {
        assert_eq!(DnsRecordType::CNAME.as_str(), "CNAME");
        assert_eq!(DnsRecordType::A.as_str(), "A");
        assert_eq!(DnsRecordType::AAAA.as_str(), "AAAA");
    }

    #[test]
    fn test_dns_record_type_serde() {
        let json = serde_json::to_string(&DnsRecordType::CNAME).unwrap();
        assert_eq!(json, "\"CNAME\"");
        let rt: DnsRecordType = serde_json::from_str("\"A\"").unwrap();
        assert_eq!(rt, DnsRecordType::A);
        let rt: DnsRecordType = serde_json::from_str("\"AAAA\"").unwrap();
        assert_eq!(rt, DnsRecordType::AAAA);
    }
}
