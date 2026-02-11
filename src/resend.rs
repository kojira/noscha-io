use worker::*;

/// Send an email via Resend API
pub async fn send_email(
    api_key: &str,
    from: &str,
    to: &str,
    subject: &str,
    html: Option<&str>,
    text: Option<&str>,
) -> Result<()> {
    let mut body = serde_json::json!({
        "from": from,
        "to": [to],
        "subject": subject,
    });

    if let Some(html_content) = html {
        body["html"] = serde_json::json!(html_content);
    }
    if let Some(text_content) = text {
        body["text"] = serde_json::json!(text_content);
    }

    let headers = Headers::new();
    headers.set("Authorization", &format!("Bearer {}", api_key))?;
    headers.set("Content-Type", "application/json")?;

    let request = Request::new_with_init(
        "https://api.resend.com/emails",
        RequestInit::new()
            .with_method(Method::Post)
            .with_headers(headers)
            .with_body(Some(wasm_bindgen::JsValue::from_str(&body.to_string()))),
    )?;

    let mut response = Fetch::Request(request).send().await?;
    if response.status_code() >= 400 {
        let err_text = response.text().await.unwrap_or_default();
        return Err(Error::RustError(format!(
            "Resend API error ({}): {}",
            response.status_code(),
            err_text
        )));
    }

    Ok(())
}
