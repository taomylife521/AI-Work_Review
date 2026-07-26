//! OS 密钥保险柜：敏感字段不再明文落盘。
//!
//! 磁盘 JSON（含 .bak 备份）中敏感字段写占位符 `__keychain__`,真实值存系统钥匙串
//! （macOS Keychain / Windows 凭据管理器 / Linux secret-service）。
//! 载入时 `hydrate_config` 注水回内存 AppConfig;保存统一走 `SecureSaveExt::save_secure`
//! （克隆 → 剥离进钥匙串 → 落盘）,内存中的 AppState.config 始终保持注水后的真实值,
//! 业务代码零改动。
//!
//! 钥匙串不可用（如 Linux 桌面缺 secret-service）时回退明文并记警告,不阻断使用。
//! 迁移:旧配置里的明文密钥在载入时即写入钥匙串,下一次保存后磁盘转为占位符。
//! 注意:保险柜保护的是磁盘/备份/云同步面;`get_config` IPC 返回的仍是内存真实值
//! （设置页需要展示与编辑）,与既有暴露面一致。

use crate::config::AppConfig;
use keyring::Entry;
use std::path::Path;

const SERVICE: &str = "work-review";
pub const KEYCHAIN_PLACEHOLDER: &str = "__keychain__";

fn store(field: &str, value: &str) -> bool {
    match Entry::new(SERVICE, field).and_then(|e| e.set_password(value)) {
        Ok(()) => true,
        Err(e) => {
            log::warn!("写入系统钥匙串失败({field})，该字段将回退明文存储: {e}");
            false
        }
    }
}

fn load(field: &str) -> Option<String> {
    match Entry::new(SERVICE, field).and_then(|e| e.get_password()) {
        Ok(value) => Some(value),
        Err(keyring::Error::NoEntry) => None,
        Err(e) => {
            log::warn!("读取系统钥匙串失败({field}): {e}");
            None
        }
    }
}

fn remove(field: &str) {
    if let Ok(entry) = Entry::new(SERVICE, field) {
        let _ = entry.delete_credential();
    }
}

/// 载入方向：占位符 → 从钥匙串取回真实值;
/// 明文（旧配置迁移）→ 立即写入钥匙串,内存保留真实值。
fn hydrate_opt(field: &mut Option<String>, name: &str) {
    match field.as_deref() {
        Some(KEYCHAIN_PLACEHOLDER) => {
            *field = load(name);
        }
        Some(value) if !value.is_empty() => {
            // 旧明文迁移：先落钥匙串,磁盘在下一次保存时转占位符
            store(name, value);
        }
        _ => {}
    }
}

/// 保存方向：真实值 → 写钥匙串,磁盘副本替换为占位符;
/// 清空/缺失 → 删除钥匙串残留,磁盘按原样(空)写。
fn strip_opt(field: &mut Option<String>, name: &str) {
    match field.as_deref() {
        Some(KEYCHAIN_PLACEHOLDER) => {}
        Some(value) if !value.is_empty() => {
            if store(name, value) {
                *field = Some(KEYCHAIN_PLACEHOLDER.to_string());
            }
        }
        _ => {
            remove(name);
        }
    }
}

fn hydrate_str(field: &mut String, name: &str) {
    if field == KEYCHAIN_PLACEHOLDER {
        *field = load(name).unwrap_or_default();
    } else if !field.is_empty() {
        store(name, field);
    }
}

fn strip_str(field: &mut String, name: &str) {
    if field == KEYCHAIN_PLACEHOLDER {
        return;
    }
    if field.is_empty() {
        remove(name);
    } else if store(name, field) {
        *field = KEYCHAIN_PLACEHOLDER.to_string();
    }
}

/// 敏感字段全名录：新增密钥字段时在此登记（hydrate/strip 必须成对）。
macro_rules! for_each_secret {
    ($config:expr, $opt:ident, $str:ident) => {
        $opt(&mut $config.text_model.api_key, "text_model.api_key");
        $opt(&mut $config.vision_model.api_key, "vision_model.api_key");
        $opt(&mut $config.ai_provider.api_key, "ai_provider.api_key");
        $opt(&mut $config.openai_api_key, "openai_api_key");
        $opt(&mut $config.assistant_search_api_key, "assistant_search_api_key");
        $opt(&mut $config.embedding_api_key, "embedding_api_key");
        $opt(&mut $config.telegram_bot_token, "telegram_bot_token");
        $opt(&mut $config.feishu_app_secret, "feishu_app_secret");
        $opt(&mut $config.feishu_verification_token, "feishu_verification_token");
        $opt(&mut $config.feishu_encrypt_key, "feishu_encrypt_key");
        $opt(&mut $config.wecom_token, "wecom_token");
        $opt(&mut $config.wecom_encoding_aes_key, "wecom_encoding_aes_key");
        $opt(&mut $config.dingtalk_app_secret, "dingtalk_app_secret");
        $str(&mut $config.remote_storage.s3.access_key, "remote_storage.s3.access_key");
        $str(&mut $config.remote_storage.s3.secret_key, "remote_storage.s3.secret_key");
        $str(&mut $config.remote_storage.webdav.password, "remote_storage.webdav.password");
        for profile in $config.text_model_profiles.iter_mut() {
            let name = format!("profile.{}.api_key", profile.id);
            $opt(&mut profile.model_config.api_key, &name);
        }
    };
}

/// 载入后调用：把占位符注水为真实值（并顺带迁移旧明文进钥匙串）。
pub fn hydrate_config(config: &mut AppConfig) {
    for_each_secret!(config, hydrate_opt, hydrate_str);
}

/// 落盘前调用（save_secure 内部）：真实值剥离进钥匙串。
pub fn strip_config(config: &mut AppConfig) {
    for_each_secret!(config, strip_opt, strip_str);
}

/// 安全保存扩展：所有 `config.save(path)` 调用点统一替换为 `config.save_secure(path)`。
pub trait SecureSaveExt {
    fn save_secure(&self, path: &Path) -> Result<(), crate::error::AppError>;
}

impl SecureSaveExt for AppConfig {
    fn save_secure(&self, path: &Path) -> Result<(), crate::error::AppError> {
        let mut disk_copy = self.clone();
        strip_config(&mut disk_copy);
        disk_copy.save(path)
    }
}
