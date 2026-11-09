//! Colour tokens, sampled from the reference design. Two palettes share one
//! shape: `DARK` and `LIGHT`. The painter carries the active one; a new
//! token is a field here plus a value in both palettes.

use superfik_render::{Color, PatternTones};

use super::visual::VisualTones;

pub const ROLE_TILE_GRAIN: f32 = 0.05;
pub const KNOB_SCRIM_OPACITY: f32 = 0.55;
pub const DISABLED_OPACITY: f32 = 0.42;
pub const ULTRA_SATURATION: f32 = 0.45;
pub const ULTRA_HUE: f32 = 0.71;
pub const ULTRA_SWEEP: f32 = 0.4;

pub const ICON_LOOK: crate::emote::Look = crate::emote::Look {
    ring: [
        Color::hex(0xa6f91f),
        Color::hex(0x1ff9df),
        Color::hex(0x1f90f9),
        Color::hex(0x1fd2f9),
        Color::hex(0x1ff976),
        Color::hex(0xf4f91f),
        Color::hex(0xf9871f),
        Color::hex(0xf9391f),
        Color::hex(0xf92c1f),
        Color::hex(0xf92c1f),
        Color::hex(0xf9461f),
        Color::hex(0xf9871f),
    ],
    eye: Color::hex(0xffffff),
    alert: Color::hex(0xf5b43a),
    done: Color::hex(0x5fd37a),
    badge: Color::hex(0x1c1c1e),
    tile_top: Color::hex(0x3a3a3f),
    tile_bottom: Color::hex(0x151517),
    tile_rim: Color::hex(0x5a5a60),
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContextTones {
    pub system: Color,
    pub tools: Color,
    pub mcp: Color,
    pub agents: Color,
    pub memory: Color,
    pub skills: Color,
    pub messages: Color,
    pub other: Color,
    pub free: Color,
    pub reserved: Color,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MemoryTones {
    pub episodic: Color,
    pub semantic: Color,
    pub procedural: Color,
    pub line: Color,
    pub line_lit: Color,
    pub unlit: Color,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    pub canvas: Color,
    pub stage: Color,
    pub surface_top: Color,
    pub surface_bottom: Color,
    pub connector: Color,
    pub well: Color,
    pub control: Color,
    pub control_active: Color,
    pub hover: Color,
    pub form_accent: Color,
    pub form_mark: Color,
    pub form_box: Color,
    pub form_box_edge: Color,
    pub bubble: Color,
    pub badge: Color,
    pub role_tile_top: Color,
    pub role_tile_bottom: Color,

    pub text: Color,
    pub text_hover: Color,
    pub text_body: Color,
    pub text_secondary: Color,
    pub text_subtle: Color,
    pub text_label: Color,
    pub status_label: Color,
    pub status_detail: Color,
    pub text_tertiary: Color,
    pub text_code: Color,
    pub placeholder: Color,
    pub icon: Color,
    pub icon_muted: Color,

    pub send: Color,
    pub send_disabled: Color,
    pub send_glyph: Color,
    pub knob: Color,
    pub knob_glyph: Color,
    pub knob_scrim: Color,
    pub orb_ink_near: Color,
    pub orb_ink_far: Color,
    pub emote_ring: [Color; 12],
    pub emote_eye: Color,
    pub emote_alert: Color,
    pub emote_done: Color,
    pub success: Color,

    pub repository: Color,
    pub sparkle: Color,
    pub skill: Color,
    pub plugin: Color,
    pub divider: Color,
    pub reference: Color,
    pub research: Color,
    pub research_tile: Color,
    pub agent: Color,
    pub agent_progress: Color,
    pub branch: Color,
    pub diff_added: Color,
    pub diff_removed: Color,
    pub diff_selected: Color,
    pub diff_hunk: Color,
    pub model: Color,
    pub ultra_lightness: f32,
    pub skill_cube: Color,
    pub skill_bolt: Color,
    pub danger: Color,
    pub step_rail: Color,
    pub particles: [Color; 5],

    pub surface_highlight: Color,
    pub surface_shadow: Color,
    pub well_rim: Color,
    pub scrim: Color,
    pub viewer_shadow: Color,
    pub tooltip: Color,
    pub tooltip_shadow: Color,

    pub timeline_rail: Color,
    pub timeline_dot: Color,
    pub timeline_accent: Color,
    pub timeline_hover: Color,
    pub attention: Color,
    pub qr_ink: Color,
    pub qr_paper: Color,

    pub context: Color,
    pub context_tones: ContextTones,

    pub memory: Color,
    pub memory_tones: MemoryTones,

    pub notes: Color,
    pub paper: PaperTones,

    pub patterns: PatternTones,

    pub visual: VisualTones,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaperTones {
    pub sheets: [Color; 5],
    pub bands: [Color; 5],
    pub ink: Color,
    pub ink_done: Color,
}

pub const DARK: Palette = Palette {
    canvas: Color::hex(0x232323),
    stage: Color::hex(0x0d0d0e),
    surface_top: Color::hex(0x3b3b3b),
    surface_bottom: Color::hex(0x373737),
    connector: Color::hex(0x363636),
    well: Color::hex(0x303030),
    control: Color::hex(0x3d3d3d),
    control_active: Color::hex(0x4d4d4d),
    hover: Color::hex(0x484848),
    form_accent: Color::hex(0x0a84ff),
    form_mark: Color::WHITE,
    form_box: Color::hex(0x3d3d3d),
    form_box_edge: Color::hex(0x575757),
    bubble: Color::hex(0x383838),
    badge: Color::hex(0x3c3c3c),
    role_tile_top: Color::hex(0x585858),
    role_tile_bottom: Color::hex(0x484848),

    text: Color::hex(0xe0e0e0),
    text_hover: Color::hex(0xf4f4f4),
    text_body: Color::hex(0xd9d9d9),
    text_secondary: Color::hex(0x9d9d9d),
    text_subtle: Color::hex(0x929292),
    text_label: Color::hex(0xd0d0d0),
    status_label: Color::hex(0x7e7e7e),
    status_detail: Color::hex(0x5e5e5e),
    text_tertiary: Color::hex(0x686868),
    text_code: Color::hex(0xe9b38f),
    placeholder: Color::hex(0x6b6b6b),
    icon: Color::hex(0xd0d0d0),
    icon_muted: Color::hex(0x8a8a8a),

    send: Color::hex(0xe1e1e1),
    send_disabled: Color::hex(0x444444),
    send_glyph: Color::hex(0x1e1e1e),
    knob: Color::hex(0xbebebe),
    knob_glyph: Color::hex(0x2d2d2d),
    knob_scrim: Color::hex(0x1a1a1a),
    orb_ink_near: Color::hex(0xf2f2f2),
    orb_ink_far: Color::hex(0x4a4a4a),
    emote_ring: [
        Color::hex(0xc593fc),
        Color::hex(0x86c1fe),
        Color::hex(0xa6d6fb),
        Color::hex(0x8be1d9),
        Color::hex(0x77c3b9),
        Color::hex(0x7cf485),
        Color::hex(0xefcb51),
        Color::hex(0xf0b25a),
        Color::hex(0xe38a52),
        Color::hex(0xd2634f),
        Color::hex(0xfa8991),
        Color::hex(0xfc98d6),
    ],
    emote_eye: Color::hex(0xffffff),
    emote_alert: Color::hex(0xf5b43a),
    emote_done: Color::hex(0x5fd37a),
    success: Color::hex(0x5fd37a),

    repository: Color::hex(0xfa8991),
    sparkle: Color::hex(0xefcb51),
    skill: Color::hex(0xc593fc),
    plugin: Color::hex(0x8be1d9),
    divider: Color::hex(0x3a3a3d),
    reference: Color::hex(0xfc98d6),
    research: Color::hex(0xa6d6fb),
    research_tile: Color::hex(0x404547),
    agent: Color::hex(0x8be1d9),
    agent_progress: Color::hex(0x77c3b9),
    branch: Color::hex(0x7cf485),
    diff_added: Color::hex(0x7cf485).with_alpha(0.13),
    diff_removed: Color::hex(0xfa8991).with_alpha(0.15),
    diff_selected: Color::hex(0xefcb51).with_alpha(0.22),
    diff_hunk: Color::hex(0xa6d6fb).with_alpha(0.10),
    model: Color::hex(0xd2634f),
    ultra_lightness: 0.78,
    skill_cube: Color::hex(0xee8dff),
    skill_bolt: Color::hex(0x86c1fe),
    danger: Color::hex(0xe5776f),
    step_rail: Color::hex(0x3a3a3a),
    particles: [
        Color::hex(0x9be37a),
        Color::hex(0xe3d36f),
        Color::hex(0xb88cf2),
        Color::hex(0x7fc6f2),
        Color::hex(0xe9e9e9),
    ],

    surface_highlight: Color::WHITE.with_alpha(0.05),
    surface_shadow: Color::BLACK.with_alpha(0.35),
    well_rim: Color::BLACK.with_alpha(0.22),
    scrim: Color::BLACK.with_alpha(0.72),
    viewer_shadow: Color::BLACK.with_alpha(0.5),
    tooltip: Color::hex(0x484848),
    tooltip_shadow: Color::BLACK.with_alpha(0.4),

    timeline_rail: Color::hex(0x3a3a3a),
    timeline_dot: Color::hex(0x6e6e6e),
    timeline_accent: Color::hex(0xff5fa8),
    timeline_hover: Color::WHITE.with_alpha(0.045),
    attention: Color::hex(0xf5b43a),
    qr_ink: Color::hex(0x111111),
    qr_paper: Color::hex(0xf7f7f5),

    context: Color::hex(0xa8adf7),
    context_tones: ContextTones {
        system: Color::hex(0x8a8a8a),
        tools: Color::hex(0xb4b4b4),
        mcp: Color::hex(0x8be1d9),
        agents: Color::hex(0x86c1fe),
        memory: Color::hex(0xf0a37a),
        skills: Color::hex(0xefcb51),
        messages: Color::hex(0xa8adf7),
        other: Color::hex(0xc593fc),
        free: Color::hex(0x3e3e3e),
        reserved: Color::hex(0x5a5a5a),
    },

    memory: Color::hex(0xf3c06b),
    memory_tones: MemoryTones {
        episodic: Color::hex(0x86c1fe),
        semantic: Color::hex(0x7cd992),
        procedural: Color::hex(0xf0a35a),
        line: Color::hex(0x3f4a5c),
        line_lit: Color::hex(0x6f8fc4),
        unlit: Color::hex(0x444444),
    },

    notes: Color::hex(0xf5d76e),
    paper: PaperTones {
        sheets: [
            Color::hex(0x3d3c39),
            Color::hex(0x4a4430),
            Color::hex(0x323d4d),
            Color::hex(0x4a3640),
            Color::hex(0x34443a),
        ],
        bands: [
            Color::hex(0x47463f),
            Color::hex(0x5b5235),
            Color::hex(0x3b4a60),
            Color::hex(0x5a3f4c),
            Color::hex(0x3e5245),
        ],
        ink: Color::hex(0xece6d6),
        ink_done: Color::hex(0x9b968a),
    },

    patterns: PatternTones {
        meter_lit: Color::hex(0xdbdbdb),
        meter_checker: [Color::hex(0x454545), Color::hex(0x5c5c5c)],
        ultra_off: Color::hex(0x2b2b2b),
        sparkle_ink: Color::hex(0x292929),
        shimmer: 0.035,
    },

    visual: VisualTones {
        dark: true,
        text: Color::hex(0xe4e4e4),
        text_secondary: Color::hex(0xa3a3a3),
        text_tertiary: Color::hex(0x737373),
        surface: Color::hex(0x2e2e2e),
        page: Color::hex(0x232323),
        border: Color::hex(0x474747),
        border_strong: Color::hex(0x5e5e5e),
    },
};

pub const LIGHT: Palette = Palette {
    canvas: Color::hex(0xececec),
    stage: Color::hex(0x0d0d0e),
    surface_top: Color::hex(0xfcfcfc),
    surface_bottom: Color::hex(0xf6f6f6),
    connector: Color::hex(0xdcdcdc),
    well: Color::hex(0xefefef),
    control: Color::hex(0xf0f0f0),
    control_active: Color::hex(0xdedede),
    hover: Color::hex(0xe4e4e4),
    form_accent: Color::hex(0x007aff),
    form_mark: Color::WHITE,
    form_box: Color::hex(0xe4e4e6),
    form_box_edge: Color::hex(0xd2d2d4),
    bubble: Color::hex(0xe2e2e2),
    badge: Color::hex(0xe6e6e6),
    role_tile_top: Color::hex(0xffffff),
    role_tile_bottom: Color::hex(0xe8e8e8),

    text: Color::hex(0x1e1e1e),
    text_hover: Color::hex(0x000000),
    text_body: Color::hex(0x2a2a2a),
    text_secondary: Color::hex(0x6a6a6a),
    text_subtle: Color::hex(0x727272),
    text_label: Color::hex(0x333333),
    status_label: Color::hex(0x808080),
    status_detail: Color::hex(0xa4a4a4),
    text_tertiary: Color::hex(0x9a9a9a),
    text_code: Color::hex(0xb3622c),
    placeholder: Color::hex(0xa0a0a0),
    icon: Color::hex(0x3a3a3a),
    icon_muted: Color::hex(0x8c8c8c),

    send: Color::hex(0x1e1e1e),
    send_disabled: Color::hex(0xdadada),
    send_glyph: Color::hex(0xf4f4f4),
    knob: Color::hex(0x5a5a5a),
    knob_glyph: Color::hex(0xf4f4f4),
    knob_scrim: Color::hex(0xffffff),
    orb_ink_near: Color::hex(0x222222),
    orb_ink_far: Color::hex(0xcfcfcf),
    emote_ring: [
        Color::hex(0x9a5ce6),
        Color::hex(0x3d8de0),
        Color::hex(0x3a8fd4),
        Color::hex(0x279c90),
        Color::hex(0x3fae9f),
        Color::hex(0x2fae3b),
        Color::hex(0xcf9f12),
        Color::hex(0xd98f1a),
        Color::hex(0xd6763a),
        Color::hex(0xd2634f),
        Color::hex(0xe0525e),
        Color::hex(0xdc4fa8),
    ],
    emote_eye: Color::hex(0x1e1e1e),
    emote_alert: Color::hex(0xd9941a),
    emote_done: Color::hex(0x3aa65a),
    success: Color::hex(0x2f9a4e),

    repository: Color::hex(0xe0525e),
    sparkle: Color::hex(0xcf9f12),
    skill: Color::hex(0x9a5ce6),
    plugin: Color::hex(0x279c90),
    divider: Color::hex(0xdedee3),
    reference: Color::hex(0xdc4fa8),
    research: Color::hex(0x3a8fd4),
    research_tile: Color::hex(0xdfe9f1),
    agent: Color::hex(0x279c90),
    agent_progress: Color::hex(0x3fae9f),
    branch: Color::hex(0x2fae3b),
    diff_added: Color::hex(0x2fae3b).with_alpha(0.14),
    diff_removed: Color::hex(0xd94a57).with_alpha(0.14),
    diff_selected: Color::hex(0xd9a800).with_alpha(0.24),
    diff_hunk: Color::hex(0x3b86c8).with_alpha(0.10),
    model: Color::hex(0xd2634f),
    ultra_lightness: 0.5,
    skill_cube: Color::hex(0xc24fdc),
    skill_bolt: Color::hex(0x3d8de0),
    danger: Color::hex(0xd6463b),
    step_rail: Color::hex(0xd8d8d8),
    particles: [
        Color::hex(0x5fb83a),
        Color::hex(0xc9a91e),
        Color::hex(0x9161d8),
        Color::hex(0x3a96d4),
        Color::hex(0x6a6a6a),
    ],

    surface_highlight: Color::WHITE.with_alpha(0.7),
    surface_shadow: Color::BLACK.with_alpha(0.1),
    well_rim: Color::BLACK.with_alpha(0.07),
    scrim: Color::hex(0xececec).with_alpha(0.8),
    viewer_shadow: Color::BLACK.with_alpha(0.18),
    tooltip: Color::hex(0xfcfcfc),
    tooltip_shadow: Color::BLACK.with_alpha(0.16),

    timeline_rail: Color::hex(0xd6d6d6),
    timeline_dot: Color::hex(0xb0b0b0),
    timeline_accent: Color::hex(0xf23d8f),
    timeline_hover: Color::BLACK.with_alpha(0.04),
    attention: Color::hex(0xd9941a),
    qr_ink: Color::hex(0x111111),
    qr_paper: Color::WHITE,

    context: Color::hex(0x5a63d8),
    context_tones: ContextTones {
        system: Color::hex(0x8c8c8c),
        tools: Color::hex(0x5e5e5e),
        mcp: Color::hex(0x279c90),
        agents: Color::hex(0x3d8de0),
        memory: Color::hex(0xd2734a),
        skills: Color::hex(0xcf9f12),
        messages: Color::hex(0x5a63d8),
        other: Color::hex(0x9a5ce6),
        free: Color::hex(0xdedede),
        reserved: Color::hex(0xb8b8b8),
    },

    memory: Color::hex(0xb7790e),
    memory_tones: MemoryTones {
        episodic: Color::hex(0x3d8de0),
        semantic: Color::hex(0x2f9e4f),
        procedural: Color::hex(0xd2762a),
        line: Color::hex(0xc9d0db),
        line_lit: Color::hex(0x6d8cc0),
        unlit: Color::hex(0xd8d8d8),
    },

    notes: Color::hex(0xc99a12),
    paper: PaperTones {
        sheets: [
            Color::hex(0xfbf8f1),
            Color::hex(0xfff2c4),
            Color::hex(0xdde8fb),
            Color::hex(0xfbe0ea),
            Color::hex(0xdff3e3),
        ],
        bands: [
            Color::hex(0xefeadf),
            Color::hex(0xfbe39a),
            Color::hex(0xc0d3f5),
            Color::hex(0xf4c4d6),
            Color::hex(0xc3e6ca),
        ],
        ink: Color::hex(0x2b2a26),
        ink_done: Color::hex(0x8a877f),
    },

    patterns: PatternTones {
        meter_lit: Color::hex(0x3a3a3a),
        meter_checker: [Color::hex(0xd2d2d2), Color::hex(0xbdbdbd)],
        ultra_off: Color::hex(0xe4e4e4),
        sparkle_ink: Color::hex(0xc4c4c4),
        shimmer: -0.03,
    },

    visual: VisualTones {
        dark: false,
        text: Color::hex(0x2a2a2a),
        text_secondary: Color::hex(0x6a6a6a),
        text_tertiary: Color::hex(0x9a9a9a),
        surface: Color::hex(0xf7f7f7),
        page: Color::hex(0xececec),
        border: Color::hex(0xcfcfcf),
        border_strong: Color::hex(0xb3b3b3),
    },
};

#[cfg(test)]
mod tests {
    use super::*;

    fn luminance(c: Color) -> f32 {
        let lin = |v: f32| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
        0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b)
    }

    fn contrast(a: Color, b: Color) -> f32 {
        let (la, lb) = (luminance(a), luminance(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    #[test]
    fn body_text_reads_on_canvas_and_surfaces_in_both_palettes() {
        for palette in [DARK, LIGHT] {
            for text in [palette.text, palette.text_body, palette.text_label] {
                for ground in [palette.canvas, palette.surface_top, palette.well, palette.bubble, palette.tooltip] {
                    assert!(contrast(text, ground) >= 4.5, "{text:?} on {ground:?}");
                }
            }
        }
    }

    #[test]
    fn ink_reads_on_every_paper_in_both_palettes() {
        for palette in [DARK, LIGHT] {
            for sheet in palette.paper.sheets {
                assert!(contrast(palette.paper.ink, sheet) >= 7.0, "{sheet:?}");
                assert!(contrast(palette.paper.ink_done, sheet) >= 2.5, "{sheet:?}");
            }
        }
    }

    #[test]
    fn dark_keeps_the_reference_design() {
        assert_eq!(DARK.canvas, Color::hex(0x232323));
        assert_eq!(DARK.surface_top, Color::hex(0x3b3b3b));
        assert_eq!(DARK.text, Color::hex(0xe0e0e0));
        assert_eq!(DARK.timeline_accent, Color::hex(0xff5fa8));
    }
}
