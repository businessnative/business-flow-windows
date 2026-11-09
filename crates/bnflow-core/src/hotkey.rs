use crate::models::{ModifierClass, ModifierKey, ShortcutSettings, ShortcutTrigger};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DictationKind {
    Dictation,
    Command,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HotkeyAction {
    Start { kind: DictationKind, hands_free: bool },
    SwitchKind(DictationKind),
    LockHandsFree,
    Stop,
    Cancel,
    PasteLast,
    Polish,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HotkeyState {
    Idle,
    Holding { kind: DictationKind, since: f64, via_mouse: Option<i32> },
    HandsFree(DictationKind),
}

pub struct HotkeyEngine {
    pub shortcuts: ShortcutSettings,
    pub state: HotkeyState,
    pub minimum_hold: f64,
    pressed: HashSet<ModifierKey>,
    pending_hands_free_stop: bool,
}

impl HotkeyEngine {
    pub fn new(shortcuts: ShortcutSettings) -> Self {
        Self { shortcuts, state: HotkeyState::Idle, minimum_hold: 0.18, pressed: HashSet::new(), pending_hands_free_stop: false }
    }

    pub fn reset(&mut self) {
        self.state = HotkeyState::Idle;
    }

    pub fn set_shortcuts(&mut self, shortcuts: ShortcutSettings) {
        self.shortcuts = shortcuts;
    }

    fn hold_chords(&self) -> Vec<(DictationKind, HashSet<ModifierKey>)> {
        let mut r = Vec::new();
        for t in &self.shortcuts.command {
            if let ShortcutTrigger::Hold { keys } = t {
                r.push((DictationKind::Command, keys.iter().copied().collect()));
            }
        }
        for t in &self.shortcuts.push_to_talk {
            if let ShortcutTrigger::Hold { keys } = t {
                r.push((DictationKind::Dictation, keys.iter().copied().collect()));
            }
        }
        r
    }

    fn classes(s: &HashSet<ModifierKey>) -> HashSet<ModifierClass> {
        s.iter().map(|k| k.modifier_class()).collect()
    }

    pub fn modifiers_changed(&mut self, now: HashSet<ModifierKey>, at: f64) -> Vec<HotkeyAction> {
        let before = self.pressed.clone();
        self.pressed = now.clone();
        match self.state.clone() {
            HotkeyState::Idle => {
                if self.pending_hands_free_stop && now.is_empty() {
                    self.pending_hands_free_stop = false;
                    return vec![];
                }
                if self.pending_hands_free_stop {
                    return vec![];
                }
                if let Some((kind, _)) = self.hold_chords().into_iter().find(|(_, chord)| chord == &now) {
                    self.state = HotkeyState::Holding { kind, since: at, via_mouse: None };
                    return vec![HotkeyAction::Start { kind, hands_free: false }];
                }
                vec![]
            }
            HotkeyState::Holding { kind, since, via_mouse } => {
                if via_mouse.is_some() {
                    return vec![];
                }
                if let Some((new_kind, _)) = self.hold_chords().into_iter().find(|(_, chord)| chord == &now) {
                    if new_kind != kind {
                        self.state = HotkeyState::Holding { kind: new_kind, since, via_mouse: None };
                        return vec![HotkeyAction::SwitchKind(new_kind)];
                    }
                }
                let chord = self.hold_chords().into_iter().find(|(k, _)| *k == kind).map(|(_, c)| c).unwrap_or_default();
                if !chord.is_subset(&now) {
                    self.state = HotkeyState::Idle;
                    return vec![if at - since < self.minimum_hold { HotkeyAction::Cancel } else { HotkeyAction::Stop }];
                }
                vec![]
            }
            HotkeyState::HandsFree(_) => {
                let newly: HashSet<_> = now.difference(&before).copied().collect();
                if !newly.is_empty() && self.hold_chords().iter().any(|(_, chord)| chord == &now) {
                    self.state = HotkeyState::Idle;
                    self.pending_hands_free_stop = true;
                    return vec![HotkeyAction::Stop];
                }
                vec![]
            }
        }
    }

    pub fn key_down(&mut self, key_code: u16, is_repeat: bool, _at: f64) -> (Vec<HotkeyAction>, bool) {
        let cls = Self::classes(&self.pressed);
        let matches = |list: &[ShortcutTrigger]| {
            list.iter().any(|t| matches!(t, ShortcutTrigger::Combo { key_code: c, modifiers } if *c == key_code && modifiers.iter().copied().collect::<HashSet<_>>() == cls))
        };
        if key_code == 53 {
            return match self.state {
                HotkeyState::Idle => (vec![], false),
                _ => {
                    self.state = HotkeyState::Idle;
                    (vec![HotkeyAction::Cancel], true)
                }
            };
        }
        match self.state.clone() {
            HotkeyState::Holding { via_mouse, kind, .. } => {
                if via_mouse.is_none() && matches(&self.shortcuts.hands_free) {
                    self.state = HotkeyState::HandsFree(kind);
                    return (vec![HotkeyAction::LockHandsFree], true);
                }
                if via_mouse.is_none() {
                    self.state = HotkeyState::Idle;
                    return (vec![HotkeyAction::Cancel], false);
                }
                (vec![], false)
            }
            HotkeyState::HandsFree(_) => {
                if matches(&self.shortcuts.hands_free) {
                    if is_repeat {
                        return (vec![], true);
                    }
                    self.state = HotkeyState::Idle;
                    return (vec![HotkeyAction::Stop], true);
                }
                (vec![], false)
            }
            HotkeyState::Idle => {
                if is_repeat {
                    let consume = matches(&self.shortcuts.hands_free) || matches(&self.shortcuts.paste_last) || matches(&self.shortcuts.polish);
                    return (vec![], consume);
                }
                if self.pending_hands_free_stop && matches(&self.shortcuts.hands_free) {
                    self.pending_hands_free_stop = false;
                    return (vec![], true);
                }
                if matches(&self.shortcuts.hands_free) {
                    self.state = HotkeyState::HandsFree(DictationKind::Dictation);
                    return (vec![HotkeyAction::Start { kind: DictationKind::Dictation, hands_free: true }], true);
                }
                if matches(&self.shortcuts.paste_last) {
                    return (vec![HotkeyAction::PasteLast], true);
                }
                if matches(&self.shortcuts.polish) {
                    return (vec![HotkeyAction::Polish], true);
                }
                (vec![], false)
            }
        }
    }

    pub fn key_up(&mut self, key_code: u16) -> bool {
        if matches!(self.state, HotkeyState::HandsFree(_)) {
            return self.shortcuts.hands_free.iter().any(|t| matches!(t, ShortcutTrigger::Combo { key_code: c, .. } if *c == key_code));
        }
        if self.pending_hands_free_stop {
            self.pending_hands_free_stop = false;
        }
        false
    }

    pub fn mouse_down(&mut self, button: i32, at: f64) -> (Vec<HotkeyAction>, bool) {
        let ptt = self.shortcuts.push_to_talk.iter().any(|t| matches!(t, ShortcutTrigger::Mouse { button: b } if *b == button));
        let hf = self.shortcuts.hands_free.iter().any(|t| matches!(t, ShortcutTrigger::Mouse { button: b } if *b == button));
        let cmd = self.shortcuts.command.iter().any(|t| matches!(t, ShortcutTrigger::Mouse { button: b } if *b == button));
        if !(ptt || hf || cmd) {
            return (vec![], false);
        }
        match self.state {
            HotkeyState::Idle => {
                if hf {
                    self.state = HotkeyState::HandsFree(DictationKind::Dictation);
                    return (vec![HotkeyAction::Start { kind: DictationKind::Dictation, hands_free: true }], true);
                }
                let kind = if cmd { DictationKind::Command } else { DictationKind::Dictation };
                self.state = HotkeyState::Holding { kind, since: at, via_mouse: Some(button) };
                (vec![HotkeyAction::Start { kind, hands_free: false }], true)
            }
            HotkeyState::HandsFree(_) => {
                self.state = HotkeyState::Idle;
                (vec![HotkeyAction::Stop], true)
            }
            HotkeyState::Holding { .. } => (vec![], true),
        }
    }

    pub fn mouse_up(&mut self, button: i32, at: f64) -> (Vec<HotkeyAction>, bool) {
        if let HotkeyState::Holding { since, via_mouse, .. } = self.state {
            if via_mouse == Some(button) {
                self.state = HotkeyState::Idle;
                let action = if at - since < self.minimum_hold { HotkeyAction::Cancel } else { HotkeyAction::Stop };
                return (vec![action], true);
            }
        }
        let configured = [&self.shortcuts.push_to_talk, &self.shortcuts.hands_free, &self.shortcuts.command]
            .iter()
            .any(|list| list.iter().any(|t| matches!(t, ShortcutTrigger::Mouse { button: b } if *b == button)));
        (vec![], configured)
    }

    pub fn bar_clicked(&mut self) -> Vec<HotkeyAction> {
        match self.state {
            HotkeyState::Idle => {
                self.state = HotkeyState::HandsFree(DictationKind::Dictation);
                vec![HotkeyAction::Start { kind: DictationKind::Dictation, hands_free: true }]
            }
            HotkeyState::HandsFree(_) | HotkeyState::Holding { .. } => {
                self.state = HotkeyState::Idle;
                vec![HotkeyAction::Stop]
            }
        }
    }
}
