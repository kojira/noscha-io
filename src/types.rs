use serde::{Deserialize, Serialize};

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
    pub fn amount_sats(&self) -> u64 {
        match self {
            Plan::OneDay => 10,
            Plan::SevenDays => 50,
            Plan::ThirtyDays => 150,
            Plan::NinetyDays => 350,
            Plan::OneYear => 800,
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
}

/// POST /api/order request body
#[derive(Debug, Deserialize)]
pub struct OrderRequest {
    pub username: String,
    pub plan: Plan,
}

/// POST /api/order response
#[derive(Debug, Serialize)]
pub struct OrderResponse {
    pub order_id: String,
    pub amount_sats: u64,
    pub bolt11: String,
    pub expires_at: String,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_amount_sats() {
        assert_eq!(Plan::OneDay.amount_sats(), 10);
        assert_eq!(Plan::SevenDays.amount_sats(), 50);
        assert_eq!(Plan::ThirtyDays.amount_sats(), 150);
        assert_eq!(Plan::NinetyDays.amount_sats(), 350);
        assert_eq!(Plan::OneYear.amount_sats(), 800);
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
}
