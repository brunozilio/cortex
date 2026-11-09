//! What Linux calls a key. CEF takes the DOM `code` from the XKB keycode
//! (the evdev code plus 8) and the key from the Windows code; the editing
//! keys carry the control character GTK gives them (Backspace is BS, not
//! the Mac's DEL), and the arrows none.

pub fn native_code(windows: i32) -> i32 {
    match windows {
        0x08 => 22,
        0x09 => 23,
        0x0D => 36,
        0x10 => 50,
        0x11 => 37,
        0x12 => 64,
        0x1B => 9,
        0x20 => 65,
        0x21 => 112,
        0x22 => 117,
        0x23 => 115,
        0x24 => 110,
        0x25 => 113,
        0x26 => 111,
        0x27 => 114,
        0x28 => 116,
        0x2E => 119,
        0x30..=0x39 => DIGITS[(windows - 0x30) as usize],
        0x41..=0x5A => LETTERS[(windows - 0x41) as usize],
        0x5B => 133,
        0x5C => 134,
        0x70..=0x7B => FUNCTIONS[(windows - 0x70) as usize],
        0xBA => 47,
        0xBB => 21,
        0xBC => 59,
        0xBD => 20,
        0xBE => 60,
        0xBF => 61,
        0xC0 => 49,
        0xDB => 34,
        0xDC => 51,
        0xDD => 35,
        0xDE => 48,
        _ => 0,
    }
}

pub fn function_char(windows: i32) -> Option<char> {
    match windows {
        0x08 | 0x09 | 0x0D | 0x1B => char::from_u32(windows as u32),
        0x2E => Some('\u{7F}'),
        _ => None,
    }
}

const LETTERS: [i32; 26] =
    [38, 56, 54, 40, 26, 41, 42, 43, 31, 44, 45, 46, 58, 57, 32, 33, 24, 27, 39, 28, 30, 55, 25, 53, 29, 52];
const DIGITS: [i32; 10] = [19, 10, 11, 12, 13, 14, 15, 16, 17, 18];
const FUNCTIONS: [i32; 12] = [67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 95, 96];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editing_keys_carry_xkb_codes_and_control_characters() {
        assert_eq!((native_code(0x08), function_char(0x08)), (22, Some('\u{8}')), "Backspace");
        assert_eq!((native_code(0x2E), function_char(0x2E)), (119, Some('\u{7F}')), "Delete");
        assert_eq!((native_code(0x0D), function_char(0x0D)), (36, Some('\r')), "Return");
        assert_eq!((native_code(0x25), function_char(0x25)), (113, None), "Left");
        assert_eq!(native_code(0x41), 38, "A");
        assert_eq!(native_code(0x5A), 52, "Z");
        assert_eq!(native_code(0x30), 19, "0");
        assert_eq!(native_code(0x31), 10, "1");
        assert_eq!(native_code(0x7B), 96, "F12");
        assert_eq!(function_char(0x41), None, "letters type their own text");
    }
}
