use crate::engine::keys::VirtualKey;
use crate::engine::ModifierState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImeState {
    Idle,
    Composing,
    Selecting,
}

#[derive(Debug, Clone)]
pub struct FsmInput {
    pub key: VirtualKey,
    pub mods: ModifierState,
    pub buffer_empty: bool,
    pub has_candidates: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FsmEffect {
    PassThrough,
    Consume,
    Alert,
    UpdateLookup,
    Commit首选,
    CommitRaw,
    Clear,
}

/// 形式化状态机处理器
pub struct StateMachine;

impl StateMachine {
    /// 状态转移核心函数：(当前状态, 输入) -> (新状态, 动作)
    pub fn transition(current: ImeState, input: &FsmInput) -> (ImeState, FsmEffect) {
        match current {
            ImeState::Idle => Self::handle_idle(input),
            ImeState::Composing => Self::handle_composing(input),
            ImeState::Selecting => Self::handle_selecting(input),
        }
    }

    fn handle_idle(input: &FsmInput) -> (ImeState, FsmEffect) {
        if Self::is_coding_key(input.key) {
            (ImeState::Composing, FsmEffect::UpdateLookup)
        } else {
            (ImeState::Idle, FsmEffect::PassThrough)
        }
    }

    fn handle_composing(input: &FsmInput) -> (ImeState, FsmEffect) {
        if input.buffer_empty {
            if input.key == VirtualKey::Backspace {
                return (ImeState::Idle, FsmEffect::Alert);
            }
            return (ImeState::Idle, FsmEffect::Consume);
        }

        // 处理组合键
        if input.mods.ctrl {
            return (ImeState::Composing, FsmEffect::Consume);
        }

        match input.key {
            VirtualKey::Space => (ImeState::Idle, FsmEffect::Commit首选),
            VirtualKey::Enter => (ImeState::Idle, FsmEffect::CommitRaw),
            VirtualKey::Backspace => (ImeState::Composing, FsmEffect::UpdateLookup),
            VirtualKey::Esc | VirtualKey::Delete => (ImeState::Idle, FsmEffect::Clear),
            k if Self::is_selection_key(k) && input.has_candidates => {
                (ImeState::Selecting, Self::map_selection_effect(k))
            }
            // 处理字母按键（包含 Shift 辅助码）
            k if Self::is_letter(k) => (ImeState::Composing, FsmEffect::UpdateLookup),
            k if matches!(k, VirtualKey::Apostrophe | VirtualKey::Semicolon) => {
                (ImeState::Composing, FsmEffect::UpdateLookup)
            }
            _ => (ImeState::Composing, FsmEffect::Consume),
        }
    }

    fn handle_selecting(input: &FsmInput) -> (ImeState, FsmEffect) {
        if input.buffer_empty {
            return (ImeState::Idle, FsmEffect::Consume);
        }

        match input.key {
            VirtualKey::Space => (ImeState::Idle, FsmEffect::Commit首选),
            VirtualKey::Enter => (ImeState::Idle, FsmEffect::CommitRaw),
            k if Self::is_selection_key(k) => (ImeState::Selecting, Self::map_selection_effect(k)),
            k if Self::is_letter(k) => (ImeState::Composing, FsmEffect::UpdateLookup),
            VirtualKey::Backspace => (ImeState::Composing, FsmEffect::UpdateLookup),
            VirtualKey::Esc => (ImeState::Idle, FsmEffect::Clear),
            _ => (ImeState::Selecting, FsmEffect::Consume),
        }
    }

    fn is_coding_key(key: VirtualKey) -> bool {
        Self::is_letter(key) || matches!(key, VirtualKey::Apostrophe | VirtualKey::Semicolon)
    }

    fn is_letter(key: VirtualKey) -> bool {
        matches!(
            key,
            VirtualKey::A
                | VirtualKey::B
                | VirtualKey::C
                | VirtualKey::D
                | VirtualKey::E
                | VirtualKey::F
                | VirtualKey::G
                | VirtualKey::H
                | VirtualKey::I
                | VirtualKey::J
                | VirtualKey::K
                | VirtualKey::L
                | VirtualKey::M
                | VirtualKey::N
                | VirtualKey::O
                | VirtualKey::P
                | VirtualKey::Q
                | VirtualKey::R
                | VirtualKey::S
                | VirtualKey::T
                | VirtualKey::U
                | VirtualKey::V
                | VirtualKey::W
                | VirtualKey::X
                | VirtualKey::Y
                | VirtualKey::Z
        )
    }

    fn is_selection_key(key: VirtualKey) -> bool {
        matches!(
            key,
            VirtualKey::Up
                | VirtualKey::Down
                | VirtualKey::Left
                | VirtualKey::Right
                | VirtualKey::PageUp
                | VirtualKey::PageDown
                | VirtualKey::Minus
                | VirtualKey::Equal
                | VirtualKey::Comma
                | VirtualKey::Dot
        )
    }

    fn map_selection_effect(_key: VirtualKey) -> FsmEffect {
        FsmEffect::Consume
    }
}
