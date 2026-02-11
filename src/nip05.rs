#[cfg(target_arch = "wasm32")]
use serde::Serialize;
#[cfg(target_arch = "wasm32")]
use worker::*;

#[cfg(target_arch = "wasm32")]
use crate::types::{is_expired_iso, Rental};

/// Validates a hex-encoded pubkey string: must be exactly 64 chars and all hex digits
pub fn validate_pubkey_hex(s: &str) -> bool {
    if s.len() != 64 {
        return false;
    }
    s.chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(target_arch = "wasm32")]
#[derive(Serialize)]
struct Nip05Response {
    names: std::collections::BTreeMap<String, String>,
    relays: std::collections::BTreeMap<String, Vec<String>>,
}

/// GET /.well-known/nostr.json?name={username}
/// Returns NIP-05 JSON with CORS headers
#[cfg(target_arch = "wasm32")]
pub async fn handle_nip05(
    _req: Request,
    ctx: RouteContext<()>,
) -> Result<Response> {
    // Get the ?name query parameter
    let name = match ctx.param("name") {
        Some(n) => n.clone(),
        None => {
            // Try to get from query string
            let url = _req.url()?;
            match url.query_pairs().find(|(k, _)| k == "name") {
                Some((_, v)) => v.to_string(),
                None => {
                    return Response::error("Missing ?name parameter", 400)
                        .map(|mut res| {
                            let _ = res.headers_mut().set("Access-Control-Allow-Origin", "*");
                            res
                        });
                }
            }
        }
    };

    // Get rental from R2
    let bucket = ctx.env.bucket("BUCKET")?;
    let rental_key = format!("rentals/{}.json", name);
    let obj = bucket.get(&rental_key).execute().await?;

    match obj {
        Some(obj) => {
            let body = obj.body().unwrap();
            let text = body.text().await?;
            let rental: Rental = serde_json::from_str(&text)
                .map_err(|e| Error::RustError(e.to_string()))?;

            if rental.status != "active" || is_expired_iso(&rental.expires_at) {
                return Response::error("Username not found", 404)
                    .map(|mut res| {
                        let _ = res.headers_mut().set("Access-Control-Allow-Origin", "*");
                        res
                    });
            }

            // Check if NIP-05 service is enabled
            if let Some(nip05_svc) = rental.services.nip05.as_ref() {
                if !nip05_svc.enabled {
                    return Response::error("NIP-05 service not enabled for this username", 404)
                        .map(|mut res| {
                            let _ = res.headers_mut().set("Access-Control-Allow-Origin", "*");
                            res
                        });
                }

                // Validate pubkey format
                if !validate_pubkey_hex(&nip05_svc.pubkey_hex) {
                    return Response::error("Invalid pubkey format in rental", 500)
                        .map(|mut res| {
                            let _ = res.headers_mut().set("Access-Control-Allow-Origin", "*");
                            res
                        });
                }

                // Build NIP-05 response
                let mut names = std::collections::BTreeMap::new();
                names.insert(name.clone(), nip05_svc.pubkey_hex.clone());

                let relays = std::collections::BTreeMap::new();

                let response_body = Nip05Response { names, relays };
                let mut response = Response::from_json(&response_body)?;
                response.headers_mut().set("Access-Control-Allow-Origin", "*")?;
                Ok(response)
            } else {
                Response::error("NIP-05 service not provisioned for this username", 404)
                    .map(|mut res| {
                        let _ = res.headers_mut().set("Access-Control-Allow-Origin", "*");
                        res
                    })
            }
        }
        None => Response::error("Username not found", 404)
            .map(|mut res| {
                let _ = res.headers_mut().set("Access-Control-Allow-Origin", "*");
                res
            }),
    }
}

/// OPTIONS /.well-known/nostr.json
/// Handle CORS preflight
#[cfg(target_arch = "wasm32")]
pub async fn handle_nip05_options(_req: Request, _ctx: RouteContext<()>) -> Result<Response> {
    let mut response = Response::ok("")?;
    response.headers_mut().set("Access-Control-Allow-Origin", "*")?;
    response.headers_mut().set("Access-Control-Allow-Methods", "GET, OPTIONS")?;
    response.headers_mut().set("Access-Control-Allow-Headers", "Content-Type")?;
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_pubkey_hex_valid() {
        // Valid 64-char hex string
        let valid = "a".repeat(64);
        assert!(validate_pubkey_hex(&valid));

        let valid2 = "1234567890abcdef".repeat(4);
        assert!(validate_pubkey_hex(&valid2));
    }

    #[test]
    fn test_validate_pubkey_hex_too_short() {
        assert!(!validate_pubkey_hex("abc"));
    }

    #[test]
    fn test_validate_pubkey_hex_too_long() {
        let long = "a".repeat(65);
        assert!(!validate_pubkey_hex(&long));
    }

    #[test]
    fn test_validate_pubkey_hex_invalid_chars() {
        let invalid = "z".repeat(64);
        assert!(!validate_pubkey_hex(&invalid));

        let invalid2 = format!("{}{}x", "a".repeat(63), "a");
        assert!(!validate_pubkey_hex(&invalid2));
    }

    #[test]
    fn test_validate_pubkey_hex_mixed_case() {
        let mixed = format!("{}{}", "A".repeat(32), "f".repeat(32));
        assert!(validate_pubkey_hex(&mixed));
    }
}
