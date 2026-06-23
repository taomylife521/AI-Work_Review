//! Auto-extracted from the historical `commands.rs`. Behavior unchanged.

use crate::database::Activity;
use crate::error::AppError;
#[cfg(target_os = "linux")]
use crate::linux_session::{current_linux_desktop_environment, current_linux_desktop_session, LinuxDesktopSession};
use crate::AppState;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, State};

/// 开始录制
#[tauri::command]
pub async fn start_recording(
    app: AppHandle,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), AppError> {
    let mut state = state.lock().map_err(|e| AppError::Unknown(e.to_string()))?;
    state.is_recording = true;
    state.is_paused = false;
    log::info!("开始录制");
    drop(state);
    crate::emit_recording_state_changed(&app);
    Ok(())
}

/// 停止录制
#[tauri::command]
pub async fn stop_recording(
    app: AppHandle,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), AppError> {
    let mut state = state.lock().map_err(|e| AppError::Unknown(e.to_string()))?;
    state.is_recording = false;
    state.is_paused = false;
    log::info!("停止录制");
    drop(state);
    crate::emit_recording_state_changed(&app);
    Ok(())
}

/// 暂停录制
#[tauri::command]
pub async fn pause_recording(
    app: AppHandle,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), AppError> {
    let mut state = state.lock().map_err(|e| AppError::Unknown(e.to_string()))?;
    state.is_paused = true;
    log::info!("暂停录制");
    drop(state);
    crate::emit_recording_state_changed(&app);
    Ok(())
}

/// 恢复录制
#[tauri::command]
pub async fn resume_recording(
    app: AppHandle,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), AppError> {
    let mut state = state.lock().map_err(|e| AppError::Unknown(e.to_string()))?;
    state.is_recording = true;
    state.is_paused = false;
    log::info!("恢复录制");
    drop(state);
    crate::emit_recording_state_changed(&app);
    Ok(())
}

/// 获取录制状态
#[tauri::command]
pub async fn get_recording_state(
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<(bool, bool), AppError> {
    let state = state.lock().map_err(|e| AppError::Unknown(e.to_string()))?;
    Ok((state.is_recording, state.is_paused))
}

/// 手动执行一次截屏
#[tauri::command]
pub async fn take_screenshot(state: State<'_, Arc<Mutex<AppState>>>) -> Result<Activity, AppError> {
    let (
        screenshot_result,
        app_name,
        window_title,
        browser_url,
        category,
        semantic_category,
        semantic_confidence,
        relative_path,
        executable_path,
    ) = {
        let state = state.lock().map_err(|e| AppError::Unknown(e.to_string()))?;

        // 获取当前活动窗口
        let active_window = crate::monitor::get_active_window().ok();

        #[cfg(target_os = "linux")]
        let active_window = if active_window.is_none()
            && !matches!(
                current_linux_desktop_session(),
                LinuxDesktopSession::Wayland
            ) {
            return Err(AppError::Unknown("获取当前活动窗口失败".to_string()));
        } else {
            active_window
        };

        #[cfg(not(target_os = "linux"))]
        let active_window = match active_window {
            Some(active_window) => Some(active_window),
            None => return Err(AppError::Unknown("获取当前活动窗口失败".to_string())),
        };

        // 检查隐私过滤
        if let Some(active_window) = active_window.as_ref() {
            if state.privacy_filter.check_privacy_full(
                &active_window.app_name,
                &active_window.window_title,
                active_window.browser_url.as_deref(),
            ) == crate::privacy::PrivacyAction::Skip
            {
                return Err(AppError::Privacy("当前窗口被隐私规则过滤".to_string()));
            }
        }

        // 执行截屏
        let result = state
            .screenshot_service
            .capture_for_window(active_window.as_ref())?;
        let relative_path = state.screenshot_service.get_relative_path(&result.path);
        let app_name = active_window
            .as_ref()
            .map(|window| window.app_name.clone())
            .unwrap_or_else(|| "Wayland Session".to_string());
        let window_title = active_window
            .as_ref()
            .map(|window| window.window_title.clone())
            .unwrap_or_else(|| "Wayland screenshot".to_string());
        let browser_url = active_window
            .as_ref()
            .and_then(|window| window.browser_url.clone());
        let executable_path = active_window
            .as_ref()
            .and_then(|window| window.executable_path.clone());
        let classification = crate::resolve_activity_classification(
            &state.config,
            &app_name,
            &window_title,
            browser_url.as_deref(),
        );

        (
            result,
            app_name,
            window_title,
            browser_url,
            classification.base_category,
            classification.semantic_category,
            classification.confidence,
            relative_path,
            executable_path,
        )
    };

    // 创建活动记录
    let activity = Activity {
        id: None,
        timestamp: screenshot_result.timestamp,
        app_name,
        window_title,
        screenshot_path: relative_path,
        ocr_text: None,
        category,
        duration: 30,
        browser_url,
        executable_path,
        semantic_category: Some(semantic_category),
        semantic_confidence: Some(i32::from(semantic_confidence)),
        screenshot_url: None,
    };

    // 保存到数据库
    let insert_result = {
        let state = state.lock().map_err(|e| AppError::Unknown(e.to_string()))?;
        state.database.insert_activity(&activity)
    };

    if let Err(error) = insert_result {
        let _ = std::fs::remove_file(&screenshot_result.path);
        if let Some(temp_path) = screenshot_result
            .ocr_source_path
            .as_ref()
            .filter(|path| *path != &screenshot_result.path)
        {
            let _ = std::fs::remove_file(temp_path);
        }
        return Err(error);
    }

    if let Some(temp_path) = screenshot_result
        .ocr_source_path
        .as_ref()
        .filter(|path| *path != &screenshot_result.path)
    {
        let _ = std::fs::remove_file(temp_path);
    }

    Ok(activity)
}

/// 检查屏幕锁定状态
#[tauri::command]
pub async fn is_screen_locked() -> Result<bool, AppError> {
    let monitor = crate::screen_lock::ScreenLockMonitor::new();
    Ok(monitor.is_locked())
}

/// 检查 PaddleOCR 是否可用
#[tauri::command]
pub async fn check_ocr_available() -> Result<serde_json::Value, AppError> {
    let paddle_available = crate::ocr::OcrService::check_paddle_available();

    Ok(serde_json::json!({
        "paddle_ocr_available": paddle_available,
        "install_command": crate::ocr::OcrService::get_paddle_install_command(),
        "platform": std::env::consts::OS,
    }))
}

/// 执行 OCR 识别
#[tauri::command]
pub async fn run_ocr(
    screenshot_path: String,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<serde_json::Value, AppError> {
    let (data_dir, full_path) = {
        let state = state.lock().map_err(|e| AppError::Unknown(e.to_string()))?;
        let full_path = state.data_dir.join(&screenshot_path);
        (state.data_dir.clone(), full_path)
    };

    if !full_path.exists() {
        return Err(AppError::Unknown(format!("截图文件不存在: {full_path:?}")));
    }

    let ocr_service = crate::ocr::OcrService::new(&data_dir);

    match ocr_service.extract_text(&full_path) {
        Ok(Some(result)) => {
            // 过滤敏感信息
            let filtered_text = crate::ocr::filter_sensitive_text(&result.text);

            Ok(serde_json::json!({
                "success": true,
                "text": filtered_text,
                "raw_text": result.text,
                "confidence": result.confidence,
                "box_count": result.boxes.len(),
            }))
        }
        Ok(None) => Ok(serde_json::json!({
            "success": true,
            "text": "",
            "message": "未检测到文字",
        })),
        Err(e) => Ok(serde_json::json!({
            "success": false,
            "error": e.to_string(),
        })),
    }
}

/// 获取 OCR 安装指南
#[tauri::command]
pub async fn get_ocr_install_guide() -> Result<serde_json::Value, AppError> {
    let platform = std::env::consts::OS;

    let guide = match platform {
        "windows" => serde_json::json!({
            "platform": "Windows",
            "steps": [
                "1. 确保已安装 Python 3.8+",
                "2. 打开命令提示符或 PowerShell",
                "3. 运行以下命令安装 PaddleOCR：",
                "   pip install paddlepaddle paddleocr -i https://mirror.baidu.com/pypi/simple",
                "4. 等待安装完成（首次运行会自动下载模型）",
                "",
                "备选方案：使用 Windows 内置 OCR（无需安装，但识别效果较弱）"
            ],
            "install_command": "pip install paddlepaddle paddleocr -i https://mirror.baidu.com/pypi/simple",
            "has_builtin_fallback": true,
        }),
        "macos" => serde_json::json!({
            "platform": "macOS",
            "steps": [
                "macOS 使用系统内置的 Vision 框架进行 OCR，无需额外安装。",
                "",
                "如需使用 PaddleOCR（效果更好）：",
                "1. 确保已安装 Python 3.8+",
                "2. 运行以下命令：",
                "   pip install paddlepaddle paddleocr",
            ],
            "install_command": "pip install paddlepaddle paddleocr",
            "has_builtin_fallback": true,
        }),
        _ => serde_json::json!({
            "platform": platform,
            "steps": [
                "1. 确保已安装 Python 3.8+",
                "2. 运行以下命令安装 PaddleOCR：",
                "   pip install paddlepaddle paddleocr",
            ],
            "install_command": "pip install paddlepaddle paddleocr",
            "has_builtin_fallback": false,
        }),
    };

    Ok(guide)
}

