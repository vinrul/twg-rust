use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishCommandRequest {
    #[serde(default)]
    pub operasi_id: String,
    #[serde(default)]
    pub skenario_id: String,
    #[serde(default)]
    pub marker_id: String,
    pub command: String,
    #[serde(default)]
    pub data: String,
    #[serde(default)]
    pub is_save: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandPayload {
    pub operasi_id: String,
    pub skenario_id: String,
    pub marker_id: String,
    pub command: String,
    pub data: String,
    pub is_save: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub satker_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandName {
    Reload,
    Zoom,
    Change,
    SnapshotRestore,
    Move,
    Rot,
    MoveGroup,
    RotGroup,
    ShowInfo,
    HideInfo,
    ShowInfoBuilding,
    HideInfoBuilding,
    PlayAnim,
    StopAnim,
    Shoot,
    ShowTextMarker,
    HideTextMarker,
}

impl CommandName {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Reload => "RELOAD",
            Self::Zoom => "ZOOM",
            Self::Change => "CHANGE",
            Self::SnapshotRestore => "SNAPSHOT_RESTORE",
            Self::Move => "MOVE",
            Self::Rot => "ROT",
            Self::MoveGroup => "MOVEGROUP",
            Self::RotGroup => "ROTGROUP",
            Self::ShowInfo => "SHOWINFO",
            Self::HideInfo => "HIDEINFO",
            Self::ShowInfoBuilding => "SHOWINFO_BUILDING",
            Self::HideInfoBuilding => "HIDEINFO_BUILDING",
            Self::PlayAnim => "PLAY_ANIM",
            Self::StopAnim => "STOP_ANIM",
            Self::Shoot => "SHOOT",
            Self::ShowTextMarker => "SHOW_TEXT_MARKER",
            Self::HideTextMarker => "HIDE_TEXT_MARKER",
        }
    }
}

impl FromStr for CommandName {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let value = value.trim().to_ascii_uppercase();
        match value.as_str() {
            "RELOAD" => Ok(Self::Reload),
            "ZOOM" => Ok(Self::Zoom),
            "CHANGE" => Ok(Self::Change),
            "SNAPSHOT_RESTORE" => Ok(Self::SnapshotRestore),
            "MOVE" => Ok(Self::Move),
            "ROT" => Ok(Self::Rot),
            "MOVEGROUP" => Ok(Self::MoveGroup),
            "ROTGROUP" => Ok(Self::RotGroup),
            "SHOWINFO" => Ok(Self::ShowInfo),
            "HIDEINFO" => Ok(Self::HideInfo),
            "SHOWINFO_BUILDING" => Ok(Self::ShowInfoBuilding),
            "HIDEINFO_BUILDING" => Ok(Self::HideInfoBuilding),
            "PLAY_ANIM" => Ok(Self::PlayAnim),
            "STOP_ANIM" => Ok(Self::StopAnim),
            "SHOOT" => Ok(Self::Shoot),
            "SHOW_TEXT_MARKER" => Ok(Self::ShowTextMarker),
            "HIDE_TEXT_MARKER" => Ok(Self::HideTextMarker),
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CommandName;
    use std::str::FromStr;

    #[test]
    fn parses_all_known_commands() {
        for name in [
            "RELOAD",
            "ZOOM",
            "CHANGE",
            "SNAPSHOT_RESTORE",
            "MOVE",
            "ROT",
            "MOVEGROUP",
            "ROTGROUP",
            "SHOWINFO",
            "HIDEINFO",
            "SHOWINFO_BUILDING",
            "HIDEINFO_BUILDING",
            "PLAY_ANIM",
            "STOP_ANIM",
            "SHOOT",
            "SHOW_TEXT_MARKER",
            "HIDE_TEXT_MARKER",
        ] {
            assert!(CommandName::from_str(name).is_ok(), "{name}");
        }
    }

    #[test]
    fn unknown_command_is_rejected() {
        assert!(CommandName::from_str("UNKNOWN").is_err());
    }
}
