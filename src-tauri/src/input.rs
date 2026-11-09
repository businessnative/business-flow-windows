use bnflow_core::{HotkeyAction, HotkeyEngine, ModifierKey, ShortcutSettings};
#[cfg(any(windows, target_os = "macos"))]
use rdev::Event;
use rdev::{EventType, Key};
use std::collections::HashSet;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Instant;

pub enum HookCmd {
    Event { down: bool, key: Key, reply: Sender<bool> },
    UiToggle,
    Reload(ShortcutSettings),
}

pub enum DictationJob {
    Start { hands_free: bool },
    Stop,
    Cancel,
    PasteLast,
}

pub fn spawn(shortcuts: ShortcutSettings, jobs: Sender<DictationJob>) -> Sender<HookCmd> {
    let (tx, rx) = mpsc::channel();
    let hook_tx = tx.clone();
    std::thread::Builder::new().name("hotkey".into()).spawn(move || hook_loop(rx, shortcuts, jobs)).ok();
    std::thread::Builder::new().name("keys".into()).spawn(move || listen(hook_tx)).ok();
    tx
}

fn ask(tx: &Sender<HookCmd>, down: bool, key: Key) -> bool {
    let (reply_tx, reply_rx) = mpsc::channel();
    if tx.send(HookCmd::Event { down, key, reply: reply_tx }).is_err() {
        return false;
    }
    reply_rx.recv_timeout(std::time::Duration::from_millis(40)).unwrap_or(false)
}

fn listen(tx: Sender<HookCmd>) {
    #[cfg(any(windows, target_os = "macos"))]
    {
        let tx_grab = tx.clone();
        if rdev::grab(move |event: Event| {
            let consume = match event.event_type {
                EventType::KeyPress(key) => ask(&tx_grab, true, key),
                EventType::KeyRelease(key) => ask(&tx_grab, false, key),
                _ => false,
            };
            if consume { None } else { Some(event) }
        })
        .is_ok()
        {
            return;
        }
    }
    let _ = rdev::listen(move |event| match event.event_type {
        EventType::KeyPress(key) => { let _ = ask(&tx, true, key); }
        EventType::KeyRelease(key) => { let _ = ask(&tx, false, key); }
        _ => {}
    });
}

fn hook_loop(rx: Receiver<HookCmd>, shortcuts: ShortcutSettings, jobs: Sender<DictationJob>) {
    let mut engine = HotkeyEngine::new(shortcuts);
    let mut mods = HashSet::new();
    let mut held_keys: HashSet<u16> = HashSet::new();
    let t0 = Instant::now();
    let now = |t0: Instant| t0.elapsed().as_secs_f64();
    while let Ok(cmd) = rx.recv() {
        match cmd {
            HookCmd::Reload(s) => engine.set_shortcuts(s),
            HookCmd::UiToggle => dispatch(&jobs, engine.bar_clicked()),
            HookCmd::Event { down, key, reply } => {
                let consume = if let Some(modifier) = modifier_of(key) {
                    if down { mods.insert(modifier); } else { mods.remove(&modifier); }
                    dispatch(&jobs, engine.modifiers_changed(mods.clone(), now(t0)));
                    false
                } else if down {
                    let code = mac_code(key).unwrap_or(0);
                    let repeat = code != 0 && !held_keys.insert(code);
                    let (actions, consume) = engine.key_down(code, repeat, now(t0));
                    dispatch(&jobs, actions);
                    consume
                } else {
                    if let Some(code) = mac_code(key) {
                        held_keys.remove(&code);
                        engine.key_up(code)
                    } else {
                        false
                    }
                };
                let _ = reply.send(consume);
            }
        }
    }
}

fn dispatch(jobs: &Sender<DictationJob>, actions: Vec<HotkeyAction>) {
    for action in actions {
        let job = match action {
            HotkeyAction::Start { hands_free, .. } => Some(DictationJob::Start { hands_free }),
            HotkeyAction::LockHandsFree => None,
            HotkeyAction::Stop => Some(DictationJob::Stop),
            HotkeyAction::Cancel => Some(DictationJob::Cancel),
            HotkeyAction::PasteLast => Some(DictationJob::PasteLast),
            HotkeyAction::SwitchKind(_) | HotkeyAction::Polish => None,
        };
        if let Some(job) = job {
            let _ = jobs.send(job);
        }
    }
}

fn modifier_of(key: Key) -> Option<ModifierKey> {
    Some(match key {
        Key::Function => ModifierKey::Fn,
        Key::ControlLeft => ModifierKey::LeftControl,
        Key::ControlRight => ModifierKey::RightControl,
        Key::Alt => ModifierKey::LeftOption,
        Key::AltGr => ModifierKey::RightOption,
        Key::MetaLeft => ModifierKey::LeftCommand,
        Key::MetaRight => ModifierKey::RightCommand,
        Key::ShiftLeft => ModifierKey::LeftShift,
        Key::ShiftRight => ModifierKey::RightShift,
        _ => return None,
    })
}

fn mac_code(key: Key) -> Option<u16> {
    Some(match key {
        Key::KeyA => 0,
        Key::KeyS => 1,
        Key::KeyD => 2,
        Key::KeyF => 3,
        Key::KeyH => 4,
        Key::KeyG => 5,
        Key::KeyZ => 6,
        Key::KeyX => 7,
        Key::KeyC => 8,
        Key::KeyV => 9,
        Key::KeyB => 11,
        Key::KeyQ => 12,
        Key::KeyW => 13,
        Key::KeyE => 14,
        Key::KeyR => 15,
        Key::KeyY => 16,
        Key::KeyT => 17,
        Key::Num1 => 18,
        Key::Num2 => 19,
        Key::Num3 => 20,
        Key::Num4 => 21,
        Key::Num6 => 22,
        Key::Num5 => 23,
        Key::Equal => 24,
        Key::Num9 => 25,
        Key::Num7 => 26,
        Key::Minus => 27,
        Key::Num8 => 28,
        Key::Num0 => 29,
        Key::RightBracket => 30,
        Key::KeyO => 31,
        Key::KeyU => 32,
        Key::LeftBracket => 33,
        Key::KeyI => 34,
        Key::KeyP => 35,
        Key::Return => 36,
        Key::KeyL => 37,
        Key::KeyJ => 38,
        Key::Quote => 39,
        Key::KeyK => 40,
        Key::SemiColon => 41,
        Key::BackSlash => 42,
        Key::Comma => 43,
        Key::Slash => 44,
        Key::KeyN => 45,
        Key::KeyM => 46,
        Key::Dot => 47,
        Key::Tab => 48,
        Key::Space => 49,
        Key::BackQuote => 50,
        Key::Backspace => 51,
        Key::Escape => 53,
        Key::MetaRight => 54,
        Key::MetaLeft => 55,
        Key::ShiftLeft => 56,
        Key::Alt => 58,
        Key::ControlLeft => 59,
        Key::ShiftRight => 60,
        Key::AltGr => 61,
        Key::ControlRight => 62,
        Key::Function => 63,
        Key::F5 => 96,
        Key::F6 => 97,
        Key::F7 => 98,
        Key::F3 => 99,
        Key::F8 => 100,
        Key::F9 => 101,
        Key::F11 => 103,
        Key::F10 => 109,
        Key::F12 => 111,
        Key::F4 => 118,
        Key::F2 => 120,
        Key::F1 => 122,
        Key::LeftArrow => 123,
        Key::RightArrow => 124,
        Key::DownArrow => 125,
        Key::UpArrow => 126,
        _ => return None,
    })
}
