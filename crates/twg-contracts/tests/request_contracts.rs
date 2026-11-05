use twg_contracts::animations::{SpawnAnimationRequest, SpawnPickupRequest};
use twg_contracts::auth::{LoginRequest, RefreshTokenRequest};
use twg_contracts::map::MarkerCreateRequest;
use twg_contracts::scenarios::ScenarioCreateRequest;
use twg_contracts::snapshots::SnapshotUpsertRequest;
use twg_contracts::websocket::{CommandPayload, PublishCommandRequest};

#[test]
fn login_request_defaults_device_id() {
    let request: LoginRequest =
        serde_json::from_str(r#"{"username":"baselineadmin","password":"secret123"}"#).unwrap();
    assert_eq!(request.device_id, "");
}

#[test]
fn refresh_request_round_trips() {
    let request: RefreshTokenRequest = serde_json::from_str(
        r#"{"refresh_token":"v2.local.token","device_id":"baseline-admin-device"}"#,
    )
    .unwrap();
    assert_eq!(request.device_id, "baseline-admin-device");
}

#[test]
fn marker_request_matches_baseline_fields() {
    let request: MarkerCreateRequest = serde_json::from_str(
        r#"{"name":"Unit","operasi_id":"op","skenario_id":"sk","unit_id":"u","kategori":"kendaraan","pos_x":115.2,"pos_y":-8.6,"jumlah":1}"#,
    )
    .unwrap();
    assert_eq!(request.jumlah, 1);
    assert_eq!(request.pos_x, 115.2);
}

#[test]
fn scenario_request_accepts_indoor_config() {
    let request: ScenarioCreateRequest = serde_json::from_str(
        r#"{"name":"Indoor","operasi_id":"op","zoom":21,"max_zoom":26,"map_mode":"indoor","base_layer":"grid_dark","indoor_config":{"grid_size_m":1}}"#,
    )
    .unwrap();
    assert_eq!(request.map_mode, "indoor");
    assert_eq!(request.indoor_config.unwrap()["grid_size_m"], 1);
}

#[test]
fn snapshot_request_preserves_json_state() {
    let request: SnapshotUpsertRequest = serde_json::from_str(
        r#"{"operasi_id":"op","skenario_id":"sk","snapshot_enabled":true,"map_state":{"zoom":16},"markers_state":{"markers":[]},"anims_state":{"anims":[]}}"#,
    )
    .unwrap();
    assert!(request.snapshot_enabled);
    assert_eq!(request.map_state["zoom"], 16);
}

#[test]
fn websocket_payload_omits_missing_satker() {
    let payload = CommandPayload {
        operasi_id: "op".to_owned(),
        skenario_id: "sk".to_owned(),
        marker_id: "marker".to_owned(),
        command: "MOVE".to_owned(),
        data: "{}".to_owned(),
        is_save: 0,
        satker_id: None,
    };
    let value = serde_json::to_value(payload).unwrap();
    assert!(value.get("satker_id").is_none());
}

#[test]
fn publish_command_request_matches_baseline() {
    let request: PublishCommandRequest = serde_json::from_str(
        r#"{"operasi_id":"op","skenario_id":"sk","marker_id":"m","command":"MOVE","data":"{}","is_save":1}"#,
    )
    .unwrap();
    assert_eq!(request.is_save, 1);
    assert_eq!(request.command, "MOVE");
}

#[test]
fn spawn_requests_match_baseline_shapes() {
    let spawn: SpawnAnimationRequest = serde_json::from_str(
        r#"{"operasi_id":"op","skenario_id":"sk","unit_id":"u","pos_x":115.2,"pos_y":-8.6,"scale":1,"marker_name":"M","name":"Spawn","username":"admin"}"#,
    )
    .unwrap();
    assert_eq!(spawn.marker_name, "M");

    let pickup: SpawnPickupRequest = serde_json::from_str(
        r#"{"operasi_id":"op","skenario_id":"sk","marker_ids":["a","b"],"name":"Pickup","username":"admin"}"#,
    )
    .unwrap();
    assert_eq!(pickup.marker_ids.len(), 2);
}
