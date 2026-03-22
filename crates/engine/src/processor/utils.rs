use crate::keys::VirtualKey;

pub fn is_letter(key: VirtualKey) -> bool {
    matches!(
        key,
        VirtualKey::Q
            | VirtualKey::W
            | VirtualKey::E
            | VirtualKey::R
            | VirtualKey::T
            | VirtualKey::Y
            | VirtualKey::U
            | VirtualKey::I
            | VirtualKey::O
            | VirtualKey::P
            | VirtualKey::A
            | VirtualKey::S
            | VirtualKey::D
            | VirtualKey::F
            | VirtualKey::G
            | VirtualKey::H
            | VirtualKey::J
            | VirtualKey::K
            | VirtualKey::L
            | VirtualKey::Z
            | VirtualKey::X
            | VirtualKey::C
            | VirtualKey::V
            | VirtualKey::B
            | VirtualKey::N
            | VirtualKey::M
    )
}

pub fn is_digit(key: VirtualKey) -> bool {
    matches!(
        key,
        VirtualKey::Digit1
            | VirtualKey::Digit2
            | VirtualKey::Digit3
            | VirtualKey::Digit4
            | VirtualKey::Digit5
            | VirtualKey::Digit6
            | VirtualKey::Digit7
            | VirtualKey::Digit8
            | VirtualKey::Digit9
            | VirtualKey::Digit0
    )
}

pub fn key_to_digit(key: VirtualKey) -> Option<usize> {
    match key {
        VirtualKey::Digit1 => Some(1),
        VirtualKey::Digit2 => Some(2),
        VirtualKey::Digit3 => Some(3),
        VirtualKey::Digit4 => Some(4),
        VirtualKey::Digit5 => Some(5),
        VirtualKey::Digit6 => Some(6),
        VirtualKey::Digit7 => Some(7),
        VirtualKey::Digit8 => Some(8),
        VirtualKey::Digit9 => Some(9),
        VirtualKey::Digit0 => Some(0),
        _ => None,
    }
}

pub fn key_to_char(key: VirtualKey, shift: bool, caps_lock: bool) -> Option<char> {
    let c = match key {
        VirtualKey::Q => Some('q'),
        VirtualKey::W => Some('w'),
        VirtualKey::E => Some('e'),
        VirtualKey::R => Some('r'),
        VirtualKey::T => Some('t'),
        VirtualKey::Y => Some('y'),
        VirtualKey::U => Some('u'),
        VirtualKey::I => Some('i'),
        VirtualKey::O => Some('o'),
        VirtualKey::P => Some('p'),
        VirtualKey::A => Some('a'),
        VirtualKey::S => Some('s'),
        VirtualKey::D => Some('d'),
        VirtualKey::F => Some('f'),
        VirtualKey::G => Some('g'),
        VirtualKey::H => Some('h'),
        VirtualKey::J => Some('j'),
        VirtualKey::K => Some('k'),
        VirtualKey::L => Some('l'),
        VirtualKey::Z => Some('z'),
        VirtualKey::X => Some('x'),
        VirtualKey::C => Some('c'),
        VirtualKey::V => Some('v'),
        VirtualKey::B => Some('b'),
        VirtualKey::N => Some('n'),
        VirtualKey::M => Some('m'),
        VirtualKey::Apostrophe => Some('\''),
        VirtualKey::Slash => Some('/'),
        VirtualKey::Minus => Some('-'),
        VirtualKey::Equal => Some('='),
        VirtualKey::Comma => Some(','),
        VirtualKey::Dot => Some('.'),
        VirtualKey::Digit1 => Some('1'),
        VirtualKey::Digit2 => Some('2'),
        VirtualKey::Digit3 => Some('3'),
        VirtualKey::Digit4 => Some('4'),
        VirtualKey::Digit5 => Some('5'),
        VirtualKey::Digit6 => Some('6'),
        VirtualKey::Digit7 => Some('7'),
        VirtualKey::Digit8 => Some('8'),
        VirtualKey::Digit9 => Some('9'),
        VirtualKey::Digit0 => Some('0'),
        VirtualKey::Grave => Some('`'),
        VirtualKey::LeftBrace => Some('['),
        VirtualKey::RightBrace => Some(']'),
        VirtualKey::Backslash => Some('\\'),
        VirtualKey::Semicolon => Some(';'),
        _ => None,
    };

    // 逻辑：如果是字母，(Shift XOR CapsLock) 为真则大写；
    // 如果不是字母，仅取决于 Shift。
    if is_letter(key) {
        if shift ^ caps_lock {
            c.map(|ch| ch.to_ascii_uppercase())
        } else {
            c
        }
    } else if shift {
        match key {
            VirtualKey::Digit1 => Some('!'),
            VirtualKey::Digit2 => Some('@'),
            VirtualKey::Digit3 => Some('#'),
            VirtualKey::Digit4 => Some('$'),
            VirtualKey::Digit5 => Some('%'),
            VirtualKey::Digit6 => Some('^'),
            VirtualKey::Digit7 => Some('&'),
            VirtualKey::Digit8 => Some('*'),
            VirtualKey::Digit9 => Some('('),
            VirtualKey::Digit0 => Some(')'),
            VirtualKey::Minus => Some('_'),
            VirtualKey::Equal => Some('+'),
            VirtualKey::Comma => Some('<'),
            VirtualKey::Dot => Some('>'),
            VirtualKey::Slash => Some('?'),
            VirtualKey::Grave => Some('~'),
            VirtualKey::LeftBrace => Some('{'),
            VirtualKey::RightBrace => Some('}'),
            VirtualKey::Backslash => Some('|'),
            VirtualKey::Semicolon => Some(':'),
            VirtualKey::Apostrophe => Some('"'),
            _ => c,
        }
    } else {
        c
    }
}

pub fn get_punctuation_key(key: VirtualKey, shift: bool) -> Option<&'static str> {
    match (key, shift) {
        (VirtualKey::Space, _) => Some(" "),
        (VirtualKey::Grave, false) => Some("`"),
        (VirtualKey::Grave, true) => Some("~"),
        (VirtualKey::Minus, false) => Some("-"),
        (VirtualKey::Minus, true) => Some("_"),
        (VirtualKey::Equal, false) => Some("="),
        (VirtualKey::Equal, true) => Some("+"),
        (VirtualKey::LeftBrace, false) => Some("["),
        (VirtualKey::LeftBrace, true) => Some("{"),
        (VirtualKey::RightBrace, false) => Some("]"),
        (VirtualKey::RightBrace, true) => Some("}"),
        (VirtualKey::Backslash, false) => Some("\\"),
        (VirtualKey::Backslash, true) => Some("|"),
        (VirtualKey::Semicolon, false) => Some(";"),
        (VirtualKey::Semicolon, true) => Some(":"),
        (VirtualKey::Apostrophe, false) => Some("'"),
        (VirtualKey::Apostrophe, true) => Some("\""),
        (VirtualKey::Comma, false) => Some(","),
        (VirtualKey::Comma, true) => Some("<"),
        (VirtualKey::Dot, false) => Some("."),
        (VirtualKey::Dot, true) => Some(">"),
        (VirtualKey::Slash, false) => Some("/"),
        (VirtualKey::Slash, true) => Some("?"),
        (VirtualKey::Digit1, true) => Some("!"),
        (VirtualKey::Digit2, true) => Some("@"),
        (VirtualKey::Digit3, true) => Some("#"),
        (VirtualKey::Digit4, true) => Some("$"),
        (VirtualKey::Digit5, true) => Some("%"),
        (VirtualKey::Digit6, true) => Some("^"),
        (VirtualKey::Digit7, true) => Some("&"),
        (VirtualKey::Digit8, true) => Some("*"),
        (VirtualKey::Digit9, true) => Some("("),
        (VirtualKey::Digit0, true) => Some(")"),
        _ => None,
    }
}

pub fn strip_tones(s: &str) -> String {
    let mut res = String::new();
    for c in s.chars() {
        match c {
            'ā' | 'á' | 'ǎ' | 'à' => res.push('a'),
            'ē' | 'é' | 'ě' | 'è' => res.push('e'),
            'ī' | 'í' | 'ǐ' | 'ì' => res.push('i'),
            'ō' | 'ó' | 'ǒ' | 'ò' => res.push('o'),
            'ū' | 'ú' | 'ǔ' | 'ù' => res.push('u'),
            'ǖ' | 'ǘ' | 'ǚ' | 'ǜ' => res.push('v'),
            'Ā' | 'Á' | 'Ǎ' | 'À' => res.push('a'),
            'Ē' | 'É' | 'Ě' | 'È' => res.push('e'),
            'Ī' | 'Í' | 'Ǐ' | 'Ì' => res.push('i'),
            'Ō' | 'Ó' | 'Ǒ' | 'Ò' => res.push('o'),
            'Ū' | 'Ú' | 'Ǔ' | 'Ù' => res.push('u'),
            'Ǖ' | 'Ǘ' | 'Ǚ' | 'Ǜ' => res.push('v'),
            _ => res.push(c),
        }
    }
    res
}
