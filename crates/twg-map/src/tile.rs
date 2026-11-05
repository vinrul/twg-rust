use crate::geo::{GeoError, GeoPoint, TileId};

pub const OSM_TILE_TEMPLATE: &str = "https://tile.openstreetmap.org/{z}/{x}/{y}.png";
pub const OSM_ATTRIBUTION: &str = "© OpenStreetMap contributors";
pub const ARCGIS_IMAGERY_TILE_TEMPLATE: &str =
    "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}";
pub const ARCGIS_IMAGERY_ATTRIBUTION: &str = "Tiles © Esri — Source: Esri, i-cubed, USDA, USGS, AEX, GeoEye, Getmapping, Aerogrid, IGN, IGP, UPR-EGP, and the GIS User Community";
pub const ARCGIS_IMAGERY_MAX_ZOOM: u8 = 19;
pub const OSM_MAX_ZOOM: u8 = 19;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BaseLayer {
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

impl Default for BaseLayer {
    fn default() -> Self {
        Self::OpenStreetMap
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
    let mut tiles = Vec::new();

    for delta_y in -radius..=radius {
        let y = center_y + delta_y;
        if !(0..count).contains(&y) {
            continue;
        }
        for delta_x in -radius..=radius {
            let x = (center_x + delta_x).rem_euclid(count);
            let tile = TileId {
                zoom,
                x: x as u32,
                y: y as u32,
            };
            if !tiles.contains(&tile) {
                tiles.push(tile);
            }
        }
    }

    tiles.sort_unstable_by_key(|tile| (tile.y, tile.x));
    Ok(TileCover {
        zoom,
        center: center_tile,
        radius: radius as u32,
        tiles,
    })
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
}
