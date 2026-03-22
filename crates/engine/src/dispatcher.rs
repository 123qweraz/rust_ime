use crate::keys::VirtualKey;
use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModifierState {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum InputEvent {
    Key {
        key: VirtualKey,
        val: i32, // 1: Press, 0: Release, 2: Repeat
        shift: bool,
        ctrl: bool,
        alt: bool,
    },
    Voice(String),
    CandidateSelect(usize), // 点击或直接选择候选词
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    NextPage,
    PrevPage,
    NextCandidate,
    PrevCandidate,
    Select(usize),
    Commit,
    CommitRaw,
    Clear,
}

pub struct KeyDispatcher {
    pub key_map: HashMap<(VirtualKey, ModifierState), Command>,

    // 双击检测状态
    pub last_tap_key: Option<VirtualKey>,
    pub last_tap_time: Option<Instant>,

    // 长按检测状态
    pub key_press_info: Option<(VirtualKey, Instant)>,
    pub long_press_triggered: bool,
}

impl KeyDispatcher {
    pub fn new() -> Self {
        Self {
            key_map: HashMap::new(),
            last_tap_key: None,
            last_tap_time: None,
            key_press_info: None,
            long_press_triggered: false,
        }
    }

    #[allow(dead_code)]
    pub fn reset_states(&mut self) {
        self.last_tap_key = None;
        self.last_tap_time = None;
        self.key_press_info = None;
        self.long_press_triggered = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::VirtualKey;

    #[test]
    fn test_dispatcher_key_map() {
        let mut dispatcher = KeyDispatcher::new();
        let none = ModifierState {
            shift: false,
            ctrl: false,
            alt: false,
            meta: false,
        };

        dispatcher
            .key_map
            .insert((VirtualKey::Space, none), Command::Commit);

        assert_eq!(
            dispatcher.key_map.get(&(VirtualKey::Space, none)),
            Some(&Command::Commit)
        );
        assert_eq!(dispatcher.key_map.get(&(VirtualKey::A, none)), None);
    }

    #[test]
    fn test_dispatcher_reset() {
        let mut dispatcher = KeyDispatcher::new();
        dispatcher.long_press_triggered = true;
        dispatcher.reset_states();
        assert!(!dispatcher.long_press_triggered);
        assert!(dispatcher.last_tap_key.is_none());
    }

    #[test]
    fn test_modifier_state() {
        let none = ModifierState {
            shift: false,
            ctrl: false,
            alt: false,
            meta: false,
        };
        let with_shift = ModifierState {
            shift: true,
            ctrl: false,
            alt: false,
            meta: false,
        };
        let with_ctrl = ModifierState {
            shift: false,
            ctrl: true,
            alt: false,
            meta: false,
        };

        assert_ne!(none, with_shift);
        assert_ne!(none, with_ctrl);
        assert_ne!(with_shift, with_ctrl);
    }

    #[test]
    fn test_command_variants() {
        use crate::Command;

        let commit = Command::Commit;
        let clear = Command::Clear;
        let select = Command::Select(0);
        let next_page = Command::NextPage;
        let prev_page = Command::PrevPage;

        assert_ne!(commit, clear);
        assert_ne!(commit, select);
        assert_ne!(select, next_page);
        assert_ne!(next_page, prev_page);
    }

    #[test]
    fn test_input_event() {
        use crate::InputEvent;

        let key_event = InputEvent::Key {
            key: crate::keys::VirtualKey::A,
            val: 1,
            shift: false,
            ctrl: false,
            alt: false,
        };

        if let InputEvent::Key { key, val, .. } = key_event {
            assert_eq!(key, crate::keys::VirtualKey::A);
            assert_eq!(val, 1);
        }
    }
}
