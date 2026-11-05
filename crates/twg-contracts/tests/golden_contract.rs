use twg_contracts::alurs::Alur;
use twg_contracts::animations::Animation;
use twg_contracts::auth::LoginResponse;
use twg_contracts::envelope::{ErrorResponse, ListResponse, SingleResponse};
use twg_contracts::last_positions::LastPositionMap;
use twg_contracts::map::{Building, Line, Marker, Particle, TextMarker};
use twg_contracts::operations::{HomeOperation, Operation};
use twg_contracts::satkers::Satker;
use twg_contracts::scenarios::Scenario;
use twg_contracts::snapshots::Snapshot;
use twg_contracts::users::{OnlineUser, User};

const LOGIN: &str = include_str!("../../../baseline/golden/redacted/auth-login-admin.json");
const USERS: &str =
    include_str!("../../../baseline/golden/redacted/private-admin-_private_users.json");
const SATKERS: &str =
    include_str!("../../../baseline/golden/redacted/private-admin-_private_satkers.json");
const OPERATION: &str = include_str!(
    "../../../baseline/golden/redacted/private-admin-_private_operasis_33333333_3333_4333_8333_333333333331.json"
);
const HOME_OPERATIONS: &str =
    include_str!("../../../baseline/golden/redacted/public_public_list_operasis.json");
const SCENARIO: &str = include_str!(
    "../../../baseline/golden/redacted/public_public_skenarios_get_44444444_4444_4444_8444_444444444441.json"
);
const SCENARIOS: &str = include_str!(
    "../../../baseline/golden/redacted/public_public_skenarios_33333333_3333_4333_8333_333333333331.json"
);
const MARKERS: &str = include_str!(
    "../../../baseline/golden/redacted/public_public_markers_list_44444444_4444_4444_8444_444444444441.json"
);
const BUILDINGS: &str = include_str!(
    "../../../baseline/golden/redacted/public_public_buildings_list_44444444_4444_4444_8444_444444444441.json"
);
const LINES: &str = include_str!(
    "../../../baseline/golden/redacted/public_public_lines_list_44444444_4444_4444_8444_444444444441.json"
);
const TEXT_MARKERS: &str = include_str!(
    "../../../baseline/golden/redacted/public_public_text_markers_list_44444444_4444_4444_8444_444444444441.json"
);
const PARTICLES: &str = include_str!(
    "../../../baseline/golden/redacted/public_public_particles_list_44444444_4444_4444_8444_444444444441.json"
);
const ANIMATIONS: &str = include_str!(
    "../../../baseline/golden/redacted/public_public_anims_list_44444444_4444_4444_8444_444444444441.json"
);
const ALURS: &str = include_str!(
    "../../../baseline/golden/redacted/public_public_alurs_list_44444444_4444_4444_8444_444444444441.json"
);
const SNAPSHOT: &str = include_str!(
    "../../../baseline/golden/redacted/public_public_game_sessions_snapshot_44444444_4444_4444_8444_444444444441.json"
);
const LAST_POSITIONS: &str = include_str!(
    "../../../baseline/golden/redacted/public_public_last_positions_44444444_4444_4444_8444_444444444441.json"
);
const ONLINE_USERS: &str =
    include_str!("../../../baseline/golden/redacted/public_public_users_online.json");
const FORBIDDEN: &str = include_str!(
    "../../../baseline/golden/redacted/admin-user-_admin_categories_pageSize_10_pageNumber_1_search_.json"
);

#[test]
fn parses_login_response() {
    let response: SingleResponse<LoginResponse> = serde_json::from_str(LOGIN).unwrap();
    assert_eq!(response.code, 200);
    assert_eq!(response.data.user.username, "baselineadmin");
    assert!(response.data.user.units.is_empty());
    assert!(response.data.user.satker_id.is_none());
}

#[test]
fn parses_user_list_with_legacy_units_string() {
    let response: ListResponse<User> = serde_json::from_str(USERS).unwrap();
    assert_eq!(response.total, 5);
    assert_eq!(response.data[0].units, "[]");
    assert!(response.data[0].satker_id.is_none());
    assert!(response.data[2].satker_id.is_some());
}

#[test]
fn parses_satker_naive_timestamps() {
    let response: ListResponse<Satker> = serde_json::from_str(SATKERS).unwrap();
    assert_eq!(response.total, 2);
    assert_eq!(response.data[0].code, "SAT-ALPHA");
    assert_eq!(
        response.data[0].created_at.to_string(),
        "2026-01-01 01:00:00"
    );
}

#[test]
fn parses_operation_and_home_operations() {
    let operation: SingleResponse<Operation> = serde_json::from_str(OPERATION).unwrap();
    assert_eq!(operation.data.name, "Operasi Alpha");
    assert!(operation.data.satker_id.is_some());

    let home: ListResponse<HomeOperation> = serde_json::from_str(HOME_OPERATIONS).unwrap();
    assert_eq!(home.total, 3);
    assert_eq!(home.data[0].skenario.len(), 1);
    assert_eq!(home.data[1].skenario.len(), 2);
}

#[test]
fn parses_scenario_detail_and_list() {
    let detail: SingleResponse<Scenario> = serde_json::from_str(SCENARIO).unwrap();
    assert_eq!(detail.data.operasi.name, "Operasi Alpha");
    assert_eq!(detail.data.center_x, 115.22528720723322);
    assert_eq!(detail.data.map_mode, twg_domain::MapMode::Outdoor);

    let list: ListResponse<Scenario> = serde_json::from_str(SCENARIOS).unwrap();
    assert_eq!(list.data.len(), 2);
    assert_eq!(list.data[1].map_mode, twg_domain::MapMode::Indoor);
    assert_eq!(list.data[1].indoor_config["grid_size_m"], 1);
}

#[test]
fn parses_map_objects() {
    let markers: ListResponse<Marker> = serde_json::from_str(MARKERS).unwrap();
    assert_eq!(markers.data[0].name, "Unit Alpha 1");
    assert_eq!(markers.data[0].scale_y, 1.0);
    assert_eq!(markers.data[0].skenario.name, "Skenario Alpha");

    let buildings: ListResponse<Building> = serde_json::from_str(BUILDINGS).unwrap();
    assert_eq!(buildings.data[0].height, 12.5);
    assert_eq!(buildings.data[0].color, "#ff8800");

    let lines: ListResponse<Line> = serde_json::from_str(LINES).unwrap();
    assert_eq!(lines.data[0].width, 4.0);
    assert_eq!(lines.data[0].alpha, 0.8);

    let text_markers: ListResponse<TextMarker> = serde_json::from_str(TEXT_MARKERS).unwrap();
    assert_eq!(text_markers.data[0].align, twg_domain::TextAlign::Center);

    let particles: ListResponse<Particle> = serde_json::from_str(PARTICLES).unwrap();
    assert_eq!(particles.data[0].tipe, "fire");
    assert_eq!(particles.data[0].is_spawn, 0);
}

#[test]
fn parses_animations_and_alurs() {
    let animations: ListResponse<Animation> = serde_json::from_str(ANIMATIONS).unwrap();
    assert_eq!(animations.data.len(), 4);
    assert_eq!(animations.data[0].tipe, "route");
    assert_eq!(animations.data[0].urut, 1);
    assert!(!animations.data[0].autoplay);

    let alurs: ListResponse<Alur> = serde_json::from_str(ALURS).unwrap();
    assert_eq!(alurs.data[0].alur, "Alur fixture Alpha");
}

#[test]
fn parses_snapshot_and_last_positions() {
    let snapshot: SingleResponse<Snapshot> = serde_json::from_str(SNAPSHOT).unwrap();
    assert!(snapshot.data.snapshot_enabled);
    assert_eq!(snapshot.data.version, 1);
    assert_eq!(snapshot.data.map_state["zoom"], 16);

    let positions: SingleResponse<LastPositionMap> = serde_json::from_str(LAST_POSITIONS).unwrap();
    assert_eq!(positions.data.last_map.zoom, 16.0);
    assert_eq!(positions.data.last_position.len(), 1);
    assert_eq!(positions.data.last_position[0].rot_z, 45.0);
}

#[test]
fn parses_online_users_and_error() {
    let online: ListResponse<OnlineUser> = serde_json::from_str(ONLINE_USERS).unwrap();
    assert_eq!(online.total, 5);
    assert_eq!(online.data[0].active_session_count, 1);
    assert_eq!(online.data[0].sessions.len(), 1);

    let error: ErrorResponse = serde_json::from_str(FORBIDDEN).unwrap();
    assert_eq!(error.code, 403);
    assert_eq!(error.message, "forbidden role");
}
