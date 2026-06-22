use crate::bot_common::{
    build_device_list, handle_cmd, normalize_command, progress_text_for_command, status_payload,
    NON_TEXT_REPLY, UNKNOWN_CMD_REPLY,
};
use crate::config::AppConfig;
use reqwest::Client;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use subtle::ConstantTimeEq;

/// 常量时间比较 verification_token，防止时序侧信道。
fn token_matches(provided: &str, expected: &str) -> bool {
    provided.as_bytes().ct_eq(expected.as_bytes()).into()
}

pub struct FeishuResponse {
    pub status: u16,
    pub body: String,
}

impl FeishuResponse {
    pub fn json(status: u16, value: &serde_json::Value) -> Self {
        Self {
            status,
            body: value.to_string(),
        }
    }

    pub fn error(status: u16, message: impl Into<String>) -> Self {
        Self::json(status, &serde_json::json!({"error": message.into()}))
    }
}

// Token cache: (token, expires_at)
static TOKEN_CACHE: Mutex<Option<(String, Instant)>> = Mutex::new(None);

async fn get_tenant_token(client: &Client, app_id: &str, app_secret: &str) -> Option<String> {
    {
        let cache = TOKEN_CACHE.lock().ok()?;
        if let Some((token, expires)) = cache.as_ref() {
            if expires > &Instant::now() {
                return Some(token.clone());
            }
        }
    }
    let resp = client
        .post("https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal")
        .json(&serde_json::json!({"app_id": app_id, "app_secret": app_secret}))
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .ok()?;
    let data: serde_json::Value = resp.json().await.ok()?;
    let token = data.get("tenant_access_token")?.as_str()?.to_string();
    let expire = data.get("expire").and_then(|v| v.as_u64()).unwrap_or(7200);
    let cache_ttl = expire.saturating_sub(60);
    if let Ok(mut cache) = TOKEN_CACHE.lock() {
        *cache = Some((
            token.clone(),
            Instant::now() + Duration::from_secs(cache_ttl.max(60)),
        ));
    }
    Some(token)
}

async fn reply_message(client: &Client, token: &str, message_id: &str, text: &str) -> Option<()> {
    let url = format!("https://open.feishu.cn/open-apis/im/v1/messages/{message_id}/reply");
    let content = serde_json::json!({"text": text}).to_string();
    client
        .post(&url)
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({"content_type": "text", "content": content}))
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .ok()?;
    Some(())
}

pub async fn handle_feishu_webhook(
    body: &str,
    config: &AppConfig,
    data_dir: &Path,
) -> FeishuResponse {
    let event: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(e) => return FeishuResponse::error(400, format!("JSON parse error: {e}")),
    };

    // URL verification challenge
    if event.get("type").and_then(|v| v.as_str()) == Some("url_verification") {
        let challenge = event
            .get("challenge")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let expected = config.feishu_verification_token.as_deref().unwrap_or("");
        if expected.is_empty() {
            return FeishuResponse::error(403, "verification token not configured");
        }
        let token = event.get("token").and_then(|v| v.as_str()).unwrap_or("");
        if !token_matches(token, expected) {
            return FeishuResponse::error(403, "verification token mismatch");
        }
        return FeishuResponse::json(200, &serde_json::json!({"challenge": challenge}));
    }

    // Message event
    let header = match event.get("header") {
        Some(h) => h,
        None => return FeishuResponse::error(400, "missing header"),
    };

    if header.get("event_type").and_then(|v| v.as_str()) != Some("im.message.receive_v1") {
        return FeishuResponse::json(
            200,
            &status_payload("ignored", "event_type_not_supported", None),
        );
    }

    let expected = config.feishu_verification_token.as_deref().unwrap_or("");
    if expected.is_empty() {
        return FeishuResponse::error(403, "verification token not configured");
    }
    let token = header.get("token").and_then(|v| v.as_str()).unwrap_or("");
    if !token_matches(token, expected) {
        return FeishuResponse::error(403, "token mismatch");
    }

    let event_body = match event.get("event") {
        Some(b) => b,
        None => return FeishuResponse::error(400, "missing event body"),
    };

    let message = match event_body.get("message") {
        Some(m) => m,
        None => return FeishuResponse::error(400, "missing message"),
    };

    let message_id = match message.get("message_id").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => return FeishuResponse::error(400, "missing message_id"),
    };

    let msg_type = message
        .get("message_type")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if msg_type != "text" {
        let reply = NON_TEXT_REPLY.to_string();
        let app_id = match config.feishu_app_id.as_deref() {
            Some(id) if !id.is_empty() => id,
            _ => {
                return FeishuResponse::json(
                    200,
                    &status_payload("ignored", "non_text_message", Some("feishu_app_id 未配置")),
                )
            }
        };
        let app_secret = match config.feishu_app_secret.as_deref() {
            Some(s) if !s.is_empty() => s,
            _ => {
                return FeishuResponse::json(
                    200,
                    &status_payload(
                        "ignored",
                        "non_text_message",
                        Some("feishu_app_secret 未配置"),
                    ),
                )
            }
        };
        let client = match Client::builder().timeout(Duration::from_secs(35)).build() {
            Ok(c) => c,
            Err(e) => return FeishuResponse::error(500, format!("HTTP client error: {e}")),
        };
        if let Some(tenant_token) = get_tenant_token(&client, app_id, app_secret).await {
            let _ = reply_message(&client, &tenant_token, message_id, &reply).await;
        }
        return FeishuResponse::json(
            200,
            &status_payload("ok", "non_text_replied", Some("已提示使用 /help")),
        );
    }

    let content_str = message
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("{}");
    let content: serde_json::Value = serde_json::from_str(content_str).unwrap_or_default();
    let text = content
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();

    if text.is_empty() {
        return FeishuResponse::json(200, &status_payload("ignored", "empty_text", None));
    }

    let app_id = match config.feishu_app_id.as_deref() {
        Some(id) if !id.is_empty() => id,
        _ => return FeishuResponse::error(500, "feishu_app_id not configured"),
    };
    let app_secret = match config.feishu_app_secret.as_deref() {
        Some(s) if !s.is_empty() => s,
        _ => return FeishuResponse::error(500, "feishu_app_secret not configured"),
    };

    let devices = build_device_list(config, data_dir);
    let client = match Client::builder().timeout(Duration::from_secs(35)).build() {
        Ok(c) => c,
        Err(e) => return FeishuResponse::error(500, format!("HTTP client error: {e}")),
    };

    let command = normalize_command(text.split_whitespace().next().unwrap_or(""));
    if let Some(progress) = progress_text_for_command(&command) {
        if let Some(token) = get_tenant_token(&client, app_id, app_secret).await {
            let _ = reply_message(&client, &token, message_id, progress).await;
        }
    }

    let reply = handle_cmd(&client, &devices, text)
        .await
        .unwrap_or_else(|| UNKNOWN_CMD_REPLY.to_string());

    let tenant_token = match get_tenant_token(&client, app_id, app_secret).await {
        Some(t) => t,
        None => return FeishuResponse::error(500, "failed to get tenant_access_token"),
    };

    match reply_message(&client, &tenant_token, message_id, &reply).await {
        Some(_) => FeishuResponse::json(
            200,
            &status_payload("ok", "replied", Some("已发送回复消息")),
        ),
        None => FeishuResponse::error(500, "failed to send reply"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 飞书命令应支持斜杠和机器人后缀() {
        assert_eq!(normalize_command("/help"), "help");
        assert_eq!(normalize_command("/reports@work_review_bot"), "reports");
        assert_eq!(normalize_command("帮助"), "帮助");
    }
}
