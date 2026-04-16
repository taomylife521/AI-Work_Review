use crate::avatar_engine::AvatarInputPayload;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::AppHandle;

const KEYBOARD_ACTIVE_WINDOW_MS: u64 = 180;
const MOUSE_ACTIVE_WINDOW_MS: u64 = 220;

static LAST_KEYBOARD_INPUT_AT_MS: AtomicU64 = AtomicU64::new(0);
static LAST_MOUSE_INPUT_AT_MS: AtomicU64 = AtomicU64::new(0);
static LAST_KEYBOARD_GROUP_CODE: AtomicU8 = AtomicU8::new(0);
static LAST_KEYBOARD_KEY_CODE: AtomicU16 = AtomicU16::new(0);
static LAST_MOUSE_GROUP_CODE: AtomicU8 = AtomicU8::new(0);
static CURSOR_RATIO_X_PERMILLE: AtomicU32 = AtomicU32::new(500);
static CURSOR_RATIO_Y_PERMILLE: AtomicU32 = AtomicU32::new(500);
static INPUT_BRIDGE_STARTED: AtomicBool = AtomicBool::new(false);
static INPUT_MONITOR_STARTED: AtomicBool = AtomicBool::new(false);

const KEYBOARD_GROUP_DIGIT_1: u8 = 1;
const KEYBOARD_GROUP_DIGIT_2: u8 = 2;
const KEYBOARD_GROUP_DIGIT_3: u8 = 3;
const KEYBOARD_GROUP_DIGIT_4: u8 = 4;
const KEYBOARD_GROUP_DIGIT_5: u8 = 5;
const KEYBOARD_GROUP_DIGIT_6: u8 = 6;
const KEYBOARD_GROUP_DIGIT_7: u8 = 7;
const KEYBOARD_GROUP_KEY_Q: u8 = 8;
const KEYBOARD_GROUP_KEY_E: u8 = 9;
const KEYBOARD_GROUP_KEY_R: u8 = 10;
const KEYBOARD_GROUP_SPACE: u8 = 11;
const KEYBOARD_GROUP_KEY_A: u8 = 12;
const KEYBOARD_GROUP_KEY_D: u8 = 13;
const KEYBOARD_GROUP_KEY_S: u8 = 14;
const KEYBOARD_GROUP_KEY_W: u8 = 15;

const MOUSE_GROUP_MOVE: u8 = 1;
const MOUSE_GROUP_LEFT: u8 = 2;
const MOUSE_GROUP_RIGHT: u8 = 3;
const MOUSE_GROUP_SIDE: u8 = 4;

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn is_input_still_active(last_input_at_ms: u64, now_ms: u64, active_window_ms: u64) -> bool {
    last_input_at_ms > 0 && now_ms.saturating_sub(last_input_at_ms) <= active_window_ms
}

fn keyboard_group_label(code: u8) -> &'static str {
    match code {
        KEYBOARD_GROUP_DIGIT_1 => "digit-1",
        KEYBOARD_GROUP_DIGIT_2 => "digit-2",
        KEYBOARD_GROUP_DIGIT_3 => "digit-3",
        KEYBOARD_GROUP_DIGIT_4 => "digit-4",
        KEYBOARD_GROUP_DIGIT_5 => "digit-5",
        KEYBOARD_GROUP_DIGIT_6 => "digit-6",
        KEYBOARD_GROUP_DIGIT_7 => "digit-7",
        KEYBOARD_GROUP_KEY_Q => "key-q",
        KEYBOARD_GROUP_KEY_E => "key-e",
        KEYBOARD_GROUP_KEY_R => "key-r",
        KEYBOARD_GROUP_SPACE => "space",
        KEYBOARD_GROUP_KEY_A => "key-a",
        KEYBOARD_GROUP_KEY_D => "key-d",
        KEYBOARD_GROUP_KEY_S => "key-s",
        KEYBOARD_GROUP_KEY_W => "key-w",
        _ => "idle",
    }
}

fn standard_keyboard_group_from_key_code(key_code: u16) -> u8 {
    match key_code {
        18 => KEYBOARD_GROUP_DIGIT_1,
        19 => KEYBOARD_GROUP_DIGIT_2,
        20 => KEYBOARD_GROUP_DIGIT_3,
        21 => KEYBOARD_GROUP_DIGIT_4,
        23 => KEYBOARD_GROUP_DIGIT_5,
        22 => KEYBOARD_GROUP_DIGIT_6,
        26 => KEYBOARD_GROUP_DIGIT_7,
        12 => KEYBOARD_GROUP_KEY_Q,
        14 => KEYBOARD_GROUP_KEY_E,
        15 => KEYBOARD_GROUP_KEY_R,
        49 => KEYBOARD_GROUP_SPACE,
        0 => KEYBOARD_GROUP_KEY_A,
        2 => KEYBOARD_GROUP_KEY_D,
        1 => KEYBOARD_GROUP_KEY_S,
        13 => KEYBOARD_GROUP_KEY_W,
        _ => 0,
    }
}

fn keyboard_visual_key_from_key_code(key_code: u16) -> &'static str {
    match key_code {
        0 => "KeyA",
        1 => "KeyS",
        2 => "KeyD",
        3 => "KeyF",
        4 => "KeyH",
        5 => "KeyG",
        6 => "KeyZ",
        7 => "KeyX",
        8 => "KeyC",
        9 => "KeyV",
        11 => "KeyB",
        12 => "KeyQ",
        13 => "KeyW",
        14 => "KeyE",
        15 => "KeyR",
        16 => "KeyY",
        17 => "KeyT",
        18 => "Num1",
        19 => "Num2",
        20 => "Num3",
        21 => "Num4",
        22 => "Num6",
        23 => "Num5",
        25 => "Num9",
        26 => "Num7",
        28 => "Num8",
        29 => "Num0",
        31 => "KeyO",
        32 => "KeyU",
        34 => "KeyI",
        35 => "KeyP",
        36 => "Return",
        37 => "KeyL",
        38 => "KeyJ",
        40 => "KeyK",
        43 => "Comma",
        44 => "Slash",
        45 => "KeyN",
        46 => "KeyM",
        47 => "Period",
        48 => "Tab",
        49 => "Space",
        50 => "BackQuote",
        51 => "Backspace",
        53 => "Escape",
        55 => "Meta",
        56 => "ShiftLeft",
        57 => "CapsLock",
        58 => "Alt",
        59 => "ControlLeft",
        60 => "ShiftRight",
        61 => "AltGr",
        62 => "ControlRight",
        63 => "Fn",
        117 => "Delete",
        123 => "LeftArrow",
        124 => "RightArrow",
        125 => "DownArrow",
        126 => "UpArrow",
        _ => "",
    }
}

fn mouse_group_label(code: u8) -> &'static str {
    match code {
        MOUSE_GROUP_LEFT => "mouse-left",
        MOUSE_GROUP_RIGHT => "mouse-right",
        MOUSE_GROUP_SIDE => "mouse-side",
        MOUSE_GROUP_MOVE => "mouse-move",
        _ => "idle",
    }
}

#[cfg(target_os = "macos")]
fn mouse_group_from_event_type(event_type: cocoa::appkit::NSEventType) -> u8 {
    use cocoa::appkit::NSEventType;

    match event_type {
        NSEventType::NSLeftMouseDown => MOUSE_GROUP_LEFT,
        NSEventType::NSRightMouseDown => MOUSE_GROUP_RIGHT,
        NSEventType::NSOtherMouseDown => MOUSE_GROUP_SIDE,
        _ => MOUSE_GROUP_MOVE,
    }
}

pub(crate) fn build_avatar_input_payload(now_ms: u64) -> AvatarInputPayload {
    let last_keyboard_input_at_ms = LAST_KEYBOARD_INPUT_AT_MS.load(Ordering::Relaxed);
    let last_mouse_input_at_ms = LAST_MOUSE_INPUT_AT_MS.load(Ordering::Relaxed);
    let keyboard_group_code = LAST_KEYBOARD_GROUP_CODE.load(Ordering::Relaxed);
    let keyboard_key_code = LAST_KEYBOARD_KEY_CODE.load(Ordering::Relaxed);
    let mouse_group_code = LAST_MOUSE_GROUP_CODE.load(Ordering::Relaxed);

    AvatarInputPayload {
        keyboard_active: is_input_still_active(
            last_keyboard_input_at_ms,
            now_ms,
            KEYBOARD_ACTIVE_WINDOW_MS,
        ),
        mouse_active: is_input_still_active(last_mouse_input_at_ms, now_ms, MOUSE_ACTIVE_WINDOW_MS),
        keyboard_group: keyboard_group_label(keyboard_group_code).to_string(),
        keyboard_visual_key: keyboard_visual_key_from_key_code(keyboard_key_code).to_string(),
        mouse_group: mouse_group_label(mouse_group_code).to_string(),
        cursor_ratio_x: CURSOR_RATIO_X_PERMILLE.load(Ordering::Relaxed) as f64 / 1000.0,
        cursor_ratio_y: CURSOR_RATIO_Y_PERMILLE.load(Ordering::Relaxed) as f64 / 1000.0,
        last_keyboard_input_at_ms,
        last_mouse_input_at_ms,
    }
}

pub(crate) fn record_keyboard_input(group_code: u8, key_code: u16) {
    LAST_KEYBOARD_INPUT_AT_MS.store(now_ms(), Ordering::Relaxed);
    LAST_KEYBOARD_GROUP_CODE.store(group_code, Ordering::Relaxed);
    LAST_KEYBOARD_KEY_CODE.store(key_code, Ordering::Relaxed);
}

pub(crate) fn record_mouse_input(group_code: u8) {
    LAST_MOUSE_INPUT_AT_MS.store(now_ms(), Ordering::Relaxed);
    LAST_MOUSE_GROUP_CODE.store(group_code, Ordering::Relaxed);
}

pub(crate) fn record_cursor_ratio(x_ratio: f64, y_ratio: f64) {
    let to_permille = |value: f64| -> u32 { (value.clamp(0.0, 1.0) * 1000.0).round() as u32 };

    CURSOR_RATIO_X_PERMILLE.store(to_permille(x_ratio), Ordering::Relaxed);
    CURSOR_RATIO_Y_PERMILLE.store(to_permille(y_ratio), Ordering::Relaxed);
}

pub fn spawn_avatar_input_bridge(app: AppHandle) {
    if INPUT_BRIDGE_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }

    tauri::async_runtime::spawn(async move {
        let mut last_payload: Option<AvatarInputPayload> = None;

        loop {
            let next_payload = build_avatar_input_payload(now_ms());
            if last_payload.as_ref() != Some(&next_payload) {
                crate::avatar_engine::emit_avatar_input(&app, &next_payload);
                last_payload = Some(next_payload);
            }

            tokio::time::sleep(Duration::from_millis(33)).await;
        }
    });
}

#[cfg(target_os = "macos")]
thread_local! {
    static MACOS_INPUT_MONITOR: std::cell::RefCell<Option<MacosInputMonitor>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(target_os = "macos")]
#[allow(dead_code)]
struct MacosInputMonitor {
    keyboard_monitor: cocoa::base::id,
    mouse_monitor: cocoa::base::id,
    keyboard_handler: block::RcBlock<(cocoa::base::id,), ()>,
    mouse_handler: block::RcBlock<(cocoa::base::id,), ()>,
}

#[cfg(target_os = "macos")]
pub fn start_avatar_input_monitor(app: &AppHandle) {
    use block::ConcreteBlock;
    use cocoa::appkit::{NSEventMask, NSEventType};
    use cocoa::base::{id, nil};
    use cocoa::foundation::{NSPoint, NSRect};

    unsafe fn current_cursor_ratio() -> (f64, f64) {
        let screens: id = msg_send![class!(NSScreen), screens];
        let count: usize = msg_send![screens, count];
        if count == 0 {
            return (0.5, 0.5);
        }

        let mut min_x = 0.0;
        let mut min_y = 0.0;
        let mut max_x = 0.0;
        let mut max_y = 0.0;

        for index in 0..count {
            let screen: id = msg_send![screens, objectAtIndex: index];
            let frame: NSRect = msg_send![screen, frame];
            let left = frame.origin.x;
            let bottom = frame.origin.y;
            let right = frame.origin.x + frame.size.width;
            let top = frame.origin.y + frame.size.height;

            if index == 0 {
                min_x = left;
                min_y = bottom;
                max_x = right;
                max_y = top;
            } else {
                min_x = min_x.min(left);
                min_y = min_y.min(bottom);
                max_x = max_x.max(right);
                max_y = max_y.max(top);
            }
        }

        let point: NSPoint = msg_send![class!(NSEvent), mouseLocation];
        let width = (max_x - min_x).max(1.0);
        let height = (max_y - min_y).max(1.0);
        let x_ratio = ((point.x - min_x) / width).clamp(0.0, 1.0);
        let y_ratio = (1.0 - ((point.y - min_y) / height)).clamp(0.0, 1.0);

        (x_ratio, y_ratio)
    }

    if INPUT_MONITOR_STARTED.load(Ordering::SeqCst) {
        return;
    }

    if !crate::screenshot::has_accessibility_permission(false) {
        log::warn!("桌宠输入联动未启动：缺少辅助功能权限");
        return;
    }

    if INPUT_MONITOR_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }

    let run_result = app.run_on_main_thread(move || unsafe {
        MACOS_INPUT_MONITOR.with(|slot| {
            if slot.borrow().is_some() {
                return;
            }

            let event_class = class!(NSEvent);
            let keyboard_handler = ConcreteBlock::new(|event: id| {
                let key_code: u16 = msg_send![event, keyCode];
                record_keyboard_input(standard_keyboard_group_from_key_code(key_code), key_code);
            })
            .copy();
            let mouse_handler = ConcreteBlock::new(|event: id| {
                let event_type_raw: usize = msg_send![event, type];
                let event_type = std::mem::transmute::<usize, NSEventType>(event_type_raw);
                record_mouse_input(mouse_group_from_event_type(event_type));
                let (cursor_ratio_x, cursor_ratio_y) = current_cursor_ratio();
                record_cursor_ratio(cursor_ratio_x, cursor_ratio_y);
            })
            .copy();

            let keyboard_mask =
                (NSEventMask::NSKeyDownMask | NSEventMask::NSFlagsChangedMask).bits();
            let mouse_mask = (NSEventMask::NSLeftMouseDownMask
                | NSEventMask::NSRightMouseDownMask
                | NSEventMask::NSOtherMouseDownMask
                | NSEventMask::NSMouseMovedMask
                | NSEventMask::NSScrollWheelMask)
                .bits();

            let keyboard_monitor: id = msg_send![
                event_class,
                addGlobalMonitorForEventsMatchingMask: keyboard_mask
                handler: &*keyboard_handler
            ];
            let mouse_monitor: id = msg_send![
                event_class,
                addGlobalMonitorForEventsMatchingMask: mouse_mask
                handler: &*mouse_handler
            ];

            if keyboard_monitor == nil || mouse_monitor == nil {
                log::warn!("桌宠输入联动注册失败：系统未返回有效的全局监听句柄");
                return;
            }

            slot.replace(Some(MacosInputMonitor {
                keyboard_monitor,
                mouse_monitor,
                keyboard_handler,
                mouse_handler,
            }));
        });
    });

    if let Err(e) = run_result {
        INPUT_MONITOR_STARTED.store(false, Ordering::SeqCst);
        log::warn!("桌宠输入联动注册失败: {e}");
    }
}

#[cfg(not(target_os = "macos"))]
pub fn start_avatar_input_monitor(_app: &AppHandle) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 输入活跃窗口应只在短时间内保持有效() {
        assert!(!is_input_still_active(0, 1000, 180));
        assert!(is_input_still_active(900, 1000, 180));
        assert!(!is_input_still_active(700, 1000, 180));
    }

    #[test]
    fn 输入载荷应根据最近输入时间生成键鼠活跃状态() {
        LAST_KEYBOARD_INPUT_AT_MS.store(1000, Ordering::Relaxed);
        LAST_MOUSE_INPUT_AT_MS.store(850, Ordering::Relaxed);
        LAST_KEYBOARD_GROUP_CODE.store(KEYBOARD_GROUP_KEY_Q, Ordering::Relaxed);
        LAST_KEYBOARD_KEY_CODE.store(12, Ordering::Relaxed);
        LAST_MOUSE_GROUP_CODE.store(MOUSE_GROUP_RIGHT, Ordering::Relaxed);
        CURSOR_RATIO_X_PERMILLE.store(250, Ordering::Relaxed);
        CURSOR_RATIO_Y_PERMILLE.store(750, Ordering::Relaxed);

        let payload = build_avatar_input_payload(1030);
        assert!(payload.keyboard_active);
        assert!(payload.mouse_active);
        assert_eq!(payload.keyboard_group, "key-q");
        assert_eq!(payload.keyboard_visual_key, "KeyQ");
        assert_eq!(payload.mouse_group, "mouse-right");
        assert_eq!(payload.cursor_ratio_x, 0.25);
        assert_eq!(payload.cursor_ratio_y, 0.75);

        let payload = build_avatar_input_payload(1200);
        assert!(!payload.keyboard_active);
        assert!(!payload.mouse_active);
    }

    #[test]
    fn 键盘键码应映射到原版键区分组() {
        assert_eq!(standard_keyboard_group_from_key_code(18), KEYBOARD_GROUP_DIGIT_1);
        assert_eq!(standard_keyboard_group_from_key_code(26), KEYBOARD_GROUP_DIGIT_7);
        assert_eq!(standard_keyboard_group_from_key_code(12), KEYBOARD_GROUP_KEY_Q);
        assert_eq!(standard_keyboard_group_from_key_code(14), KEYBOARD_GROUP_KEY_E);
        assert_eq!(standard_keyboard_group_from_key_code(15), KEYBOARD_GROUP_KEY_R);
        assert_eq!(standard_keyboard_group_from_key_code(49), KEYBOARD_GROUP_SPACE);
        assert_eq!(standard_keyboard_group_from_key_code(0), KEYBOARD_GROUP_KEY_A);
        assert_eq!(standard_keyboard_group_from_key_code(2), KEYBOARD_GROUP_KEY_D);
        assert_eq!(standard_keyboard_group_from_key_code(1), KEYBOARD_GROUP_KEY_S);
        assert_eq!(standard_keyboard_group_from_key_code(13), KEYBOARD_GROUP_KEY_W);
        assert_eq!(standard_keyboard_group_from_key_code(123), 0);
    }

    #[test]
    fn 键盘键码应映射到源资源图层名称() {
        assert_eq!(keyboard_visual_key_from_key_code(0), "KeyA");
        assert_eq!(keyboard_visual_key_from_key_code(45), "KeyN");
        assert_eq!(keyboard_visual_key_from_key_code(31), "KeyO");
        assert_eq!(keyboard_visual_key_from_key_code(35), "KeyP");
        assert_eq!(keyboard_visual_key_from_key_code(25), "Num9");
        assert_eq!(keyboard_visual_key_from_key_code(49), "Space");
        assert_eq!(keyboard_visual_key_from_key_code(36), "Return");
        assert_eq!(keyboard_visual_key_from_key_code(56), "ShiftLeft");
        assert_eq!(keyboard_visual_key_from_key_code(62), "ControlRight");
        assert_eq!(keyboard_visual_key_from_key_code(43), "Comma");
        assert_eq!(keyboard_visual_key_from_key_code(47), "Period");
        assert_eq!(keyboard_visual_key_from_key_code(123), "LeftArrow");
        assert_eq!(keyboard_visual_key_from_key_code(124), "RightArrow");
        assert_eq!(keyboard_visual_key_from_key_code(125), "DownArrow");
        assert_eq!(keyboard_visual_key_from_key_code(126), "UpArrow");
        assert_eq!(keyboard_visual_key_from_key_code(127), "");
    }

    #[test]
    fn 鼠标分组标签应映射到原版鼠标模式名称() {
        assert_eq!(mouse_group_label(MOUSE_GROUP_MOVE), "mouse-move");
        assert_eq!(mouse_group_label(MOUSE_GROUP_LEFT), "mouse-left");
        assert_eq!(mouse_group_label(MOUSE_GROUP_RIGHT), "mouse-right");
        assert_eq!(mouse_group_label(MOUSE_GROUP_SIDE), "mouse-side");
    }
}
