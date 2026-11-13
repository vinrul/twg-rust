use bevy::prelude::*;

/// Loading progress of the assets that are visible in the map viewport.
///
/// `markers` covers marker model meshes/scenes, `animations` covers the walk
/// clips of animated play units, and `tiles` covers the raster tiles currently
/// in view. Failed assets count as finished so a broken download cannot leave
/// the loading bar on screen forever.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RenderLoadStatus {
    pub markers_loaded: u32,
    pub markers_total: u32,
    pub animations_loaded: u32,
    pub animations_total: u32,
    pub tiles_loaded: u32,
    pub tiles_total: u32,
}

impl RenderLoadStatus {
    pub fn is_loading(&self) -> bool {
        self.markers_loaded < self.markers_total
            || self.animations_loaded < self.animations_total
            || self.tiles_loaded < self.tiles_total
    }

    pub fn fraction(&self) -> f32 {
        let loaded = self.markers_loaded + self.animations_loaded + self.tiles_loaded;
        let total = self.markers_total + self.animations_total + self.tiles_total;
        if total == 0 {
            1.0
        } else {
            loaded.min(total) as f32 / total as f32
        }
    }

    pub fn detail(&self) -> String {
        let mut parts = Vec::new();
        if self.markers_total > 0 {
            parts.push(format!(
                "Marker {}/{}",
                self.markers_loaded, self.markers_total
            ));
        }
        if self.animations_total > 0 {
            parts.push(format!(
                "Animasi {}/{}",
                self.animations_loaded, self.animations_total
            ));
        }
        if self.tiles_total > 0 {
            parts.push(format!("Peta {}/{}", self.tiles_loaded, self.tiles_total));
        }
        parts.join(" · ")
    }
}

#[cfg(test)]
mod tests {
    use super::RenderLoadStatus;

    #[test]
    fn progress_is_aggregated_across_asset_kinds() {
        let status = RenderLoadStatus {
            markers_loaded: 1,
            markers_total: 2,
            animations_loaded: 0,
            animations_total: 1,
            tiles_loaded: 3,
            tiles_total: 5,
        };
        assert!(status.is_loading());
        assert!((status.fraction() - 0.5).abs() < 1.0e-6);
        assert!(status.detail().contains("Marker 1/2"));
    }

    #[test]
    fn empty_or_finished_status_is_not_loading() {
        let empty = RenderLoadStatus::default();
        assert!(!empty.is_loading());
        assert_eq!(empty.fraction(), 1.0);

        let finished = RenderLoadStatus {
            markers_loaded: 2,
            markers_total: 2,
            tiles_loaded: 4,
            tiles_total: 4,
            ..Default::default()
        };
        assert!(!finished.is_loading());
        assert_eq!(finished.fraction(), 1.0);
    }
}
