# Taste

## Code Style

- Wants a professional, modular project layout: never cram a crate/domain into a single `lib.rs`; split it into one module (often one file) per domain/resource so maintenance stays easy. Confidence: 0.85

## Workflow

- Prefers a written, step-by-step execution checklist (staged steps with exit gates) before moving on to the next stage of implementation work. Confidence: 0.6

## Tooling

- Runs containers with Podman Desktop (podman CLI), not Docker; projects should target Podman, and Postgres images are taken from the locally available `postgres:18.4-alpine`. Confidence: 0.75
- Uses OpenStreetMap tiles via maptalks and wants no map API keys at all — keyed tile providers (MapTiler, etc.) should be removed from plans rather than rotated or proxied. Confidence: 0.65
