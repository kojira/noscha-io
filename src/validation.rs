/// Reserved usernames that cannot be registered
pub const RESERVED_USERNAMES: &[&str] = &[
    "admin", "www", "mail", "api", "ns1", "ns2", "_dmarc", "autoconfig",
    "postmaster", "abuse", "hostmaster", "webmaster", "ftp", "smtp", "imap",
    "pop", "pop3", "root", "test", "localhost", "noscha",
];

/// Validate username: 3-20 chars, lowercase alphanumeric + hyphen, no leading/trailing hyphen
pub fn validate_username(username: &str) -> Result<(), String> {
    if username.len() < 3 {
        return Err("Username must be at least 3 characters".to_string());
    }
    if username.len() > 20 {
        return Err("Username must be at most 20 characters".to_string());
    }
    if username.starts_with('-') || username.ends_with('-') {
        return Err("Username cannot start or end with a hyphen".to_string());
    }
    if !username.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
        return Err("Username can only contain lowercase letters, digits, and hyphens".to_string());
    }
    if RESERVED_USERNAMES.contains(&username) {
        return Err("This username is reserved".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_usernames() {
        assert!(validate_username("abc").is_ok());
        assert!(validate_username("test-user").is_ok());
        assert!(validate_username("a1b").is_ok());
        assert!(validate_username("aaa").is_ok());
        assert!(validate_username("abcdefghijklmnopqrst").is_ok());
    }

    #[test]
    fn test_too_short() {
        assert!(validate_username("ab").is_err());
        assert!(validate_username("a").is_err());
        assert!(validate_username("").is_err());
    }

    #[test]
    fn test_too_long() {
        assert!(validate_username("abcdefghijklmnopqrstu").is_err());
    }

    #[test]
    fn test_hyphen_boundaries() {
        assert!(validate_username("-abc").is_err());
        assert!(validate_username("abc-").is_err());
    }

    #[test]
    fn test_invalid_chars() {
        assert!(validate_username("ABC").is_err());
        assert!(validate_username("ab@c").is_err());
        assert!(validate_username("ab c").is_err());
    }

    #[test]
    fn test_reserved() {
        assert!(validate_username("admin").is_err());
        assert!(validate_username("www").is_err());
        assert!(validate_username("noscha").is_err());
    }
}
