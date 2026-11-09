//! The cursor a page asks for.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PageCursor {
    Default,
    Pointer,
    Text,
    Wait,
    Progress,
    Crosshair,
    Move,
    NotAllowed,
    Help,
    ResizeNs,
    ResizeEw,
    ResizeNeSw,
    ResizeNwSe,
    Grab,
    Grabbing,
    ZoomIn,
    ZoomOut,
}
