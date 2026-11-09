//! Type scale. Inter for the interface, the monospace family for code,
//! a handful of sizes fitted to the reference video (ink widths measured
//! against our renderer).

use superfik_render::{FontWeight, TextStyle};

pub const CODE: TextStyle = TextStyle::new(12.5, 20.0, FontWeight::Regular).mono();
pub const CODE_SMALL: TextStyle = TextStyle::new(11.0, 20.0, FontWeight::Regular).mono();

pub const BODY: TextStyle = TextStyle::new(15.0, 23.5, FontWeight::Regular).tracked(-0.016);
pub const HEADING: TextStyle = TextStyle::new(16.5, 24.0, FontWeight::SemiBold).tracked(-0.016);
pub const TABLE: TextStyle = TextStyle::new(13.5, 20.0, FontWeight::Regular).tracked(-0.012);
pub const TABLE_HEAD: TextStyle = TextStyle::new(13.5, 20.0, FontWeight::SemiBold).tracked(-0.012);
pub const INPUT: TextStyle = TextStyle::new(15.0, 22.0, FontWeight::Regular).tracked(-0.016);
pub const BUBBLE: TextStyle = TextStyle::new(15.0, 21.0, FontWeight::Regular).tracked(-0.016);
pub const TITLE: TextStyle = TextStyle::new(12.6, 18.0, FontWeight::Medium);
pub const SKILL: TextStyle = TextStyle::new(12.0, 18.0, FontWeight::Medium);
pub const MODEL: TextStyle = TextStyle::new(13.0, 18.0, FontWeight::Medium);
pub const LABEL: TextStyle = TextStyle::new(11.6, 16.0, FontWeight::Regular);
pub const CHIP: TextStyle = TextStyle::new(12.75, 18.0, FontWeight::Regular);
pub const METER: TextStyle = TextStyle::new(11.5, 16.0, FontWeight::Regular);
pub const MENU: TextStyle = TextStyle::new(13.0, 18.0, FontWeight::Regular);
pub const HEADER: TextStyle = TextStyle::new(12.0, 16.0, FontWeight::Regular);
pub const SUBTITLE: TextStyle = TextStyle::new(11.5, 16.0, FontWeight::Regular);
pub const TOOLTIP: TextStyle = TextStyle::new(12.0, 16.0, FontWeight::Regular);
pub const CAPTION: TextStyle = TextStyle::new(11.0, 14.0, FontWeight::Regular);
pub const BRANCH_CAPTION: TextStyle = TextStyle::new(10.75, 14.0, FontWeight::Regular);
pub const STATUS: TextStyle = TextStyle::new(12.75, 18.0, FontWeight::Regular);
pub const TIMELINE: TextStyle = TextStyle::new(13.5, 18.0, FontWeight::Regular).tracked(-0.01);
pub const EMOTE_MARK: TextStyle = TextStyle::new(11.0, 14.0, FontWeight::Medium);

pub const STORE_DISPLAY: TextStyle = TextStyle::new(26.0, 32.0, FontWeight::SemiBold).tracked(-0.02);
pub const STORE_SUBTITLE: TextStyle = TextStyle::new(12.0, 16.0, FontWeight::Regular);
pub const STORE_BODY: TextStyle = TextStyle::new(14.0, 21.0, FontWeight::Regular);

pub const NOTE_TITLE: TextStyle = TextStyle::new(13.0, 19.0, FontWeight::SemiBold).tracked(-0.01);
pub const NOTE_LINE: TextStyle = TextStyle::new(12.5, 18.0, FontWeight::Regular);

pub const GET_STARTED_WELCOME: TextStyle = TextStyle::new(40.0, 48.0, FontWeight::SemiBold).tracked(-0.03);
pub const GET_STARTED_TITLE: TextStyle = TextStyle::new(32.0, 38.0, FontWeight::SemiBold).tracked(-0.025);
pub const GET_STARTED_FEATURE: TextStyle = TextStyle::new(14.5, 20.0, FontWeight::Medium).tracked(-0.01);
pub const GET_STARTED_BODY: TextStyle = TextStyle::new(13.5, 20.0, FontWeight::Regular);

pub const UPDATE_TITLE: TextStyle = TextStyle::new(17.0, 24.0, FontWeight::Medium).tracked(-0.012);
pub const UPDATE_CAPTION: TextStyle = TextStyle::new(12.75, 18.0, FontWeight::Regular);
