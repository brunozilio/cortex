//! Tokens for agents' visuals (SVG drawn in the conversation): the colour
//! ramps their `c-*` classes name, the type sizes of their label classes,
//! and the neutral tones each theme gives them.

use superfik_render::Color;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ramp {
    pub name: &'static str,
    pub stops: [Color; 7],
}

const fn ramp(name: &'static str, stops: [u32; 7]) -> Ramp {
    let mut colors = [Color::BLACK; 7];
    let mut i = 0;
    while i < 7 {
        colors[i] = Color::hex(stops[i]);
        i += 1;
    }
    Ramp { name, stops: colors }
}

pub mod stop {
    pub const S50: usize = 0;
    pub const S100: usize = 1;
    pub const S200: usize = 2;
    pub const S400: usize = 3;
    pub const S600: usize = 4;
    pub const S800: usize = 5;
}

pub const PURPLE: Ramp = ramp("purple", [0xEEEDFE, 0xCECBF6, 0xAFA9EC, 0x7F77DD, 0x534AB7, 0x3C3489, 0x26215C]);
pub const TEAL: Ramp = ramp("teal", [0xE1F5EE, 0x9FE1CB, 0x5DCAA5, 0x1D9E75, 0x0F6E56, 0x085041, 0x04342C]);
pub const CORAL: Ramp = ramp("coral", [0xFAECE7, 0xF5C4B3, 0xF0997B, 0xD85A30, 0x993C1D, 0x712B13, 0x4A1B0C]);
pub const PINK: Ramp = ramp("pink", [0xFBEAF0, 0xF4C0D1, 0xED93B1, 0xD4537E, 0x993556, 0x72243E, 0x4B1528]);
pub const GRAY: Ramp = ramp("gray", [0xF1EFE8, 0xD3D1C7, 0xB4B2A9, 0x888780, 0x5F5E5A, 0x444441, 0x2C2C2A]);
pub const BLUE: Ramp = ramp("blue", [0xE6F1FB, 0xB5D4F4, 0x85B7EB, 0x378ADD, 0x185FA5, 0x0C447C, 0x042C53]);
pub const GREEN: Ramp = ramp("green", [0xEAF3DE, 0xC0DD97, 0x97C459, 0x639922, 0x3B6D11, 0x27500A, 0x173404]);
pub const AMBER: Ramp = ramp("amber", [0xFAEEDA, 0xFAC775, 0xEF9F27, 0xBA7517, 0x854F0B, 0x633806, 0x412402]);
pub const RED: Ramp = ramp("red", [0xFCEBEB, 0xF7C1C1, 0xF09595, 0xE24B4A, 0xA32D2D, 0x791F1F, 0x501313]);

pub const RAMPS: [Ramp; 9] = [PURPLE, TEAL, CORAL, PINK, GRAY, BLUE, GREEN, AMBER, RED];

pub const LABEL: f32 = 14.0;
pub const LABEL_SMALL: f32 = 12.0;
pub const RADII: [f32; 3] = [8.0, 12.0, 16.0];
pub const FRAME_RADIUS: f32 = 10.0;
pub const PENDING_HEIGHT: f32 = 160.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VisualTones {
    pub dark: bool,
    pub text: Color,
    pub text_secondary: Color,
    pub text_tertiary: Color,
    pub surface: Color,
    pub page: Color,
    pub border: Color,
    pub border_strong: Color,
}
