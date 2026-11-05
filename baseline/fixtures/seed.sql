-- Baseline fixture untuk database test lokal.
-- Password semua akun: Baseline#2026
-- Hash dibuat ulang dengan scripts/hash-bcrypt.
--
-- Guard: seed hanya boleh dijalankan pada database bernama twg_baseline.

DO $$
BEGIN
    IF current_database() <> 'twg_baseline' THEN
        RAISE EXCEPTION 'Refusing to seed database %, expected twg_baseline', current_database();
    END IF;
END $$;

TRUNCATE TABLE
    game_session_snapshots,
    user_sessions,
    commands,
    last_positions,
    last_maps,
    anims,
    alurs,
    particles,
    text_markers,
    plot_lines,
    buildings,
    markers,
    skenarios,
    operasis,
    satkers,
    users,
    categories
RESTART IDENTITY CASCADE;

-- Users inserted first because satkers.created_by references users.
INSERT INTO users (id, name, username, password, role, units, profile_img, created_at, updated_at)
VALUES
    ('aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1', 'Baseline Admin', 'baselineadmin', '$2a$10$7BuDXppm38Mpqv/04I3qQOjuBC4HqDGjjgBtUziquCOMW0iS2aV9m', 'admin', '[]', '', '2026-01-01 00:00:00', '2026-01-01 00:00:00'),
    ('aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa2', 'Baseline Monitor', 'baselinemonitor', '$2a$10$7BuDXppm38Mpqv/04I3qQOjuBC4HqDGjjgBtUziquCOMW0iS2aV9m', 'admin_monitor', '[]', '', '2026-01-01 00:01:00', '2026-01-01 00:01:00'),
    ('aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa3', 'Baseline Admin Satker', 'baselineadminsatker', '$2a$10$7BuDXppm38Mpqv/04I3qQOjuBC4HqDGjjgBtUziquCOMW0iS2aV9m', 'admin_satker', '[]', '', '2026-01-01 00:02:00', '2026-01-01 00:02:00'),
    ('aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa4', 'Baseline User Satker', 'baselineusersatker', '$2a$10$7BuDXppm38Mpqv/04I3qQOjuBC4HqDGjjgBtUziquCOMW0iS2aV9m', 'user_satker', '[]', '', '2026-01-01 00:03:00', '2026-01-01 00:03:00'),
    ('aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa5', 'Baseline User', 'baselineuser', '$2a$10$7BuDXppm38Mpqv/04I3qQOjuBC4HqDGjjgBtUziquCOMW0iS2aV9m', 'user', '[]', '', '2026-01-01 00:04:00', '2026-01-01 00:04:00'),
    ('aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa6', 'Baseline User Beta', 'baselineuserbeta', '$2a$10$7BuDXppm38Mpqv/04I3qQOjuBC4HqDGjjgBtUziquCOMW0iS2aV9m', 'user_satker', '[]', '', '2026-01-01 00:05:00', '2026-01-01 00:05:00');

INSERT INTO satkers (id, code, name, is_active, created_by, created_at, updated_at)
VALUES
    ('11111111-1111-4111-8111-111111111111', 'SAT-ALPHA', 'Satker Alpha', TRUE, 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1', '2026-01-01 01:00:00', '2026-01-01 01:00:00'),
    ('22222222-2222-4222-8222-222222222222', 'SAT-BETA', 'Satker Beta', TRUE, 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1', '2026-01-01 01:01:00', '2026-01-01 01:01:00');

UPDATE users SET satker_id = '11111111-1111-4111-8111-111111111111'
WHERE id IN ('aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa3', 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa4');

UPDATE users SET satker_id = '22222222-2222-4222-8222-222222222222'
WHERE id = 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa6';

INSERT INTO categories (id, name, icon, created_at, updated_at)
VALUES
    ('c0000000-0000-4000-8000-000000000001', 'Infanteri', 'infantry', '2026-01-01 02:00:00', '2026-01-01 02:00:00'),
    ('c0000000-0000-4000-8000-000000000002', 'Kendaraan', 'vehicle', '2026-01-01 02:01:00', '2026-01-01 02:01:00');

INSERT INTO operasis (id, name, satker_id, created_at, updated_at)
VALUES
    ('33333333-3333-4333-8333-333333333330', 'Operasi Global', NULL, '2026-01-02 00:00:00', '2026-01-02 00:00:00'),
    ('33333333-3333-4333-8333-333333333331', 'Operasi Alpha', '11111111-1111-4111-8111-111111111111', '2026-01-02 01:00:00', '2026-01-02 01:00:00'),
    ('33333333-3333-4333-8333-333333333332', 'Operasi Beta', '22222222-2222-4222-8222-222222222222', '2026-01-02 02:00:00', '2026-01-02 02:00:00');

INSERT INTO skenarios (
    id, operasi_id, satker_id, name, center_x, center_y, zoom, min_zoom, max_zoom, pitch, bearing,
    map_mode, base_layer, indoor_config, created_at, updated_at
)
VALUES
    (
        '44444444-4444-4444-8444-444444444440', '33333333-3333-4333-8333-333333333330', NULL,
        'Skenario Global', 115.22528720723322, -8.648393442650265, 15, 2, 19.4, 0, 0,
        'outdoor', 'osm', '{}', '2026-01-03 00:00:00', '2026-01-03 00:00:00'
    ),
    (
        '44444444-4444-4444-8444-444444444441', '33333333-3333-4333-8333-333333333331', '11111111-1111-4111-8111-111111111111',
        'Skenario Alpha', 115.22528720723322, -8.648393442650265, 15, 2, 19.4, 45, 30,
        'outdoor', 'satellite', '{}', '2026-01-03 01:00:00', '2026-01-03 01:00:00'
    ),
    (
        '44444444-4444-4444-8444-444444444442', '33333333-3333-4333-8333-333333333332', '22222222-2222-4222-8222-222222222222',
        'Skenario Beta', 115.23000000000000, -8.650000000000000, 14, 2, 19.4, 0, 0,
        'outdoor', 'osm', '{}', '2026-01-03 02:00:00', '2026-01-03 02:00:00'
    ),
    (
        '44444444-4444-4444-8444-444444444443', '33333333-3333-4333-8333-333333333331', '11111111-1111-4111-8111-111111111111',
        'Skenario Indoor Alpha', 0, 0, 21, 21, 26, 0, 0,
        'indoor', 'grid_dark',
        $json${"background":"grid_dark","grid_size_m":1,"floor":1,"floor_plan_url":"","floor_plan_width_m":100,"floor_plan_height_m":60,"floor_plan_opacity":0.9}$json$,
        '2026-01-03 03:00:00', '2026-01-03 03:00:00'
    );

INSERT INTO markers (
    id, operasi_id, skenario_id, unit_id, name, kategori, jumlah,
    rot_x, rot_y, rot_z, pos_x, pos_y, keterangan, scale, is_spawn, shoot,
    scale_y, scale_z, altitude, created_at, updated_at
)
VALUES
    (
        '55555555-5555-4555-8555-555555555551', '33333333-3333-4333-8333-333333333331',
        '44444444-4444-4444-8444-444444444441', 'abababab-abab-4bab-8bab-abababababab',
        'Unit Alpha 1', 'kendaraan', 1, 0, 0, 30, 115.2250, -8.6480,
        'Unit fixture Alpha', 1, 0, '', 1, 1, 0, '2026-01-04 00:00:00', '2026-01-04 00:00:00'
    ),
    (
        '55555555-5555-4555-8555-555555555552', '33333333-3333-4333-8333-333333333330',
        '44444444-4444-4444-8444-444444444440', 'abababab-abab-4bab-8bab-abababababab',
        'Unit Global 1', 'kendaraan', 2, 0, 0, 0, 115.2260, -8.6490,
        'Unit fixture Global', 1, 0, '', 1, 1, 0, '2026-01-04 01:00:00', '2026-01-04 01:00:00'
    ),
    (
        '55555555-5555-4555-8555-555555555553', '33333333-3333-4333-8333-333333333332',
        '44444444-4444-4444-8444-444444444442', 'abababab-abab-4bab-8bab-abababababab',
        'Unit Beta 1', 'kendaraan', 1, 0, 0, 0, 115.2300, -8.6500,
        'Unit fixture Beta', 1, 0, '', 1, 1, 0, '2026-01-04 02:00:00', '2026-01-04 02:00:00'
    );

INSERT INTO buildings (id, operasi_id, skenario_id, name, keterangan, color, geom, height, created_at, updated_at)
VALUES (
    '66666666-6666-4666-8666-666666666661', '33333333-3333-4333-8333-333333333331',
    '44444444-4444-4444-8444-444444444441', 'Gedung Alpha', 'Building fixture',
    '#ff8800', $json$[[115.2248,-8.6488],[115.2252,-8.6488],[115.2252,-8.6484],[115.2248,-8.6484]]$json$,
    12.5, '2026-01-05 00:00:00', '2026-01-05 00:00:00'
);

INSERT INTO plot_lines (id, operasi_id, skenario_id, name, color, width, alpha, geom, created_at, updated_at)
VALUES (
    '77777777-7777-4777-8777-777777777771', '33333333-3333-4333-8333-333333333331',
    '44444444-4444-4444-8444-444444444441', 'Garis Alpha', '#00aaff', 4, 0.8,
    $json$[[115.2249,-8.6490],[115.2255,-8.6486],[115.2260,-8.6489]]$json$,
    '2026-01-05 01:00:00', '2026-01-05 01:00:00'
);

INSERT INTO text_markers (
    id, operasi_id, skenario_id, text, color, font_size, align, wrap_text, pos_x, pos_y, created_at, updated_at
)
VALUES (
    '88888888-8888-4888-8888-888888888881', '33333333-3333-4333-8333-333333333331',
    '44444444-4444-4444-8444-444444444441', 'Titik Kumpul Alpha', '#ffffff', 18, 'center', FALSE,
    115.2251, -8.6487, '2026-01-05 02:00:00', '2026-01-05 02:00:00'
);

INSERT INTO particles (
    id, operasi_id, skenario_id, name, pos_x, pos_y, data, is_spawn, tipe, created_at, updated_at
)
VALUES (
    '99999999-9999-4999-8999-999999999991', '33333333-3333-4333-8333-333333333331',
    '44444444-4444-4444-8444-444444444441', 'Particle Alpha', 115.2253, -8.6485,
    $json${"color":"#ff5500","count":20}$json$, 0, 'fire', '2026-01-05 03:00:00', '2026-01-05 03:00:00'
);

INSERT INTO anims (
    id, operasi_id, skenario_id, name, username, keterangan, tipe, data, geom,
    speed, detik, urut, replay, autoplay, created_at, updated_at
)
VALUES
    (
        'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb1', '33333333-3333-4333-8333-333333333331',
        '44444444-4444-4444-8444-444444444441', 'Route Alpha', 'baselineadmin',
        'Route fixture', 'route',
        $json$[{"x":115.2250,"y":-8.6480,"z":0},{"x":115.2260,"y":-8.6485,"z":0}]$json$,
        $json$[[115.2250,-8.6480],[115.2260,-8.6485]]$json$,
        10, 10, 1, FALSE, FALSE, '2026-01-06 00:00:00', '2026-01-06 00:00:00'
    ),
    (
        'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb2', '33333333-3333-4333-8333-333333333331',
        '44444444-4444-4444-8444-444444444441', 'Spawn Alpha', 'baselineadmin',
        'Spawn fixture', 'spawn',
        $json${"marker_id":"55555555-5555-4555-8555-555555555551"}$json$, '',
        10, 5, 2, FALSE, FALSE, '2026-01-06 01:00:00', '2026-01-06 01:00:00'
    ),
    (
        'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb3', '33333333-3333-4333-8333-333333333331',
        '44444444-4444-4444-8444-444444444441', 'Sequence Alpha', 'baselineadmin',
        'Sequence fixture', 'sequence',
        $json${"children":[{"id":"route-1","tipe":"route","data":"[]","geom":"[]","detik":5,"speed":10}],"delay_ms":100}$json$, '',
        10, 5, 3, FALSE, FALSE, '2026-01-06 02:00:00', '2026-01-06 02:00:00'
    ),
    (
        'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb4', '33333333-3333-4333-8333-333333333331',
        '44444444-4444-4444-8444-444444444441', 'Shoot Alpha', 'baselineadmin',
        'Shoot fixture', 'shoot',
        $json${"marker_id":"55555555-5555-4555-8555-555555555551","tipe":"fire"}$json$, '',
        10, 2, 4, FALSE, FALSE, '2026-01-06 03:00:00', '2026-01-06 03:00:00'
    );

INSERT INTO alurs (id, operasi_id, skenario_id, alur, created_at, updated_at)
VALUES (
    'acacacac-acac-4cac-8cac-acacacacacac', '33333333-3333-4333-8333-333333333331',
    '44444444-4444-4444-8444-444444444441', 'Alur fixture Alpha',
    '2026-01-06 04:00:00', '2026-01-06 04:00:00'
);

INSERT INTO commands (
    id, operasi_id, skenario_id, marker_id, command, data, created_at, updated_at
)
VALUES (
    'cccccccc-cccc-4ccc-8ccc-ccccccccccc1', '33333333-3333-4333-8333-333333333331',
    '44444444-4444-4444-8444-444444444441', '55555555-5555-4555-8555-555555555551',
    'MOVE', $json${"x":115.2252,"y":-8.6482,"z":0}$json$,
    '2026-01-06 05:00:00', '2026-01-06 05:00:00'
);

INSERT INTO last_maps (
    id, operasi_id, skenario_id, center_x, center_y, zoom, bearing, pitch, created_at, updated_at
)
VALUES (
    'dddddddd-dddd-4ddd-8ddd-ddddddddddd1', '33333333-3333-4333-8333-333333333331',
    '44444444-4444-4444-8444-444444444441', 115.2253, -8.6484, 16, 15, 35,
    '2026-01-06 06:00:00', '2026-01-06 06:00:00'
);

INSERT INTO last_positions (
    id, operasi_id, skenario_id, marker_id, rot_x, rot_y, rot_z, pos_x, pos_y, created_at, updated_at
)
VALUES (
    'eeeeeeee-eeee-4eee-8eee-eeeeeeeeeee1', '33333333-3333-4333-8333-333333333331',
    '44444444-4444-4444-8444-444444444441', '55555555-5555-4555-8555-555555555551',
    0, 0, 45, 115.2251, -8.6481, '2026-01-06 07:00:00', '2026-01-06 07:00:00'
);

INSERT INTO game_session_snapshots (
    id, operasi_id, skenario_id, snapshot_enabled, map_state, markers_state, anims_state,
    updated_by, updated_at, version
)
VALUES (
    'ffffffff-ffff-4fff-8fff-fffffffffff1', '33333333-3333-4333-8333-333333333331',
    '44444444-4444-4444-8444-444444444441', TRUE,
    $json${"center_x":115.2253,"center_y":-8.6484,"zoom":16,"pitch":35,"bearing":15}$json$,
    $json${"markers":[]}$json$,
    $json${"anims":[]}$json$,
    'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa3', '2026-01-06 08:00:00', 1
);
