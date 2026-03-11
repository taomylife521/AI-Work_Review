// 锁屏检测模块 (Windows / macOS)
// 监听系统锁屏/解锁事件，用于控制录制状态

#![allow(dead_code)]

use chrono::Timelike;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// 屏幕锁定状态
pub struct ScreenLockMonitor {
    /// 是否锁定
    is_locked: Arc<AtomicBool>,
}

impl ScreenLockMonitor {
    /// 创建锁屏监控器
    pub fn new() -> Self {
        Self {
            is_locked: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 检查屏幕是否锁定 (Windows)
    /// 使用 OpenInputDesktop 方式判断：锁屏时系统桌面切换到 Winlogon 桌面，
    /// 此时当前线程无法打开输入桌面，可靠性远高于 GetForegroundWindow/quser
    #[cfg(target_os = "windows")]
    pub fn is_locked(&self) -> bool {
        use winapi::um::winnt::GENERIC_ALL;
        use winapi::um::winuser::{CloseDesktop, OpenInputDesktop, SwitchDesktop};

        unsafe {
            // 尝试打开当前输入桌面
            // 锁屏时系统会切换到 Winlogon 桌面，当前进程无权限打开，返回 null
            let desktop = OpenInputDesktop(0, 0, GENERIC_ALL);
            if desktop.is_null() {
                // 无法打开输入桌面，说明已经锁屏
                log::debug!("锁屏检测: OpenInputDesktop 返回 null，判断为锁屏");
                return true;
            }

            // 尝试切换到该桌面（如果切换失败，说明是受限的 Winlogon 桌面）
            let switched = SwitchDesktop(desktop);
            CloseDesktop(desktop);

            if switched == 0 {
                // SwitchDesktop 失败，说明是锁屏桌面
                log::debug!("锁屏检测: SwitchDesktop 失败，判断为锁屏");
                return true;
            }
        }

        false
    }

    /// 检查屏幕是否锁定 (macOS)
    /// 使用多种方法检测，避免依赖 Python/pyobjc
    #[cfg(target_os = "macos")]
    pub fn is_locked(&self) -> bool {
        use std::process::Command;

        // 方法1: 检查是否有屏幕保护程序运行
        let output = Command::new("pgrep")
            .args(["-x", "ScreenSaverEngine"])
            .output();

        if let Ok(out) = output {
            if out.status.success() {
                log::debug!("锁屏检测: 屏幕保护程序运行中");
                return true;
            }
        }

        // 方法2: 使用 ioreg 检测显示器电源状态
        // 当屏幕关闭（锁屏后自动关闭）时，IODisplayWrangler 的 DevicePowerState 为 0
        let output = Command::new("ioreg")
            .args(["-r", "-c", "IODisplayWrangler", "-d", "1"])
            .output();

        if let Ok(out) = output {
            let stdout = String::from_utf8_lossy(&out.stdout);
            // 检查 DevicePowerState = 0 表示显示器已关闭
            if stdout.contains("\"DevicePowerState\" = 0") {
                log::debug!("锁屏检测: 显示器已关闭");
                return true;
            }
        }

        // 方法3: 使用 osascript 检查屏幕保护状态
        let output = Command::new("osascript")
            .args([
                "-e",
                "tell application \"System Events\" to return running of screen saver preferences",
            ])
            .output();

        if let Ok(out) = output {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if stdout.trim() == "true" {
                log::debug!("锁屏检测: 屏幕保护已激活");
                return true;
            }
        }

        // 方法4: 检查 loginwindow 进程是否在前台（用户在锁屏界面）
        let output = Command::new("osascript")
            .args(["-e", "tell application \"System Events\" to get name of first application process whose frontmost is true"])
            .output();

        if let Ok(out) = output {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let frontmost = stdout.trim().to_lowercase();
            if frontmost == "loginwindow" || frontmost == "screensaverengine" {
                log::debug!("锁屏检测: 前台应用为锁屏界面");
                return true;
            }
        }

        false
    }

    /// 检查屏幕是否锁定 (其他平台)
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    pub fn is_locked(&self) -> bool {
        false
    }

    /// 设置锁定状态（用于手动更新）
    pub fn set_locked(&self, locked: bool) {
        self.is_locked.store(locked, Ordering::SeqCst);
    }

    /// 检查是否在工作时间内
    pub fn is_work_time(start_hour: u8, end_hour: u8) -> bool {
        let now = chrono::Local::now();
        let hour = now.hour() as u8;

        if start_hour <= end_hour {
            // 正常时间范围，如 9-18
            hour >= start_hour && hour < end_hour
        } else {
            // 跨午夜，如 22-6
            hour >= start_hour || hour < end_hour
        }
    }
}

impl Default for ScreenLockMonitor {
    fn default() -> Self {
        Self::new()
    }
}
