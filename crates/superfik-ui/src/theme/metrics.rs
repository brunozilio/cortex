//! Spacing, radii and component dimensions (logical pixels).

pub const NODE_WIDTH: f32 = 216.0;
pub const NODE_RADIUS: f32 = 10.0;
pub const NODE_PADDING: f32 = 10.0;
pub const NODE_HEADER: f32 = 40.0;
pub const NODE_GAP: f32 = 14.0;
pub const NODE_CLEARANCE: f32 = 10.0;

pub const WELL_RADIUS: f32 = 8.0;
pub const ROW_HEIGHT: f32 = 30.0;
pub const ROW_GAP: f32 = 7.0;
pub const ROW_RADIUS: f32 = 7.0;
pub const CODE_BLOCK_PAD: f32 = 10.0;
pub const CODE_BLOCK_FILL: f32 = 0.06;
pub const TABLE_PAD_X: f32 = 12.0;
pub const TABLE_PAD_Y: f32 = 7.0;
pub const TABLE_MIN_COLUMN: f32 = 56.0;
pub const TABLE_RULE: f32 = 0.12;

pub const CHIP_HEIGHT: f32 = 32.0;
pub const RECENTRE_MARGIN: f32 = 24.0;
pub const CHIP_TINT: f32 = 0.16;
pub const CHIP_RADIUS: f32 = 7.0;
pub const FOCUS_RING: f32 = 3.0;
pub const SEND_SIZE: f32 = 33.0;

pub const ICON_SMALL: f32 = 12.0;
pub const HEADER_ICON: f32 = 16.0;
pub const HEADER_ICON_X: f32 = 23.0;
pub const HEADER_TITLE_X: f32 = 38.0;
pub const ROW_ICON_GAP: f32 = 6.0;
pub const BRANCH_ICON_GAP: f32 = 5.0;
pub const SKILL_ICON: f32 = 17.5;
pub const SKILL_BOLT: f32 = 16.0;
pub const SKILL_ICON_GAP: f32 = 14.0;
pub const PAGE_ROWS: usize = 5;
pub const PAGER_HEIGHT: f32 = 22.0;
pub const PAGE_FLIP: f32 = 60.0;
pub const ICON: f32 = 14.0;
pub const ICON_LARGE: f32 = 16.0;

pub const ROLE_TILE_RADIUS: f32 = 5.0;
pub const ROLE_ICON: f32 = 18.0;

pub const TOOLTIP_MAX_WIDTH: f32 = 360.0;
pub const TOOLTIP_PADDING: (f32, f32) = (8.0, 5.0);
pub const TOOLTIP_RADIUS: f32 = 6.0;
pub const TOOLTIP_GAP: f32 = 16.0;
pub const TOOLTIP_MARGIN: f32 = 8.0;

pub const BADGE_HEIGHT: f32 = 16.0;
pub const BADGE_RADIUS: f32 = 4.5;
pub const BADGE_PADDING: f32 = 5.0;

pub const TILE_REMOVE_RADIUS: f32 = 9.0;
pub const TILE_REMOVE_SCRIM: f32 = 3.0;
pub const TILE_REMOVE_GLYPH: f32 = 12.0;
pub const TILE_REMOVE_INSET: f32 = 4.0;
pub const WELL_INSET: f32 = 11.0;
pub const COMPOSER_EMOTE: f32 = 24.0;
pub const ORB_TILE: (f32, f32) = (49.0, 27.0);
pub const BURST: f32 = 14.0;
pub const CONTROL_ICON: f32 = 18.0;
pub const CHIP_ICON: f32 = 15.0;
pub const SEND_GLYPH: f32 = 22.0;

pub const COMPOSER_WIDTH: f32 = 440.0;
pub const COMPOSER_HEIGHT: f32 = 190.0;
pub const INPUT_LINES: f32 = 4.0;
pub const INPUT_FULL_LINES: usize = 2;
pub const INPUT_INSET: f32 = 28.0;

pub const ASK_HEADER: f32 = 46.0;
pub const ASK_INSET: f32 = 11.0;
pub const ASK_WELL_PADDING: f32 = 13.0;
pub const ASK_SUBJECT_LINES: f32 = 5.0;
pub const ASK_OPTION: f32 = 42.0;
pub const ASK_OPTION_GAP: f32 = 4.0;
pub const ASK_KEY: f32 = 20.0;
pub const ASK_FOOTER_GAP: f32 = 9.0;
pub const ASK_BUTTON_GAP: f32 = 8.0;
pub const ASK_RISE: f32 = 10.0;

pub const CONNECTOR_RADIUS: f32 = 3.5;
pub const CONNECTOR_CORNER: f32 = 10.0;
pub const CONNECTOR_FILLET: f32 = 7.0;
pub const CONNECTOR_TIP: f32 = 6.0;
pub const GOO: f32 = 7.0;

pub const COLUMN_NEAR: f32 = 62.0;
pub const BESIDE_GAP: f32 = 38.0;
pub const COLUMN_FAR: f32 = 112.0;
pub const STACK_CENTER: f32 = 93.5;
pub const PORT_SPACING: f32 = 21.5;
pub const TRUNK_INSET: f32 = 30.0;
pub const TRUNK_STEP: f32 = 21.0;
pub const RESEARCH_GAP: f32 = 30.0;

pub const RESEARCH_TILE: f32 = 19.5;
pub const RESEARCH_TILE_RADIUS: f32 = 5.0;
pub const RESEARCH_GLYPH: f32 = 13.0;

pub const TRANSCRIPT_WIDTH: f32 = 436.0;
pub const AGENT_SHEET_WIDTH: f32 = TRANSCRIPT_WIDTH + NODE_PADDING * 2.0;
pub const AGENT_SHEET_HEIGHT: f32 = 480.0;
pub const STEP_ROW: f32 = 24.0;
pub const STEP_INSET: f32 = 10.0;
pub const STEP_RAIL: f32 = 1.5;
pub const STEP_OUTPUT_LINE: f32 = 15.0;
pub const STEP_OUTPUT_PAD: f32 = 6.0;
pub const STEP_OUTPUT_LINES: usize = 12;

pub const NOTICE_PADDING: f32 = 12.0;
pub const NOTICE_TOP: f32 = 18.0;

pub const SETTINGS_FOOTER: f32 = 48.0;
pub const SETTINGS_BUTTON: f32 = 28.0;
pub const SETTINGS_BUTTON_GAP: f32 = 6.0;

pub const SETTINGS_NAV_WIDTH: f32 = 200.0;
pub const SETTINGS_NAV_ROW: f32 = 30.0;
pub const SETTINGS_NAV_GAP: f32 = 2.0;
pub const SETTINGS_CONTENT_WIDTH: f32 = 600.0;
pub const SETTINGS_TOP: f32 = 8.0;
pub const SETTINGS_INTRO_GAP: f32 = 24.0;

pub const FORM_HEADING: f32 = 30.0;
pub const FORM_ROW: f32 = 48.0;
pub const FORM_ROW_DETAILED: f32 = 60.0;
pub const FORM_ROW_PADDING: f32 = 16.0;
pub const FORM_CARD_RADIUS: f32 = 12.0;
pub const FORM_GROUP_GAP: f32 = 24.0;
pub const FORM_FIELD_WIDTH: f32 = 260.0;
pub const FORM_FIELD_HEIGHT: f32 = 30.0;
pub const FORM_FIELD_LABEL: f32 = 22.0;
pub const FORM_FIELD_GAP: f32 = 12.0;
pub const FORM_SHEET_BUTTON_GAP: f32 = 8.0;
pub const QR_SIDE: f32 = 184.0;
pub const QR_RADIUS: f32 = 10.0;
pub const FORM_FIELD_MULTILINE: f32 = 148.0;
pub const FOOTER_DOT_RADIUS: f32 = 3.5;
pub const FOOTER_DOT_INSET: f32 = 5.0;
pub const FORM_COLUMN_GAP: f32 = 16.0;
pub const FORM_ICON: f32 = 28.0;
pub const FORM_ICON_GAP: f32 = 12.0;

pub const GLYPH_STROKE: f32 = 1.5 * ICON / 24.0;

pub const SWITCH_WIDTH: f32 = 32.0;
pub const SWITCH_HEIGHT: f32 = 19.0;
pub const SWITCH_INSET: f32 = 2.0;

pub const SIDEBAR_MIN_WIDTH: f32 = 160.0;
pub const SIDEBAR_MAX_WIDTH: f32 = 230.0;
pub const SIDEBAR_KEEP: f32 = 160.0;
pub const TIMELINE_TOP: f32 = 52.0;
pub const TITLEBAR_HEIGHT: f32 = 28.0;
pub const TIMELINE_ROW: f32 = 38.0;
pub const TIMELINE_ROW_HEIGHT: f32 = 32.0;
pub const TIMELINE_ROW_RADIUS: f32 = 8.0;
pub const TIMELINE_ROW_INSET: f32 = 8.0;
pub const TIMELINE_GROUP_X: f32 = 24.0;
pub const TIMELINE_ITEM_X: f32 = 40.0;
pub const TIMELINE_LABEL_GAP: f32 = 14.0;
pub const TIMELINE_TRAILING: f32 = 36.0;
pub const TIMELINE_BOTTOM: f32 = 16.0;
pub const TIMELINE_SHORTCUT_GAP: f32 = 8.0;
pub const TIMELINE_DOT_RADIUS: f32 = 3.25;
pub const TIMELINE_RAIL_RADIUS: f32 = 0.75;
pub const TIMELINE_BEND_GAP: f32 = 8.0;

pub const VIEWER_MARGIN: f32 = 48.0;
pub const VIEWER_RADIUS: f32 = 12.0;
pub const VIEWER_FILE: f32 = 220.0;
pub const VIEWER_CLOSE_INSET: f32 = 22.0;

pub const USAGE_COLUMNS: usize = 20;
pub const USAGE_ROWS: usize = 5;
pub const USAGE_DOT_GAP: f32 = 3.0;
pub const USAGE_WELL_PADDING: f32 = 10.0;
pub const LEGEND_ROW: f32 = 17.0;
pub const LEGEND_SWATCH: f32 = 6.0;

pub const NOTE_RADIUS: f32 = 9.0;
pub const NOTE_BAND: f32 = 7.0;
pub const NOTE_PADDING: f32 = 9.0;
pub const NOTE_GAP: f32 = 8.0;
pub const NOTE_CHECK: f32 = 12.0;
pub const NOTE_SWATCH: f32 = 14.0;
pub const NOTE_SWATCH_GAP: f32 = 6.0;
pub const NOTE_MAX_LINES: usize = 6;
pub const NOTE_MIN_EDIT_LINES: usize = 3;

pub const MEMORY_GRAPH_HEIGHT: f32 = 132.0;
pub const MEMORY_GRAPH_PADDING: f32 = 12.0;
pub const MEMORY_DOT: [f32; 3] = [3.0, 4.2, 3.6];
pub const MEMORY_LINE: f32 = 0.7;
pub const MEMORY_GRAPH_ASPECT: f32 = 1.35;
pub const MEMORY_DOT_MAX_SCALE: f32 = 2.0;
pub const MEMORY_DOT_GROW: f32 = 2.2;
pub const MEMORY_EXPLORER_MARGIN: f32 = 28.0;
pub const MEMORY_EXPLORER_GRAPH: f32 = 0.55;
pub const MEMORY_EXPLORER_GAP: f32 = 20.0;
pub const MEMORY_ITEM_PADDING: f32 = 10.0;
pub const MEMORY_ITEM_GAP: f32 = 6.0;
pub const MEMORY_SEARCH: f32 = 36.0;
pub const MEMORY_LEGEND_GAP: f32 = 18.0;
pub const MEMORY_ZOOM: (f32, f32) = (0.6, 4.0);

pub const STORE_TILE_RADIUS: f32 = 0.225;
pub const STORE_MARK: f32 = 0.54;
pub const STORE_MONOGRAM: f32 = 0.46;

pub const TOKEN_PAD: f32 = 5.0;
pub const TOKEN_RADIUS: f32 = 7.0;
pub const TOKEN_MARK: f32 = 14.0;
pub const TOKEN_MARK_GAP: f32 = 4.0;
pub const TOKEN_MARK_RISE: f32 = 0.364;
pub const TOKEN_SPACE: f32 = 4.0;
pub const TOKEN_FILL: f32 = 0.18;

pub const SCREEN_HEADER: f32 = 56.0;
pub const SCREEN_PADDING: f32 = 12.0;
pub const SCREEN_TITLE_GAP: f32 = 16.0;
pub const SCREEN_ACTION_GAP: f32 = 8.0;

pub const GET_STARTED_WIDTH: f32 = 520.0;
pub const SIGN_IN_WIDTH: f32 = 480.0;
pub const SIGN_IN_GAP: f32 = 12.0;
pub const UPDATE_WIDTH: f32 = 340.0;
pub const UPDATE_EMOTE: f32 = 72.0;
pub const UPDATE_BAR: f32 = 5.0;
pub const UPDATE_BAR_SEGMENT: f32 = 0.22;
pub const UPDATE_GAP: f32 = 10.0;
pub const GET_STARTED_EMOTE: f32 = 58.0;
pub const GET_STARTED_EMOTE_GAP: f32 = 26.0;
pub const GET_STARTED_TITLE_GAP: f32 = 40.0;
pub const GET_STARTED_TILE: f32 = 42.0;
pub const GET_STARTED_TILE_RADIUS: f32 = 10.0;
pub const GET_STARTED_GLYPH: f32 = 21.0;
pub const GET_STARTED_TEXT_GAP: f32 = 16.0;
pub const GET_STARTED_CARD_PADDING: f32 = 14.0;
pub const GET_STARTED_ROW_GAP: f32 = 12.0;
pub const GET_STARTED_CARD_PILL: (f32, f32) = (44.0, 24.0);
pub const GET_STARTED_CARD_SMOOTH: f32 = 2.0;
pub const GET_STARTED_ACTION_GAP: f32 = 40.0;
pub const GET_STARTED_BUTTON: (f32, f32) = (196.0, 34.0);
pub const GET_STARTED_RISE: f32 = 10.0;
pub const GET_STARTED_CAST_GLYPH: f32 = 30.0;
pub const GET_STARTED_CAST_EMOTE: f32 = 44.0;
pub const GET_STARTED_CAST_SPACING: f32 = 96.0;
pub const GET_STARTED_CAST_GAP: f32 = 64.0;
pub const GET_STARTED_CAST_FROM: f32 = 0.55;
pub const GET_STARTED_TRAIL_DOTS: usize = 5;
pub const GET_STARTED_TRAIL_DOT: f32 = 1.7;
pub const SCREEN_CONTENT_INSET: f32 = 32.0;

pub const ROW_SELECT_SHARE: f32 = 0.45;

pub const BROWSER_WIDTH: f32 = 680.0;
pub const BROWSER_PAGE_HEIGHT: f32 = 420.0;
pub const BROWSER_BUTTON: f32 = 28.0;
pub const BROWSER_BUTTON_GAP: f32 = 2.0;
pub const BROWSER_ADDRESS_GAP: f32 = 8.0;
pub const BROWSER_REVEAL_MARGIN: f32 = 24.0;
pub const PAGE_SWIPE_TRAVEL: f32 = 120.0;
pub const BROWSER_EXPANDED_INSET: f32 = 24.0;
pub const BROWSER_EXPANDED_TOP: f32 = 40.0;
pub const BROWSER_LOADING: (f32, f32) = (44.0, 24.0);

pub const STATUS_BUTTON_HEIGHT: f32 = 28.0;
pub const STATUS_BUTTON_PADDING: f32 = 12.0;
pub const STATUS_BUTTON_GAP: f32 = 6.0;
pub const STATUS_DONE_TINT: f32 = 0.14;
pub const STATUS_DONE_BORDER: f32 = 0.35;
