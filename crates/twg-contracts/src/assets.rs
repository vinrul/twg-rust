use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
use uuid::Uuid;

use crate::time::Timestamp;

/// Upload category. Every asset belongs to exactly one category, and the
/// category decides which file types are accepted and which conversions are
/// offered in the preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum AssetCategory {
    /// glTF/GLB model (marker, vehicle, building, ...).
    Model,
    /// Texture image applied to a model or the map.
    Texture,
    /// Icon image used by UI/markers.
    Icon,
}

impl AssetCategory {
    pub const ALL: [Self; 3] = [Self::Model, Self::Texture, Self::Icon];

    pub const fn is_model(self) -> bool {
        matches!(self, Self::Model)
    }

    pub const fn is_image(self) -> bool {
        matches!(self, Self::Texture | Self::Icon)
    }

    /// Accepted upload extensions (lowercase, without the dot).
    pub const fn upload_extensions(self) -> &'static [&'static str] {
        match self {
            Self::Model => &["glb", "gltf"],
            Self::Texture | Self::Icon => &["png", "jpg", "jpeg", "webp"],
        }
    }

    pub fn allows_extension(self, extension: &str) -> bool {
        self.upload_extensions()
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(extension))
    }
}

/// Model behaviour. Animated models upload two sources (`idle` and the GLB
/// that carries the animation); static models upload a single `static` source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum AssetModelKind {
    Static,
    Animated,
}

impl AssetModelKind {
    pub const ALL: [Self; 2] = [Self::Static, Self::Animated];

    /// Source slots required by this model kind, in upload order.
    pub const fn slots(self) -> &'static [AssetSourceSlot] {
        match self {
            Self::Static => &[AssetSourceSlot::Static],
            Self::Animated => &[AssetSourceSlot::Idle, AssetSourceSlot::Animated],
        }
    }

    /// Multipart field name used to upload the given slot.
    pub const fn upload_field(self, slot: AssetSourceSlot) -> &'static str {
        match (self, slot) {
            (_, AssetSourceSlot::Idle) => "idle",
            (_, AssetSourceSlot::Animated) => "animated",
            (_, AssetSourceSlot::Static) => "file",
        }
    }
}

/// How an asset may be moved during Play/simulation.
///
/// Only static models use `Limited` or `Free`; animated models are always
/// `Free` (walk clip, obstacle avoidance) and textures/icons are always `None`.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum AssetMoveMode {
    /// Not pickable and never moves during Play.
    #[default]
    None,
    /// Pickable during Play; moved with the 3D move gizmo (barriers, cones).
    Limited,
    /// Pickable during Play; ordered to move with a right click, avoiding
    /// obstacles like an animated unit but without its walk animation.
    Free,
}

impl AssetMoveMode {
    pub const ALL: [Self; 3] = [Self::None, Self::Limited, Self::Free];

    pub const fn is_movable(self) -> bool {
        !matches!(self, Self::None)
    }
}

/// One source file of an asset. Textures/icons only have `static`; models have
/// either `static` or both `idle` and `animated`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum AssetSourceSlot {
    Static,
    Idle,
    Animated,
}

impl AssetSourceSlot {
    pub const fn is_animated_source(self) -> bool {
        matches!(self, Self::Animated)
    }

    pub const fn is_idle_source(self) -> bool {
        matches!(self, Self::Idle)
    }

    /// True when this slot is valid for the given category/model kind.
    pub fn valid_for(self, category: AssetCategory, model_kind: Option<AssetModelKind>) -> bool {
        if category.is_image() {
            return self == Self::Static;
        }
        match model_kind {
            Some(kind) => kind.slots().contains(&self),
            None => false,
        }
    }
}

/// Converted artefact produced from one source file of an asset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum AssetVariantKind {
    /// Model compressed with KHR_draco_mesh_compression.
    Draco,
    /// Model simplified to the second level of detail.
    Lod2,
    /// LOD2 model that is also Draco compressed.
    DracoLod2,
    #[serde(rename = "resize_512")]
    #[strum(to_string = "resize_512")]
    Resize512,
    #[serde(rename = "resize_256")]
    #[strum(to_string = "resize_256")]
    Resize256,
    #[serde(rename = "resize_128")]
    #[strum(to_string = "resize_128")]
    Resize128,
}

impl AssetVariantKind {
    pub const ALL: [Self; 6] = [
        Self::Draco,
        Self::Lod2,
        Self::DracoLod2,
        Self::Resize512,
        Self::Resize256,
        Self::Resize128,
    ];

    pub const fn from_resize_side(side: u32) -> Option<Self> {
        match side {
            512 => Some(Self::Resize512),
            256 => Some(Self::Resize256),
            128 => Some(Self::Resize128),
            _ => None,
        }
    }

    pub const fn resize_side(self) -> Option<u32> {
        match self {
            Self::Resize512 => Some(512),
            Self::Resize256 => Some(256),
            Self::Resize128 => Some(128),
            Self::Draco | Self::Lod2 | Self::DracoLod2 => None,
        }
    }

    pub const fn is_draco(self) -> bool {
        matches!(self, Self::Draco | Self::DracoLod2)
    }

    pub const fn is_lod2(self) -> bool {
        matches!(self, Self::Lod2 | Self::DracoLod2)
    }

    /// File stem used on disk for the variant, e.g. `draco_lod2.glb`.
    pub const fn file_stem(self) -> &'static str {
        match self {
            Self::Draco => "draco",
            Self::Lod2 => "lod2",
            Self::DracoLod2 => "draco_lod2",
            Self::Resize512 => "resize_512",
            Self::Resize256 => "resize_256",
            Self::Resize128 => "resize_128",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetFile {
    pub filename: String,
    pub url: String,
    pub mime: String,
    pub size_bytes: i64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetVariant {
    pub kind: AssetVariantKind,
    pub filename: String,
    pub url: String,
    pub mime: String,
    pub size_bytes: i64,
    pub sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    /// True when the variant points at the source upload because the file
    /// already met the requested target (no re-encode happened).
    pub reused: bool,
}

/// One uploaded source file plus everything converted from it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetSource {
    pub slot: AssetSourceSlot,
    pub file: AssetFile,
    /// Detected on upload from the GLB `extensionsUsed` array.
    pub has_draco: bool,
    /// Detected LOD level of the upload (glTF metadata or filename marker).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lod: Option<u16>,
    pub variants: Vec<AssetVariant>,
}

impl AssetSource {
    pub fn variant(&self, kind: AssetVariantKind) -> Option<&AssetVariant> {
        self.variants.iter().find(|variant| variant.kind == kind)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetGroup {
    pub id: Uuid,
    pub name: String,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetGroupRequest {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Asset {
    pub id: Uuid,
    pub name: String,
    pub category: AssetCategory,
    /// Only set for models; `None` for textures and icons.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_kind: Option<AssetModelKind>,
    /// Movement allowed during Play. Animated models always report `Free`.
    #[serde(default)]
    pub move_mode: AssetMoveMode,
    /// Semantic asset category (Personel, Bangunan, Kendaraan, ...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_name: Option<String>,
    pub sources: Vec<AssetSource>,
    /// Bumped on every metadata or variant change so clients can re-sync.
    pub version: i64,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl Asset {
    pub fn source(&self, slot: AssetSourceSlot) -> Option<&AssetSource> {
        self.sources.iter().find(|source| source.slot == slot)
    }

    /// Only source of a texture/icon or a static model.
    pub fn primary_source(&self) -> Option<&AssetSource> {
        self.source(AssetSourceSlot::Static)
    }

    pub fn variant(&self, slot: AssetSourceSlot, kind: AssetVariantKind) -> Option<&AssetVariant> {
        self.source(slot).and_then(|source| source.variant(kind))
    }

    pub fn model_kind_or_static(&self) -> AssetModelKind {
        self.model_kind.unwrap_or(AssetModelKind::Static)
    }
}

/// Cheap sync payload: everything the desktop needs to decide which files to
/// download, without the file bytes themselves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetManifest {
    pub generated_at: Timestamp,
    pub assets: Vec<Asset>,
}

/// Progress of an offline cache sync. Shared between the download thread and
/// the UI (including the web build, which only renders the values).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncProgress {
    pub done: usize,
    pub total: usize,
    pub current: String,
    #[serde(default)]
    pub finished: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl SyncProgress {
    pub fn fraction(&self) -> f32 {
        if self.total == 0 {
            1.0
        } else {
            self.done as f32 / self.total as f32
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetUpdateRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub category: Option<AssetCategory>,
    #[serde(default)]
    pub group_id: Option<Uuid>,
    /// Only honored for static models.
    #[serde(default)]
    pub move_mode: Option<AssetMoveMode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConvertTarget {
    Draco,
    Lod2,
    DracoLod2,
    /// Shrink a texture/icon; requires `size` to be 512, 256, or 128.
    Resize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConvertRequest {
    pub target: ConvertTarget,
    /// Required for animated models (`idle` or `animated`); defaults to
    /// `static` for static models and images.
    #[serde(default)]
    pub slot: Option<AssetSourceSlot>,
    #[serde(default)]
    pub size: Option<u32>,
    /// Re-run the conversion even when the variant already exists.
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConvertResult {
    pub asset: Asset,
    /// Source slot the conversion applied to.
    pub slot: AssetSourceSlot,
    /// True when a conversion actually ran.
    pub converted: bool,
    /// Variant produced or reused by the request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<AssetVariantKind>,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::{
        AssetCategory, AssetModelKind, AssetMoveMode, AssetSourceSlot, AssetVariantKind,
        ConvertTarget,
    };
    use serde_json::json;
    use std::str::FromStr;

    #[test]
    fn categories_use_the_expected_wire_names() {
        for (category, wire) in [
            (AssetCategory::Model, "model"),
            (AssetCategory::Texture, "texture"),
            (AssetCategory::Icon, "icon"),
        ] {
            assert_eq!(category.to_string(), wire);
            assert_eq!(AssetCategory::from_str(wire).unwrap(), category);
            assert_eq!(serde_json::to_value(category).unwrap(), json!(wire));
        }
    }

    #[test]
    fn categories_gate_upload_extensions() {
        assert!(AssetCategory::Model.allows_extension("GLB"));
        assert!(AssetCategory::Model.allows_extension("gltf"));
        assert!(!AssetCategory::Model.allows_extension("png"));
        assert!(AssetCategory::Texture.allows_extension("png"));
        assert!(AssetCategory::Icon.allows_extension("JPEG"));
        assert!(!AssetCategory::Icon.allows_extension("glb"));
    }

    #[test]
    fn model_kinds_declare_their_sources() {
        assert_eq!(AssetModelKind::Static.slots(), &[AssetSourceSlot::Static]);
        assert_eq!(
            AssetModelKind::Animated.slots(),
            &[AssetSourceSlot::Idle, AssetSourceSlot::Animated]
        );
        assert_eq!(
            AssetModelKind::Animated.upload_field(AssetSourceSlot::Idle),
            "idle"
        );
        assert_eq!(
            AssetModelKind::Animated.upload_field(AssetSourceSlot::Animated),
            "animated"
        );
        assert_eq!(
            AssetModelKind::Static.upload_field(AssetSourceSlot::Static),
            "file"
        );
        assert!(
            AssetSourceSlot::Idle.valid_for(AssetCategory::Model, Some(AssetModelKind::Animated))
        );
        assert!(
            !AssetSourceSlot::Idle.valid_for(AssetCategory::Model, Some(AssetModelKind::Static))
        );
        assert!(AssetSourceSlot::Static.valid_for(AssetCategory::Icon, None));
        assert!(!AssetSourceSlot::Animated.valid_for(AssetCategory::Icon, None));
    }

    #[test]
    fn variant_kinds_round_trip_and_map_resize_sides() {
        for kind in AssetVariantKind::ALL {
            assert_eq!(AssetVariantKind::from_str(&kind.to_string()).unwrap(), kind);
        }
        assert_eq!(AssetVariantKind::Resize512.to_string(), "resize_512");
        assert_eq!(
            serde_json::to_value(AssetVariantKind::Resize128).unwrap(),
            json!("resize_128")
        );
        assert_eq!(
            AssetVariantKind::from_resize_side(512),
            Some(AssetVariantKind::Resize512)
        );
        assert_eq!(AssetVariantKind::from_resize_side(1024), None);
        assert_eq!(AssetVariantKind::Resize256.resize_side(), Some(256));
        assert!(AssetVariantKind::DracoLod2.is_draco());
        assert!(AssetVariantKind::DracoLod2.is_lod2());
        assert!(!AssetVariantKind::Lod2.is_draco());
    }

    #[test]
    fn move_modes_use_snake_case() {
        for (mode, wire) in [
            (AssetMoveMode::None, "none"),
            (AssetMoveMode::Limited, "limited"),
            (AssetMoveMode::Free, "free"),
        ] {
            assert_eq!(mode.to_string(), wire);
            assert_eq!(AssetMoveMode::from_str(wire).unwrap(), mode);
            assert_eq!(serde_json::to_value(mode).unwrap(), json!(wire));
        }
        assert!(!AssetMoveMode::None.is_movable());
        assert!(AssetMoveMode::Limited.is_movable());
        assert!(AssetMoveMode::Free.is_movable());
    }

    #[test]
    fn convert_targets_use_snake_case() {
        assert_eq!(
            serde_json::to_value(ConvertTarget::DracoLod2).unwrap(),
            json!("draco_lod2")
        );
        assert_eq!(
            serde_json::to_value(ConvertTarget::Resize).unwrap(),
            json!("resize")
        );
    }
}
