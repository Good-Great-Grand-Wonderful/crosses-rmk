use rmk::types::action::KeyAction;
use rmk::{a, k};

pub fn get_default_keymap() -> [[[KeyAction; 12]; 5]; 1] {
    [[
        [k!(Grave), k!(Kc1), k!(Kc2), k!(Kc3), k!(Kc4), k!(Kc5), k!(Kc6), k!(Kc7), k!(Kc8), k!(Kc9), k!(Kc0), k!(Delete)],
        [k!(Tab), k!(Q), k!(W), k!(E), k!(R), k!(T), k!(Y), k!(U), k!(I), k!(O), k!(P), k!(Backspace)],
        [k!(LCtrl), k!(A), k!(S), k!(D), k!(F), k!(G), k!(H), k!(J), k!(K), k!(L), k!(Semicolon), k!(Quote)],
        [k!(LShift), k!(Z), k!(X), k!(C), k!(V), k!(B), k!(N), k!(M), k!(Comma), k!(Dot), k!(Slash), k!(Escape)],
        [a!(No), a!(No), a!(No), k!(LGui), k!(Tab), k!(Space), k!(Enter), k!(Tab), k!(RAlt), a!(No), a!(No), a!(No)],
    ]]
}
