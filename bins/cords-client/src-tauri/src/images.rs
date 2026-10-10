//! Explicit user-requested image imports. No session credentials or `WebView` network access.
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::net::{IpAddr, SocketAddr};

fn public_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(ip) => {
            !(ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_documentation()
                || ip.is_unspecified()
                || ip.is_multicast())
        }
        IpAddr::V6(ip) => ip.to_ipv4_mapped().map_or_else(
            || {
                !(ip.is_loopback()
                    || ip.is_unspecified()
                    || ip.is_multicast()
                    || ip.is_unique_local()
                    || ip.is_unicast_link_local())
            },
            |mapped| public_address(IpAddr::V4(mapped)),
        ),
    }
}

#[tauri::command]
pub(crate) async fn load_image_url(url: String) -> Result<String, String> {
    let url = reqwest::Url::parse(&url).map_err(|_| "Enter a valid HTTPS image URL")?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return Err("Image URLs must use HTTPS without embedded credentials".into());
    }
    let host = url
        .host_str()
        .ok_or("Image URL must include a host")?
        .to_string();
    let port = url
        .port_or_known_default()
        .ok_or("Image URL port is invalid")?;
    let addresses: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), port))
        .await
        .map_err(|_| "Image host could not be resolved")?
        .collect();
    if addresses.is_empty()
        || addresses
            .iter()
            .any(|address| !public_address(address.ip()))
    {
        return Err(
            "Image hosts must not resolve to local, private, or special-use addresses".into(),
        );
    }
    let pinned = addresses[0];
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(15))
        .resolve(&host, pinned)
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
    if !response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value.split(';').next().is_some_and(|mime| {
                matches!(
                    mime.trim(),
                    "image/png" | "image/jpeg" | "image/webp" | "image/svg+xml"
                )
            })
        })
    {
        return Err("Image host returned an unsupported content type".into());
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

#[cfg(test)]
mod tests {
    use super::public_address;
    use std::net::IpAddr;

    #[test]
    fn image_hosts_reject_local_and_private_addresses() -> Result<(), std::net::AddrParseError> {
        for address in [
            "127.0.0.1",
            "10.0.0.1",
            "169.254.1.1",
            "::1",
            "fd00::1",
            "fe80::1",
            "::ffff:127.0.0.1",
        ] {
            assert!(!public_address(address.parse::<IpAddr>()?));
        }
        assert!(public_address("1.1.1.1".parse()?));
        assert!(public_address("2606:4700:4700::1111".parse()?));
        Ok(())
    }
}
