use hmac::{Hmac, Mac};
use reqwest::Client;
use sha2::{Digest, Sha256};
use std::path::Path;
use work_review_core::config::{
    RemoteStorageConfig, RemoteStorageProvider, S3Config, WebDavConfig,
};
use work_review_core::error::{AppError, Result};

type HmacSha256 = Hmac<Sha256>;

pub async fn upload_screenshot(
    client: &Client,
    config: &RemoteStorageConfig,
    local_path: &Path,
    relative_path: &str,
) -> Result<String> {
    let file_bytes = tokio::fs::read(local_path)
        .await
        .map_err(|e| AppError::Screenshot(format!("读取截图文件失败: {e}")))?;

    match config.provider {
        RemoteStorageProvider::S3 => {
            upload_s3(client, &config.s3, &file_bytes, relative_path).await
        }
        RemoteStorageProvider::WebDav => {
            upload_webdav(client, &config.webdav, &file_bytes, relative_path).await
        }
        RemoteStorageProvider::None => Err(AppError::Config("远程存储未配置".into())),
    }
}

// --- S3 (MinIO compatible) with hand-crafted SigV4 ---

async fn upload_s3(
    client: &Client,
    config: &S3Config,
    file_bytes: &[u8],
    relative_path: &str,
) -> Result<String> {
    let endpoint = config.endpoint.trim_end_matches('/');
    let object_key = remote_object_path(&config.path_prefix, relative_path);

    let url = format!("{}/{}/{}", endpoint, &config.bucket, &object_key);
    let parsed =
        reqwest::Url::parse(&url).map_err(|e| AppError::Config(format!("S3 URL 解析失败: {e}")))?;
    let host = parsed
        .host_str()
        .ok_or_else(|| AppError::Config("S3 endpoint 缺少 host".into()))?;
    let host_with_port = if let Some(port) = parsed.port() {
        format!("{host}:{port}")
    } else {
        host.to_string()
    };

    let now = chrono::Utc::now();
    let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
    let date_stamp = now.format("%Y%m%d").to_string();

    let payload_hash = hex::encode(Sha256::digest(file_bytes));
    // Content-Type 按扩展名推导；同时参与 SigV4 签名，签名与实际请求头必须一致
    let content_type = content_type_for_extension(&object_key);

    let canonical_uri = format!("/{}/{}", &config.bucket, url_encode_path(&object_key));
    let canonical_querystring = "";

    let canonical_headers = format!(
        "content-type:{content_type}\nhost:{host_with_port}\nx-amz-content-sha256:{payload_hash}\nx-amz-date:{amz_date}\n"
    );
    let signed_headers = "content-type;host;x-amz-content-sha256;x-amz-date";

    let canonical_request = format!(
        "PUT\n{canonical_uri}\n{canonical_querystring}\n{canonical_headers}\n{signed_headers}\n{payload_hash}"
    );

    let credential_scope = format!("{}/{}/s3/aws4_request", date_stamp, config.region);
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{}\n{}\n{}",
        amz_date,
        credential_scope,
        hex::encode(Sha256::digest(canonical_request.as_bytes()))
    );

    let signing_key = derive_signing_key(&config.secret_key, &date_stamp, &config.region, "s3");
    let signature = hex::encode(hmac_sha256(&signing_key, string_to_sign.as_bytes()));

    let authorization = format!(
        "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
        config.access_key, credential_scope, signed_headers, signature
    );

    let resp = client
        .put(&url)
        .header("Content-Type", content_type)
        .header("Host", &host_with_port)
        .header("x-amz-content-sha256", &payload_hash)
        .header("x-amz-date", &amz_date)
        .header("Authorization", &authorization)
        .body(file_bytes.to_vec())
        .send()
        .await
        .map_err(|e| AppError::Screenshot(format!("S3 PUT 请求失败: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        let body_preview = body.chars().take(500).collect::<String>();
        return Err(AppError::Screenshot(format!(
            "S3 PUT 返回 {}: {}",
            status, body_preview
        )));
    }

    let public_url = public_url_or_fallback(config.public_url_base.as_deref(), &object_key, &url);

    Ok(public_url)
}

fn derive_signing_key(secret_key: &str, date_stamp: &str, region: &str, service: &str) -> Vec<u8> {
    let k_date = hmac_sha256(
        format!("AWS4{secret_key}").as_bytes(),
        date_stamp.as_bytes(),
    );
    let k_region = hmac_sha256(&k_date, region.as_bytes());
    let k_service = hmac_sha256(&k_region, service.as_bytes());
    hmac_sha256(&k_service, b"aws4_request")
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can take key of any size");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

/// 依据文件扩展名推导 Content-Type（截图上传目前只涉及 jpg/png）。
fn content_type_for_extension(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        _ => "application/octet-stream",
    }
}

fn url_encode_path(path: &str) -> String {
    path.split('/')
        .map(|segment| {
            segment
                .bytes()
                .map(|b| {
                    if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~'
                    {
                        String::from(b as char)
                    } else {
                        format!("%{b:02X}")
                    }
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("/")
}

// --- WebDAV ---

/// 判断主机名是否为本机/内网地址（NAS 等本地部署允许走 http）。
fn is_private_or_local_host(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    if host == "localhost" || host.ends_with(".localhost") {
        return true;
    }
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        return match ip {
            std::net::IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
            std::net::IpAddr::V6(v6) => v6.is_loopback(),
        };
    }
    false
}

/// WebDAV 端点安全校验：远程端点必须使用 https，明文 http 仅允许本机/内网地址
/// （与模型端点策略一致），防止截图与凭据经明文链路发往远程服务器。
fn ensure_webdav_endpoint_allowed(url: &str) -> Result<()> {
    let lower = url.trim().to_ascii_lowercase();
    if lower.starts_with("https://") {
        return Ok(());
    }
    let Some(rest) = lower.strip_prefix("http://") else {
        return Err(AppError::Config(
            "WebDAV 地址必须以 http:// 或 https:// 开头".to_string(),
        ));
    };

    // 提取主机名：截到 path/query/fragment 之前，剥离端口；IPv6 形如 [::1]:5005
    let host_port = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = if let Some(inner) = host_port.strip_prefix('[') {
        inner.split(']').next().unwrap_or("")
    } else {
        host_port.split(':').next().unwrap_or(host_port)
    };

    if is_private_or_local_host(host) {
        return Ok(());
    }
    Err(AppError::Config(
        "远程 WebDAV 端点必须使用 https（本机/内网地址除外）".to_string(),
    ))
}

async fn upload_webdav(
    client: &Client,
    config: &WebDavConfig,
    file_bytes: &[u8],
    relative_path: &str,
) -> Result<String> {
    ensure_webdav_endpoint_allowed(&config.url)?;
    let base = config.url.trim_end_matches('/');
    let object_path = remote_object_path(&config.path_prefix, relative_path);

    ensure_webdav_directories(
        client,
        base,
        &object_path,
        &config.username,
        &config.password,
    )
    .await?;

    let put_url = format!("{}/{}", base, &object_path);
    let resp = client
        .put(&put_url)
        .basic_auth(&config.username, Some(&config.password))
        .header("Content-Type", "image/jpeg")
        .body(file_bytes.to_vec())
        .send()
        .await
        .map_err(|e| AppError::Screenshot(format!("WebDAV PUT 失败: {e}")))?;

    let status = resp.status().as_u16();
    if !resp.status().is_success() && status != 201 && status != 204 {
        let body = resp.text().await.unwrap_or_default();
        let body_preview = body.chars().take(500).collect::<String>();
        return Err(AppError::Screenshot(format!(
            "WebDAV PUT 返回 {}: {}",
            status, body_preview
        )));
    }

    let public_url =
        public_url_or_fallback(config.public_url_base.as_deref(), &object_path, &put_url);

    Ok(public_url)
}

async fn ensure_webdav_directories(
    client: &Client,
    base_url: &str,
    object_path: &str,
    username: &str,
    password: &str,
) -> Result<()> {
    let parts: Vec<&str> = object_path.split('/').collect();
    let dir_parts = &parts[..parts.len().saturating_sub(1)];

    let mut current = String::new();
    for part in dir_parts {
        if part.is_empty() {
            continue;
        }
        if !current.is_empty() {
            current.push('/');
        }
        current.push_str(part);

        let mkcol_url = format!("{}/{}/", base_url, &current);
        let mkcol_method = reqwest::Method::from_bytes(b"MKCOL")
            .map_err(|e| AppError::Screenshot(format!("MKCOL method: {e}")))?;

        match client
            .request(mkcol_method, &mkcol_url)
            .basic_auth(username, Some(password))
            .send()
            .await
        {
            Ok(r) if r.status().is_success() || r.status().as_u16() == 405 => {}
            Ok(r) => log::debug!("MKCOL {} 返回 {}", mkcol_url, r.status()),
            Err(e) => log::debug!("MKCOL {mkcol_url} 失败: {e}"),
        }
    }
    Ok(())
}

fn remote_object_path(prefix: &str, relative_path: &str) -> String {
    let relative_path = relative_path.replace('\\', "/");
    let prefix = prefix.trim().trim_matches('/');
    if prefix.is_empty() {
        relative_path
    } else {
        format!("{prefix}/{relative_path}")
    }
}

fn public_url_or_fallback(base_url: Option<&str>, object_path: &str, fallback: &str) -> String {
    let Some(base_url) = base_url
        .map(str::trim)
        .filter(|base_url| !base_url.is_empty())
    else {
        return fallback.to_string();
    };
    format!("{}/{}", base_url.trim_end_matches('/'), object_path)
}

#[cfg(test)]
mod tests {
    use super::{
        content_type_for_extension, ensure_webdav_endpoint_allowed, public_url_or_fallback,
        remote_object_path,
    };

    #[test]
    fn 内容类型应按扩展名推导() {
        assert_eq!(content_type_for_extension("a/b/shot.jpg"), "image/jpeg");
        assert_eq!(content_type_for_extension("shot.JPEG"), "image/jpeg");
        assert_eq!(content_type_for_extension("shot.png"), "image/png");
        assert_eq!(
            content_type_for_extension("shot.webp"),
            "application/octet-stream"
        );
        assert_eq!(
            content_type_for_extension("noext"),
            "application/octet-stream"
        );
    }

    #[test]
    fn webdav端点应拒绝远程明文http仅放行本机内网() {
        assert!(ensure_webdav_endpoint_allowed("https://dav.example.com/dav").is_ok());
        assert!(ensure_webdav_endpoint_allowed("http://localhost:5005/dav").is_ok());
        assert!(ensure_webdav_endpoint_allowed("http://127.0.0.1/dav").is_ok());
        assert!(ensure_webdav_endpoint_allowed("http://[::1]:5005/dav").is_ok());
        assert!(ensure_webdav_endpoint_allowed("http://192.168.1.20:5005/dav").is_ok());
        assert!(ensure_webdav_endpoint_allowed("http://10.0.0.8/dav").is_ok());
        assert!(ensure_webdav_endpoint_allowed("http://172.16.0.2/dav").is_ok());

        assert!(ensure_webdav_endpoint_allowed("http://dav.example.com/dav").is_err());
        assert!(ensure_webdav_endpoint_allowed("http://8.8.8.8/dav").is_err());
        assert!(ensure_webdav_endpoint_allowed("ftp://dav.example.com").is_err());
    }

    #[test]
    fn 远程对象路径应包含路径前缀并统一分隔符() {
        assert_eq!(
            remote_object_path(" workreview/ ", r"screenshots\2026-05-22\shot.jpg"),
            "workreview/screenshots/2026-05-22/shot.jpg"
        );
        assert_eq!(
            remote_object_path("", "screenshots/2026-05-22/shot.jpg"),
            "screenshots/2026-05-22/shot.jpg"
        );
    }

    #[test]
    fn 公开访问地址应使用远程对象路径并忽略空前缀() {
        assert_eq!(
            public_url_or_fallback(
                Some(" https://cdn.example.com/workreview/ "),
                "archive/screenshots/shot.jpg",
                "https://webdav.example.com/archive/screenshots/shot.jpg",
            ),
            "https://cdn.example.com/workreview/archive/screenshots/shot.jpg"
        );
        assert_eq!(
            public_url_or_fallback(
                Some("   "),
                "archive/screenshots/shot.jpg",
                "https://webdav.example.com/archive/screenshots/shot.jpg",
            ),
            "https://webdav.example.com/archive/screenshots/shot.jpg"
        );
    }
}
