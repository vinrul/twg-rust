# twg-gltf-draco

Vendored copy of [`bevy_gltf_draco`](https://github.com/jiangheng90-opensource/bevy_gltf_draco)
0.2.1, a Bevy plugin that decodes glTF models using
`KHR_draco_mesh_compression` on native and WebAssembly targets.

It is vendored instead of pulled from crates.io because the published crate
depends on Bevy 0.19, which conflicts with this workspace's Bevy 0.20.1.

## Changes against upstream

- `Cargo.toml` uses the workspace Bevy version and dependencies.
- The `on_gltf_primitive` extension hook returns
  `impl ConditionalSendFuture<Output = ()>` as Bevy 0.20 requires, instead of
  being declared as an `async fn`.
- `futures` is only a WebAssembly dependency (it backs the one-shot channel
  that marshals Draco decode results off the browser main thread).

## Usage

```rust
use bevy::prelude::*;
use twg_gltf_draco::GltfDracoDecoderPlugin;

App::new()
    .add_plugins(DefaultPlugins)
    .add_plugins(GltfDracoDecoderPlugin)
    .run();
```

Draco-compressed glTF files must be loaded with validation disabled, because
Beam validation rejects `extensionsRequired: ["KHR_draco_mesh_compression"]`
before the extension handler can run:

```rust
asset_server.load_with_settings(path, |settings: &mut GltfLoaderSettings| {
    settings.validate = false;
});
```

## Licence

Dual licensed MIT OR Apache-2.0, matching upstream. See `LICENSE-MIT` and
`LICENSE-APACHE`.

Upstream copyright (c) 2026 jiangheng90.
