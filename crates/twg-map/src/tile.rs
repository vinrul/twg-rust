use std::collections::HashSet;

use crate::geo::{GeoError, GeoPoint, TileId};

pub const OSM_TILE_TEMPLATE: &str = "https://tile.openstreetmap.org/{z}/{x}/{y}.png";
pub const OSM_ATTRIBUTION: &str = "© OpenStreetMap contributors";
pub const ARCGIS_IMAGERY_TILE_TEMPLATE: &str =
    "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}";
pub const ARCGIS_IMAGERY_ATTRIBUTION: &str = "Tiles © Esri — Source: Esri, i-cubed, USDA, USGS, AEX, GeoEye, Getmapping, Aerogrid, IGN, IGP, UPR-EGP, and the GIS User Community";
pub const ARCGIS_IMAGERY_MAX_ZOOM: u8 = 19;
pub const OSM_MAX_ZOOM: u8 = 19;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum BaseLayer {
    #[default]
    OpenStreetMap,
    ArcGisImagery,
}

impl BaseLayer {
    pub const ALL: [Self; 2] = [Self::OpenStreetMap, Self::ArcGisImagery];

    pub const fn label(self) -> &'static str {
        match self {
            Self::OpenStreetMap => "OSM / Street",
            Self::ArcGisImagery => "Satellite / Esri",
        }
    }

    pub const fn attribution(self) -> &'static str {
        match self {
            Self::OpenStreetMap => OSM_ATTRIBUTION,
            Self::ArcGisImagery => ARCGIS_IMAGERY_ATTRIBUTION,
        }
    }

    pub const fn max_zoom(self) -> u8 {
        match self {
            Self::OpenStreetMap => OSM_MAX_ZOOM,
            Self::ArcGisImagery => ARCGIS_IMAGERY_MAX_ZOOM,
        }
    }

    pub const fn slug(self) -> &'static str {
        match self {
            Self::OpenStreetMap => "osm",
            Self::ArcGisImagery => "satellite",
        }
    }

    pub fn tile_source_with_proxy(self, proxy: Option<&str>) -> XyzTileSource {
        match proxy.map(str::trim).filter(|value| !value.is_empty()) {
            Some(base) => XyzTileSource::new(
                format!(
                    "{}/tiles/{}/{{z}}/{{x}}/{{y}}.png",
                    base.trim_end_matches('/'),
                    self.slug()
                ),
                self.attribution(),
                self.max_zoom(),
            ),
            None => self.tile_source(),
        }
    }

    pub fn tile_source(self) -> XyzTileSource {
        match self {
            Self::OpenStreetMap => XyzTileSource::osm(),
            Self::ArcGisImagery => XyzTileSource::arcgis_imagery(),
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::OpenStreetMap => Self::ArcGisImagery,
            Self::ArcGisImagery => Self::OpenStreetMap,
        }
    }
}

pub trait TileSource: Send + Sync {
    fn url(&self, tile: TileId) -> String;
    fn attribution(&self) -> &str;
    fn max_zoom(&self) -> u8;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XyzTileSource {
    template: String,
    attribution: String,
    max_zoom: u8,
}

impl XyzTileSource {
    pub fn new(template: impl Into<String>, attribution: impl Into<String>, max_zoom: u8) -> Self {
        Self {
            template: template.into(),
            attribution: attribution.into(),
            max_zoom,
        }
    }

    pub fn osm() -> Self {
        Self::new(OSM_TILE_TEMPLATE, OSM_ATTRIBUTION, OSM_MAX_ZOOM)
    }

    pub fn arcgis_imagery() -> Self {
        Self::new(
            ARCGIS_IMAGERY_TILE_TEMPLATE,
            ARCGIS_IMAGERY_ATTRIBUTION,
            ARCGIS_IMAGERY_MAX_ZOOM,
        )
    }
}

impl Default for XyzTileSource {
    fn default() -> Self {
        Self::osm()
    }
}

impl TileSource for XyzTileSource {
    fn url(&self, tile: TileId) -> String {
        self.template
            .replace("{z}", &tile.zoom.to_string())
            .replace("{x}", &tile.x.to_string())
            .replace("{y}", &tile.y.to_string())
    }

    fn attribution(&self) -> &str {
        &self.attribution
    }

    fn max_zoom(&self) -> u8 {
        self.max_zoom
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileCover {
    pub zoom: u8,
    pub center: TileId,
    pub radius: u32,
    pub tiles: Vec<TileId>,
}

impl TileCover {
    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }
}

pub fn cover_tiles(center: GeoPoint, zoom: u8, radius: u32) -> Result<TileCover, GeoError> {
    let center_tile = center.tile(zoom)?;
    let count = i64::from(TileId::count(zoom)?);
    let center_x = i64::from(center_tile.x);
    let center_y = i64::from(center_tile.y);
    let radius = i64::from(radius);

    let y_min = (center_y - radius).max(0);
    let y_max = (center_y + radius).min(count - 1);
    if y_max < y_min {
        return Ok(TileCover {
            zoom,
            center: center_tile,
            radius: radius as u32,
            tiles: Vec::new(),
        });
    }

    // Split the wrapped longitude range into at most two non-wrapping
    // intervals, then run a quadtree range traversal over each interval.
    let min_x = center_x - radius;
    let max_x = center_x + radius;
    let width = max_x - min_x + 1;
    let mut intervals = Vec::new();
    if width >= count {
        intervals.push((0, count - 1));
    } else {
        let start = min_x.rem_euclid(count);
        if start + width <= count {
            intervals.push((start, start + width - 1));
        } else {
            intervals.push((start, count - 1));
            intervals.push((0, start + width - count - 1));
        }
    }

    let mut found = HashSet::new();
    for (x_min, x_max) in intervals {
        quadtree_cover(
            zoom,
            TileRange {
                x_min,
                x_max,
                y_min,
                y_max,
            },
            &mut found,
        );
    }

    let mut tiles = found.into_iter().collect::<Vec<_>>();
    tiles.sort_unstable_by_key(|tile| (tile.y, tile.x));
    Ok(TileCover {
        zoom,
        center: center_tile,
        radius: radius as u32,
        tiles,
    })
}

struct TileRange {
    x_min: i64,
    x_max: i64,
    y_min: i64,
    y_max: i64,
}

fn quadtree_cover(zoom: u8, range: TileRange, found: &mut HashSet<TileId>) {
    fn visit(
        level: u8,
        x: u32,
        y: u32,
        target: u8,
        range: &TileRange,
        found: &mut HashSet<TileId>,
    ) {
        let span = 1i64 << (target - level);
        let tile_min_x = i64::from(x) * span;
        let tile_max_x = tile_min_x + span - 1;
        let tile_min_y = i64::from(y) * span;
        let tile_max_y = tile_min_y + span - 1;
        if tile_max_x < range.x_min
            || tile_min_x > range.x_max
            || tile_max_y < range.y_min
            || tile_min_y > range.y_max
        {
            return;
        }
        if level == target {
            found.insert(TileId { zoom: target, x, y });
            return;
        }
        for child_y in 0..2 {
            for child_x in 0..2 {
                visit(
                    level + 1,
                    x * 2 + child_x,
                    y * 2 + child_y,
                    target,
                    range,
                    found,
                );
            }
        }
    }

    visit(0, 0, 0, zoom, &range, found);
}

#[cfg(test)]
mod tests {
    use super::{BaseLayer, TileSource, XyzTileSource, cover_tiles};
    use crate::geo::{GeoPoint, TileId};

    #[test]
    fn osm_url_contains_all_xyz_components() {
        let source = XyzTileSource::osm();
        let url = source.url(TileId::new(12, 3210, 2030).unwrap());
        assert_eq!(url, "https://tile.openstreetmap.org/12/3210/2030.png");
        assert_eq!(source.attribution(), "© OpenStreetMap contributors");
    }

    #[test]
    fn cover_has_expected_size_when_away_from_edges() {
        let cover = cover_tiles(GeoPoint::new(115.225, -8.648), 12, 2).unwrap();
        assert_eq!(cover.len(), 25);
        assert!(cover.tiles.iter().all(|tile| tile.zoom == 12));
    }

    #[test]
    fn cover_wraps_x_at_date_line() {
        let cover = cover_tiles(GeoPoint::new(179.999, 0.0), 2, 1).unwrap();
        assert!(cover.tiles.iter().any(|tile| tile.x == 0));
        assert!(cover.tiles.iter().any(|tile| tile.x == 3));
    }

    #[test]
    fn cover_is_unique_at_low_zoom() {
        let cover = cover_tiles(GeoPoint::new(0.0, 0.0), 1, 4).unwrap();
        assert_eq!(cover.len(), 4);
    }

    #[test]
    fn arcgis_url_uses_row_column_order() {
        let source = XyzTileSource::arcgis_imagery();
        let url = source.url(TileId::new(12, 3210, 2030).unwrap());
        assert_eq!(
            url,
            "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/12/2030/3210"
        );
        assert_eq!(source.max_zoom(), 19);
        assert!(source.attribution().contains("Esri"));
    }

    #[test]
    fn every_base_layer_exposes_a_source() {
        for layer in BaseLayer::ALL {
            let source = layer.tile_source();
            assert_eq!(source.max_zoom(), layer.max_zoom());
            assert_eq!(source.attribution(), layer.attribution());
        }
    }

    #[test]
    fn proxy_source_uses_the_local_tile_route() {
        let source =
            BaseLayer::ArcGisImagery.tile_source_with_proxy(Some("http://127.0.0.1:8010/"));
        assert_eq!(
            source.url(TileId::new(19, 429948, 274783).unwrap()),
            "http://127.0.0.1:8010/tiles/satellite/19/429948/274783.png"
        );
        assert_eq!(source.max_zoom(), 19);
    }

    #[test]
    fn switching_layers_cycles_back_to_the_first_layer() {
        let mut layer = BaseLayer::default();
        for _ in 0..BaseLayer::ALL.len() {
            layer = layer.next();
        }
        assert_eq!(layer, BaseLayer::default());
    }
}
