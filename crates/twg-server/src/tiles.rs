use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    Router,
    extract::{Extension, Path as AxumPath},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use tokio::sync::{Mutex, Semaphore};

const OSM_TEMPLATE: &str = "https://tile.openstreetmap.org/{z}/{x}/{y}.png";
const ESRI_TEMPLATE: &str =
    "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}";
const USER_AGENT: &str = "twg-rust-tile-proxy/0.1 (+https://github.com/vinrul/twg-rust)";
const MAX_PROVIDER_ZOOM: u8 = 19;

#[derive(Clone)]
pub struct TileProxy {
    agent: Arc<ureq::Agent>,
    cache_dir: Arc<PathBuf>,
    semaphore: Arc<Semaphore>,
    throttle: Arc<Mutex<Option<Instant>>>,
    min_interval: Duration,
}

impl TileProxy {
    pub fn from_env() -> Self {
        let cache_dir =
            std::env::var("TILE_CACHE_DIR").unwrap_or_else(|_| ".tile-cache".to_owned());
        let concurrency = std::env::var("TILE_PROXY_CONCURRENCY")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(8usize)
            .max(1);
        let min_interval_ms = std::env::var("TILE_PROXY_MIN_INTERVAL_MS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(50u64);
        let agent = ureq::Agent::config_builder().build().new_agent();

        Self {
            agent: Arc::new(agent),
            cache_dir: Arc::new(PathBuf::from(cache_dir)),
            semaphore: Arc::new(Semaphore::new(concurrency)),
            throttle: Arc::new(Mutex::new(None)),
            min_interval: Duration::from_millis(min_interval_ms),
        }
    }
}

pub fn router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/tiles/{layer}/{z}/{x}/{y}", get(tile))
        .layer(Extension(TileProxy::from_env()))
}

async fn tile(
    Extension(proxy): Extension<TileProxy>,
    AxumPath((layer, z, x, y)): AxumPath<(String, u8, u32, String)>,
) -> Response {
    let Some(layer) = normalize_layer(&layer) else {
        return bad_request("unknown tile layer");
    };
    if z > MAX_PROVIDER_ZOOM {
        return bad_request("zoom above provider maximum");
    }
    let count = 1u32 << z;
    let y = y.strip_suffix(".png").unwrap_or(&y);
    let Ok(y) = y.parse::<u32>() else {
        return bad_request("invalid tile row");
    };
    if x >= count || y >= count {
        return bad_request("tile coordinate out of range");
    }

    let path = cache_path(&proxy.cache_dir, layer, z, x, y);
    if let Ok(bytes) = tokio::fs::read(&path).await {
        return tile_response(bytes);
    }

    let Ok(_permit) = proxy.semaphore.acquire().await else {
        return (StatusCode::SERVICE_UNAVAILABLE, "tile proxy closed").into_response();
    };
    {
        let mut last = proxy.throttle.lock().await;
        if let Some(previous) = *last {
            let elapsed = previous.elapsed();
            if elapsed < proxy.min_interval {
                tokio::time::sleep(proxy.min_interval - elapsed).await;
            }
        }
        *last = Some(Instant::now());
    }

    let url = upstream_url(layer, z, x, y);
    let request_url = url.clone();
    let agent = proxy.agent.clone();
    let fetched = tokio::task::spawn_blocking(move || {
        let mut response = agent
            .get(&request_url)
            .header("User-Agent", USER_AGENT)
            .call()
            .map_err(|error| error.to_string())?;
        let mut bytes = Vec::new();
        response
            .body_mut()
            .with_config()
            .reader()
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        Ok::<Vec<u8>, String>(bytes)
    })
    .await;

    let bytes = match fetched {
        Ok(Ok(bytes)) => bytes,
        Ok(Err(error)) => {
            tracing::warn!(%url, %error, "tile proxy upstream failed");
            return (StatusCode::BAD_GATEWAY, "upstream tile failed").into_response();
        }
        Err(error) => {
            tracing::warn!(%url, %error, "tile proxy task failed");
            return (StatusCode::BAD_GATEWAY, "upstream task failed").into_response();
        }
    };

    if let Some(parent) = path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    if let Err(error) = tokio::fs::write(&path, &bytes).await {
        tracing::warn!(?path, %error, "tile cache write failed");
    }
    tile_response(bytes)
}

fn normalize_layer(layer: &str) -> Option<&'static str> {
    match layer.to_ascii_lowercase().as_str() {
        "osm" | "openstreetmap" | "street" => Some("osm"),
        "satellite" | "esri" | "arcgis" => Some("satellite"),
        _ => None,
    }
}

fn upstream_url(layer: &str, z: u8, x: u32, y: u32) -> String {
    match layer {
        "satellite" => ESRI_TEMPLATE
            .replace("{z}", &z.to_string())
            .replace("{x}", &x.to_string())
            .replace("{y}", &y.to_string()),
        _ => OSM_TEMPLATE
            .replace("{z}", &z.to_string())
            .replace("{x}", &x.to_string())
            .replace("{y}", &y.to_string()),
    }
}

fn cache_path(root: &Path, layer: &str, z: u8, x: u32, y: u32) -> PathBuf {
    root.join(layer)
        .join(z.to_string())
        .join(x.to_string())
        .join(format!("{y}.png"))
}

fn tile_response(bytes: Vec<u8>) -> Response {
    (
        [
            (header::CONTENT_TYPE, "image/png"),
            (header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        bytes,
    )
        .into_response()
}

fn bad_request(message: &str) -> Response {
    (StatusCode::BAD_REQUEST, message.to_owned()).into_response()
}

#[cfg(test)]
mod tests {
    use super::{cache_path, normalize_layer, upstream_url};
    use std::path::Path;

    #[test]
    fn layers_are_normalized() {
        assert_eq!(normalize_layer("OSM"), Some("osm"));
        assert_eq!(normalize_layer("satellite"), Some("satellite"));
        assert_eq!(normalize_layer("unknown"), None);
    }

    #[test]
    fn upstream_urls_match_provider_templates() {
        assert_eq!(
            upstream_url("osm", 19, 429948, 274783),
            "https://tile.openstreetmap.org/19/429948/274783.png"
        );
        assert_eq!(
            upstream_url("satellite", 19, 429948, 274783),
            "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/19/274783/429948"
        );
    }

    #[test]
    fn cache_path_is_hierarchical() {
        let path = cache_path(Path::new("cache"), "osm", 19, 12, 34);
        assert_eq!(path, Path::new("cache/osm/19/12/34.png"));
    }
}
