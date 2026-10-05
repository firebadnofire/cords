//! Explicit user-requested image imports. No session credentials or `WebView` network access.
use base64::{Engine as _, engine::general_purpose::STANDARD};

#[tauri::command]
pub(crate) async fn load_image_url(url: String) -> Result<String, String> {
    let url = reqwest::Url::parse(&url).map_err(|_| "Enter a valid HTTPS image URL")?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return Err("Image URLs must use HTTPS without embedded credentials".into());
    }
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|_| "Image loader initialization failed")?;
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|_| "Image request failed; check URL and certificate")?;
    if !response.status().is_success() {
        return Err(
            "Image host rejected the request or redirected it; use the final HTTPS URL".into(),
        );
    }
    let limit = 5 * 1024 * 1024;
    if response
        .content_length()
        .is_some_and(|length| length > limit)
    {
        return Err("Image exceeds the 5 MiB limit".into());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "Image transfer failed")?
    {
        if bytes.len() + chunk.len() > usize::try_from(limit).map_err(|_| "Image limit invalid")? {
            return Err("Image exceeds the 5 MiB limit".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(STANDARD.encode(bytes))
}
