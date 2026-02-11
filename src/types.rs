use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Service types that can be individually selected
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ServiceType {
    Subdomain,
    EmailForwarding,
    Nip05,
}

/// Supported rental plans with pricing in sats
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Plan {
    #[serde(rename = "1d")]
    OneDay,
    #[serde(rename = "7d")]
    SevenDays,
    #[serde(rename = "30d")]
    ThirtyDays,
    #[serde(rename = "90d")]
    NinetyDays,
    #[serde(rename = "365d")]
    OneYear,
}

impl Plan {
    /// Price for a single service type
    pub fn service_price(&self, service: &ServiceType) -> u64 {
        match (self, service) {
            (Plan::OneDay, ServiceType::Subdomain) => 500,
            (Plan::OneDay, ServiceType::EmailForwarding) => 1500,
            (Plan::OneDay, ServiceType::Nip05) => 200,
            (Plan::SevenDays, ServiceType::Subdomain) => 1000,
            (Plan::SevenDays, ServiceType::EmailForwarding) => 2500,
            (Plan::SevenDays, ServiceType::Nip05) => 500,
            (Plan::ThirtyDays, ServiceType::Subdomain) => 2000,
            (Plan::ThirtyDays, ServiceType::EmailForwarding) => 5000,
            (Plan::ThirtyDays, ServiceType::Nip05) => 1000,
            (Plan::NinetyDays, ServiceType::Subdomain) => 5000,
            (Plan::NinetyDays, ServiceType::EmailForwarding) => 12000,
            (Plan::NinetyDays, ServiceType::Nip05) => 2500,
            (Plan::OneYear, ServiceType::Subdomain) => 15000,
            (Plan::OneYear, ServiceType::EmailForwarding) => 40000,
            (Plan::OneYear, ServiceType::Nip05) => 8000,
        }
    }

    /// Bundle price when all 3 services are selected
    pub fn bundle_price(&self) -> u64 {
        match self {
            Plan::OneDay => 1800,
            Plan::SevenDays => 3300,
            Plan::ThirtyDays => 6500,
            Plan::NinetyDays => 16000,
            Plan::OneYear => 50000,
        }
    }

    /// Calculate total price based on selected services
    pub fn calculate_total(plan: &Plan, services: &[ServiceType]) -> u64 {
        let unique: HashSet<&ServiceType> = services.iter().collect();
        if unique.len() == 3 {
            plan.bundle_price()
        } else {
            unique.iter().map(|s| plan.service_price(s)).sum()
        }
    }

    pub fn period_key(&self) -> &'static str {
        match self {
            Plan::OneDay => "1d",
            Plan::SevenDays => "7d",
            Plan::ThirtyDays => "30d",
            Plan::NinetyDays => "90d",
            Plan::OneYear => "365d",
        }
    }

    pub fn service_price_dynamic(&self, service: &ServiceType, pricing: &PricingConfig) -> u64 {
        let key = self.period_key();
        let svc_key = match service {
            ServiceType::Subdomain => "subdomain",
            ServiceType::EmailForwarding => "email",
            ServiceType::Nip05 => "nip05",
        };
        pricing.get(key).and_then(|m| m.get(svc_key)).copied().unwrap_or_else(|| self.service_price(service))
    }

    pub fn bundle_price_dynamic(&self, pricing: &PricingConfig) -> u64 {
        let key = self.period_key();
        pricing.get(key).and_then(|m| m.get("bundle")).copied().unwrap_or_else(|| self.bundle_price())
    }

    pub fn calculate_total_dynamic(plan: &Plan, services: &[ServiceType], pricing: &PricingConfig) -> u64 {
        let unique: HashSet<&ServiceType> = services.iter().collect();
        if unique.len() == 3 {
            plan.bundle_price_dynamic(pricing)
        } else {
            unique.iter().map(|s| plan.service_price_dynamic(s, pricing)).sum()
        }
    }

    pub fn duration_days(&self) -> u64 {
        match self {
            Plan::OneDay => 1,
            Plan::SevenDays => 7,
            Plan::ThirtyDays => 30,
            Plan::NinetyDays => 90,
            Plan::OneYear => 365,
        }
    }
}

/// Order status lifecycle
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum OrderStatus {
    Pending,
    Paid,
    Provisioned,
    Expired,
}

/// Order stored in R2 at orders/{order_id}.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub order_id: String,
    pub username: String,
    pub plan: Plan,
    pub amount_sats: u64,
    pub bolt11: String,
    pub status: OrderStatus,
    pub created_at: String,
    pub expires_at: String,
    /// Coinos invoice hash for webhook verification
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coinos_invoice_hash: Option<String>,
    /// Webhook secret for this order
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhook_secret: Option<String>,
    /// Services requested in this order (for provisioning after payment)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub services_requested: Option<OrderServicesRequest>,
    /// Management token for user self-service
    #[serde(skip_serializing_if = "Option::is_none")]
    pub management_token: Option<String>,
    /// If set, this order is a renewal for an existing rental
    #[serde(skip_serializing_if = "Option::is_none")]
    pub renewal_for: Option<String>,
}

/// Services requested in an order
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OrderServicesRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<OrderEmailRequest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subdomain: Option<OrderSubdomainRequest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nip05: Option<OrderNip05Request>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderEmailRequest {
    pub forward_to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderSubdomainRequest {
    #[serde(rename = "type")]
    pub record_type: String,
    pub target: String,
    #[serde(default)]
    pub proxied: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderNip05Request {
    pub pubkey: String,
}

/// POST /api/order request body
#[derive(Debug, Deserialize)]
pub struct OrderRequest {
    pub username: String,
    pub plan: Plan,
    #[serde(default)]
    pub services: Option<OrderServicesRequest>,
}

/// POST /api/order response
#[derive(Debug, Serialize)]
pub struct OrderResponse {
    pub order_id: String,
    pub amount_sats: u64,
    pub bolt11: String,
    pub expires_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub management_token: Option<String>,
}

/// GET /api/check/{username} response
#[derive(Debug, Serialize)]
pub struct CheckUsernameResponse {
    pub available: bool,
    pub username: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// GET /api/order/{order_id}/status response
#[derive(Debug, Serialize)]
pub struct OrderStatusResponse {
    pub order_id: String,
    pub status: OrderStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub management_token: Option<String>,
}

/// Subdomain service configuration stored in rental
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubdomainService {
    pub enabled: bool,
    #[serde(rename = "type")]
    pub record_type: String,
    pub target: String,
    #[serde(default)]
    pub proxied: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cf_record_id: Option<String>,
}

/// Email service configuration stored in rental
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailService {
    pub enabled: bool,
    pub forward_to: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cf_rule_id: Option<String>,
}

/// NIP-05 service configuration stored in rental
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Nip05Service {
    pub enabled: bool,
    pub pubkey_hex: String,
    #[serde(default)]
    pub relays: Vec<String>,
}

/// Services configured for a rental
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RentalServices {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<EmailService>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subdomain: Option<SubdomainService>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nip05: Option<Nip05Service>,
}

/// Rental object stored in R2 at rentals/{username}.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rental {
    pub username: String,
    pub status: String,
    pub created_at: String,
    pub expires_at: String,
    pub plan: Plan,
    pub services: RentalServices,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub management_token: Option<String>,
}

/// POST /api/renew request body
#[derive(Debug, Deserialize)]
pub struct RenewRequest {
    pub management_token: String,
    pub plan: Plan,
    #[serde(default)]
    pub services: Option<OrderServicesRequest>,
}

/// POST /api/renew response
#[derive(Debug, Serialize)]
pub struct RenewResponse {
    pub order_id: String,
    pub amount_sats: u64,
    pub bolt11: String,
    pub expires_at: String,
}

/// Coinos webhook payload
#[derive(Debug, Deserialize)]
pub struct CoinosWebhookPayload {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub amount: Option<u64>,
    #[serde(default)]
    pub hash: Option<String>,
    #[serde(default)]
    pub confirmed: Option<bool>,
    #[serde(default)]
    pub secret: Option<String>,
}

/// Dynamic pricing config stored in R2 at config/pricing.json
/// Format: {"1d":{"subdomain":500,"email":1500,"nip05":200,"bundle":1800}, ...}
pub type PricingConfig = HashMap<String, HashMap<String, u64>>;

/// Get default pricing config
pub fn default_pricing() -> PricingConfig {
    let mut config = HashMap::new();
    let periods: [(&str, [(&str, u64); 4]); 5] = [
        ("1d", [("subdomain",500),("email",1500),("nip05",200),("bundle",1800)]),
        ("7d", [("subdomain",1000),("email",2500),("nip05",500),("bundle",3300)]),
        ("30d", [("subdomain",2000),("email",5000),("nip05",1000),("bundle",6500)]),
        ("90d", [("subdomain",5000),("email",12000),("nip05",2500),("bundle",16000)]),
        ("365d", [("subdomain",15000),("email",40000),("nip05",8000),("bundle",50000)]),
    ];
    for (period, prices) in periods {
        let mut m = HashMap::new();
        for (svc, price) in prices {
            m.insert(svc.to_string(), price);
        }
        config.insert(period.to_string(), m);
    }
    config
}

/// Admin session stored in R2 at sessions/{token}.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminSession {
    pub token: String,
    pub pubkey: String,
    pub created_at: String,
    pub expires_at: String,
}

/// Admin challenge stored in R2 at challenges/{challenge}.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminChallenge {
    pub challenge: String,
    pub created_at: String,
    pub expires_at: String,
}

/// Nostr event from NIP-07 signing (simplified, no crypto verification)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NostrEvent {
    #[serde(default)]
    pub id: Option<String>,
    pub pubkey: String,
    #[serde(default)]
    pub created_at: Option<u64>,
    #[serde(default)]
    pub kind: Option<u32>,
    #[serde(default)]
    pub tags: Option<Vec<Vec<String>>>,
    pub content: String,
    #[serde(default)]
    pub sig: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_price() {
        assert_eq!(Plan::OneDay.service_price(&ServiceType::Subdomain), 500);
        assert_eq!(Plan::OneDay.service_price(&ServiceType::EmailForwarding), 1500);
        assert_eq!(Plan::OneDay.service_price(&ServiceType::Nip05), 200);
        assert_eq!(Plan::ThirtyDays.service_price(&ServiceType::Subdomain), 2000);
        assert_eq!(Plan::ThirtyDays.service_price(&ServiceType::EmailForwarding), 5000);
        assert_eq!(Plan::ThirtyDays.service_price(&ServiceType::Nip05), 1000);
        assert_eq!(Plan::OneYear.service_price(&ServiceType::Subdomain), 15000);
        assert_eq!(Plan::OneYear.service_price(&ServiceType::EmailForwarding), 40000);
        assert_eq!(Plan::OneYear.service_price(&ServiceType::Nip05), 8000);
    }

    #[test]
    fn test_bundle_price() {
        assert_eq!(Plan::OneDay.bundle_price(), 1800);
        assert_eq!(Plan::SevenDays.bundle_price(), 3300);
        assert_eq!(Plan::ThirtyDays.bundle_price(), 6500);
        assert_eq!(Plan::NinetyDays.bundle_price(), 16000);
        assert_eq!(Plan::OneYear.bundle_price(), 50000);
    }

    #[test]
    fn test_calculate_total_single_service() {
        let services = vec![ServiceType::Subdomain];
        assert_eq!(Plan::calculate_total(&Plan::ThirtyDays, &services), 2000);
    }

    #[test]
    fn test_calculate_total_two_services() {
        let services = vec![ServiceType::Subdomain, ServiceType::Nip05];
        assert_eq!(Plan::calculate_total(&Plan::ThirtyDays, &services), 3000);
    }

    #[test]
    fn test_calculate_total_bundle() {
        let services = vec![ServiceType::Subdomain, ServiceType::EmailForwarding, ServiceType::Nip05];
        assert_eq!(Plan::calculate_total(&Plan::ThirtyDays, &services), 6500);
        // Bundle price (6500) < sum of individual (2000+5000+1000=8000)
    }

    #[test]
    fn test_plan_duration_days() {
        assert_eq!(Plan::OneDay.duration_days(), 1);
        assert_eq!(Plan::SevenDays.duration_days(), 7);
        assert_eq!(Plan::ThirtyDays.duration_days(), 30);
        assert_eq!(Plan::NinetyDays.duration_days(), 90);
        assert_eq!(Plan::OneYear.duration_days(), 365);
    }

    #[test]
    fn test_plan_serde() {
        let json = serde_json::to_string(&Plan::OneDay).unwrap();
        assert_eq!(json, "\"1d\"");
        let plan: Plan = serde_json::from_str("\"30d\"").unwrap();
        assert_eq!(plan, Plan::ThirtyDays);
    }

    #[test]
    fn test_order_status_serde() {
        let json = serde_json::to_string(&OrderStatus::Pending).unwrap();
        assert_eq!(json, "\"pending\"");
        let status: OrderStatus = serde_json::from_str("\"paid\"").unwrap();
        assert_eq!(status, OrderStatus::Paid);
    }

    #[test]
    fn test_pricing_config_default() {
        let config = default_pricing();
        assert_eq!(*config.get("30d").unwrap().get("bundle").unwrap(), 6500);
        assert_eq!(*config.get("90d").unwrap().get("bundle").unwrap(), 16000);
        assert_eq!(*config.get("365d").unwrap().get("bundle").unwrap(), 50000);
    }

    #[test]
    fn test_pricing_config_serde() {
        let json = r#"{"30d":{"subdomain":2000,"email":5000,"nip05":1000,"bundle":5000},"90d":{"subdomain":5000,"email":12000,"nip05":2500,"bundle":12000}}"#;
        let config: PricingConfig = serde_json::from_str(json).unwrap();
        assert_eq!(*config.get("30d").unwrap().get("bundle").unwrap(), 5000);
        assert_eq!(*config.get("90d").unwrap().get("bundle").unwrap(), 12000);
    }

    #[test]
    fn test_nostr_event_serde() {
        let event = NostrEvent {
            id: None,
            pubkey: "abc".into(),
            created_at: None,
            kind: Some(27235),
            tags: None,
            content: "hello".into(),
            sig: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: NostrEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.pubkey, "abc");
        assert_eq!(parsed.kind, Some(27235));
    }

    #[test]
    fn test_admin_session_serde() {
        let session = AdminSession {
            token: "sess_abc".into(),
            pubkey: "deadbeef".into(),
            created_at: "2025-01-01T00:00:00Z".into(),
            expires_at: "2025-01-02T00:00:00Z".into(),
        };
        let json = serde_json::to_string(&session).unwrap();
        let parsed: AdminSession = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.token, "sess_abc");
    }
}
