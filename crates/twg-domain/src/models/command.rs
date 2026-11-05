use serde::{Deserialize, Serialize};

use super::{Id, Timestamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandType {
    #[serde(rename = "RELOAD")]
    Reload,
    #[serde(rename = "ZOOM")]
    Zoom,
    #[serde(rename = "CHANGE")]
    Change,
    #[serde(rename = "SNAPSHOT_RESTORE")]
    SnapshotRestore,
    #[serde(rename = "MOVE")]
    Move,
    #[serde(rename = "ROT")]
    Rotate,
    #[serde(rename = "MOVEGROUP")]
    MoveGroup,
    #[serde(rename = "ROTGROUP")]
    RotateGroup,
    #[serde(rename = "SHOWINFO")]
    ShowInfo,
    #[serde(rename = "HIDEINFO")]
    HideInfo,
    #[serde(rename = "SHOWINFO_BUILDING")]
    ShowInfoBuilding,
    #[serde(rename = "HIDEINFO_BUILDING")]
    HideInfoBuilding,
    #[serde(rename = "PLAY_ANIM")]
    PlayAnimation,
    #[serde(rename = "STOP_ANIM")]
    StopAnimation,
    #[serde(rename = "SHOOT")]
    Shoot,
    #[serde(rename = "SHOW_TEXT_MARKER")]
    ShowTextMarker,
    #[serde(rename = "HIDE_TEXT_MARKER")]
    HideTextMarker,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Command {
    pub id: Id,
    pub operation_id: Id,
    pub scenario_id: Id,
    pub marker_id: Id,
    pub command: CommandType,
    pub data: String,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
