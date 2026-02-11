use crate::types::Rental;

/// Validate an email address (basic format check)
pub fn validate_email(email: &str) -> Result<(), String> {
    if email.is_empty() {
        return Err("Email address cannot be empty".to_string());
    }
    if email.len() > 254 {
        return Err("Email address is too long".to_string());
    }

    let parts: Vec<&str> = email.splitn(2, '@').collect();
    if parts.len() != 2 {
        return Err("Email must contain exactly one @ symbol".to_string());
    }

    let local = parts[0];
    let domain = parts[1];

    if local.is_empty() {
        return Err("Email local part cannot be empty".to_string());
    }
    if local.len() > 64 {
        return Err("Email local part is too long".to_string());
    }
    if domain.is_empty() {
        return Err("Email domain cannot be empty".to_string());
    }
    if !domain.contains('.') {
        return Err("Email domain must contain a dot".to_string());
    }
    if domain.starts_with('.') || domain.ends_with('.') {
        return Err("Email domain cannot start or end with a dot".to_string());
    }
    if domain.contains("..") {
        return Err("Email domain cannot contain consecutive dots".to_string());
    }

    Ok(())
}

/// Extract the username portion from a recipient email address for the given domain
pub fn extract_username(recipient: &str, domain: &str) -> Option<String> {
    let suffix = format!("@{}", domain);
    let lower = recipient.to_lowercase();
    if lower.ends_with(&suffix.to_lowercase()) {
        let local = &lower[..lower.len() - suffix.len()];
        if local.is_empty() {
            None
        } else {
            Some(local.to_string())
        }
    } else {
        None
    }
}

/// Look up the forwarding address from a rental if email is enabled and the rental is active
pub fn lookup_forward_address(rental: &Rental) -> Option<String> {
    if rental.status != "active" {
        return None;
    }
    rental.services.email.as_ref().and_then(|email_svc| {
        if email_svc.enabled && !email_svc.forward_to.is_empty() {
            Some(email_svc.forward_to.clone())
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    #[test]
    fn test_validate_email_valid() {
        assert!(validate_email("alice@example.com").is_ok());
        assert!(validate_email("user.name@domain.co.jp").is_ok());
        assert!(validate_email("test+tag@mail.example.org").is_ok());
        assert!(validate_email("a@b.co").is_ok());
    }

    #[test]
    fn test_validate_email_empty() {
        assert!(validate_email("").is_err());
    }

    #[test]
    fn test_validate_email_no_at() {
        assert!(validate_email("userexample.com").is_err());
    }

    #[test]
    fn test_validate_email_no_domain_dot() {
        assert!(validate_email("user@localhost").is_err());
    }

    #[test]
    fn test_validate_email_empty_local() {
        assert!(validate_email("@example.com").is_err());
    }

    #[test]
    fn test_validate_email_empty_domain() {
        assert!(validate_email("user@").is_err());
    }

    #[test]
    fn test_validate_email_domain_leading_dot() {
        assert!(validate_email("user@.example.com").is_err());
    }

    #[test]
    fn test_validate_email_domain_trailing_dot() {
        assert!(validate_email("user@example.com.").is_err());
    }

    #[test]
    fn test_validate_email_domain_consecutive_dots() {
        assert!(validate_email("user@example..com").is_err());
    }

    #[test]
    fn test_validate_email_too_long() {
        let long_local = "a".repeat(65);
        assert!(validate_email(&format!("{}@example.com", long_local)).is_err());
    }

    #[test]
    fn test_extract_username_valid() {
        assert_eq!(
            extract_username("alice@noscha.io", "noscha.io"),
            Some("alice".to_string())
        );
    }

    #[test]
    fn test_extract_username_case_insensitive() {
        assert_eq!(
            extract_username("Alice@Noscha.IO", "noscha.io"),
            Some("alice".to_string())
        );
    }

    #[test]
    fn test_extract_username_wrong_domain() {
        assert_eq!(extract_username("alice@other.com", "noscha.io"), None);
    }

    #[test]
    fn test_extract_username_empty_local() {
        assert_eq!(extract_username("@noscha.io", "noscha.io"), None);
    }

    fn make_rental(status: &str, email_enabled: bool, forward_to: &str) -> Rental {
        Rental {
            username: "alice".to_string(),
            status: status.to_string(),
            created_at: "2025-01-01T00:00:00Z".to_string(),
            expires_at: "2025-02-01T00:00:00Z".to_string(),
            plan: Plan::ThirtyDays,
            services: RentalServices {
                email: Some(EmailService {
                    enabled: email_enabled,
                    forward_to: forward_to.to_string(),
                    cf_rule_id: None,
                }),
                subdomain: None,
                nip05: None,
            },
        }
    }

    #[test]
    fn test_lookup_forward_active_enabled() {
        let rental = make_rental("active", true, "alice@example.com");
        assert_eq!(
            lookup_forward_address(&rental),
            Some("alice@example.com".to_string())
        );
    }

    #[test]
    fn test_lookup_forward_expired() {
        let rental = make_rental("expired", true, "alice@example.com");
        assert_eq!(lookup_forward_address(&rental), None);
    }

    #[test]
    fn test_lookup_forward_disabled() {
        let rental = make_rental("active", false, "alice@example.com");
        assert_eq!(lookup_forward_address(&rental), None);
    }

    #[test]
    fn test_lookup_forward_empty_address() {
        let rental = make_rental("active", true, "");
        assert_eq!(lookup_forward_address(&rental), None);
    }

    #[test]
    fn test_lookup_forward_no_email_service() {
        let rental = Rental {
            username: "bob".to_string(),
            status: "active".to_string(),
            created_at: "2025-01-01T00:00:00Z".to_string(),
            expires_at: "2025-02-01T00:00:00Z".to_string(),
            plan: Plan::SevenDays,
            services: RentalServices {
                email: None,
                subdomain: None,
                nip05: None,
            },
        };
        assert_eq!(lookup_forward_address(&rental), None);
    }
}
