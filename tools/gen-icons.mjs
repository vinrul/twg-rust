#!/usr/bin/env node
// Generates the application icons used by the Windows resources and by the
// Linux/macOS packages. Run with: node tools/gen-icons.mjs
//
// The artwork is drawn here instead of shipping binary-only sources so the
// icons can be tweaked and regenerated on any machine with Node.js.

import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { deflateSync } from "node:zlib";

const OUT_DIR = "assets/icons";
const SIZES = [16, 24, 32, 48, 64, 128, 256, 512, 1024];

// Bright tiles keep the icon visible on dark taskbars and title bars; the
// navy glyph keeps it readable on light themes too.
const PALETTES = {
  desktop: {
    top: [0x86, 0xef, 0xac],
    bottom: [0x16, 0xa3, 0x4a],
  },
  admin: {
    top: [0xfd, 0xba, 0x74],
    bottom: [0xea, 0x58, 0x0c],
  },
};
const INK = [0x0b, 0x12, 0x20];
const GRID = [255, 255, 255];

const clamp01 = (value) => Math.min(1, Math.max(0, value));
const lerp = (a, b, t) => a + (b - a) * t;
const mix = (a, b, t) => [
  lerp(a[0], b[0], t),
  lerp(a[1], b[1], t),
  lerp(a[2], b[2], t),
];

function crc32(bytes) {
  let crc = 0xffffffff;
  for (const byte of bytes) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit += 1) {
      crc = crc & 1 ? 0xedb88320 ^ (crc >>> 1) : crc >>> 1;
    }
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function pngChunk(type, data) {
  const name = Buffer.from(type, "ascii");
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([name, data])));
  return Buffer.concat([length, name, data, crc]);
}

function encodePng(width, height, rgba) {
  const header = Buffer.alloc(13);
  header.writeUInt32BE(width, 0);
  header.writeUInt32BE(height, 4);
  header[8] = 8; // bit depth
  header[9] = 6; // RGBA
  const raw = Buffer.alloc((width * 4 + 1) * height);
  for (let y = 0; y < height; y += 1) {
    raw[y * (width * 4 + 1)] = 0; // filter: none
    rgba.copy(
      raw,
      y * (width * 4 + 1) + 1,
      y * width * 4,
      (y + 1) * width * 4,
    );
  }
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    pngChunk("IHDR", header),
    pngChunk("IDAT", deflateSync(raw, { level: 9 })),
    pngChunk("IEND", Buffer.alloc(0)),
  ]);
}

// ---------------------------------------------------------------- geometry

function roundedSquareDistance(x, y, radius) {
  const half = 0.5;
  const qx = Math.abs(x - half) - half + radius;
  const qy = Math.abs(y - half) - half + radius;
  const outside = Math.hypot(Math.max(qx, 0), Math.max(qy, 0));
  const inside = Math.min(Math.max(qx, qy), 0);
  return outside + inside - radius;
}

function inCircle(x, y, cx, cy, radius) {
  const dx = x - cx;
  const dy = y - cy;
  return dx * dx + dy * dy <= radius * radius;
}

function inTriangle(x, y, [ax, ay], [bx, by], [cx, cy]) {
  const d1 = (x - bx) * (ay - by) - (ax - bx) * (y - by);
  const d2 = (x - cx) * (by - cy) - (bx - cx) * (y - cy);
  const d3 = (x - ax) * (cy - ay) - (cx - ax) * (y - ay);
  const hasNegative = d1 < 0 || d2 < 0 || d3 < 0;
  const hasPositive = d1 > 0 || d2 > 0 || d3 > 0;
  return !(hasNegative && hasPositive);
}

// ---------------------------------------------------------------- artwork

function drawBackground(sample, x, y, palette) {
  const distance = roundedSquareDistance(x, y, 0.21);
  if (distance > 0) return;
  const color = mix(palette.top, palette.bottom, clamp01(y));
  sample.over(color, 1);
  // Faint tactical grid, clipped to the rounded square.
  const spacing = 1 / 8;
  const line = 0.0035;
  const gridDistance = (value) => {
    const fraction = (((value / spacing) % 1) + 1) % 1;
    return Math.min(fraction, 1 - fraction);
  };
  const onGrid =
    gridDistance(x) < line / spacing || gridDistance(y) < line / spacing;
  if (onGrid) sample.over(GRID, 0.16);
  // Inner highlight along the edge.
  if (distance > -0.008) sample.over([255, 255, 255], 0.28);
}

function drawDesktopGlyph(sample, x, y, size, palette) {
  const small = size <= 32;
  const head = inCircle(x, y, 0.5, 0.4, 0.165);
  const tail = inTriangle(x, y, [0.5, 0.75], [0.335, 0.42], [0.665, 0.42]);
  if (head || tail) sample.over(INK, 1);
  if (head) {
    // Punch the pin's hole back to the tile colour.
    if (inCircle(x, y, 0.5, 0.4, small ? 0.082 : 0.07)) {
      sample.over(mix(palette.top, palette.bottom, clamp01(y)), 1);
    }
  }
  // Broken crosshair ring around the pin.
  const dx = x - 0.5;
  const dy = y - 0.4;
  const radius = Math.hypot(dx, dy);
  const angle = Math.atan2(dy, dx);
  const ring = Math.abs(radius - 0.315) < (small ? 0.02 : 0.011);
  const gap = ((angle % (Math.PI / 2)) + Math.PI / 2) % (Math.PI / 2);
  if (ring && gap > 0.22 && gap < Math.PI / 2 - 0.22) {
    sample.over(INK, 0.8);
  }
  // Cardinal ticks.
  for (const [angleX, angleY] of [
    [1, 0],
    [-1, 0],
    [0, 1],
    [0, -1],
  ]) {
    const along = dx * angleX + dy * angleY;
    const across = Math.abs(dx * angleY - dy * angleX);
    if (along > 0.345 && along < 0.41 && across < (small ? 0.014 : 0.008)) {
      sample.over(INK, 0.85);
    }
  }
}

function drawAdminGlyph(sample, x, y, size) {
  const small = size <= 24;
  const cx = 0.5;
  const cy = 0.49;
  const dx = x - cx;
  const dy = y - cy;
  const radius = Math.hypot(dx, dy);
  const angle = Math.atan2(dy, dx);

  if (!small) {
    // Radar sweep: a fading wedge pointing up-right.
    const sweepStart = -Math.PI * 0.62;
    const sweepEnd = -Math.PI * 0.1;
    if (radius < 0.39 && angle > sweepStart && angle < sweepEnd) {
      const fade = (angle - sweepStart) / (sweepEnd - sweepStart);
      sample.over(INK, 0.12 + 0.22 * fade);
    }
    // Leading edge of the sweep.
    const edgeDistance = Math.abs(angle - sweepEnd);
    if (radius < 0.39 && edgeDistance < 0.03) {
      sample.over(INK, 0.9);
    }
  }
  // Range rings.
  const rings = small ? [0.18, 0.38] : [0.145, 0.255, 0.365];
  for (const ring of rings) {
    if (Math.abs(radius - ring) < (small ? 0.024 : 0.011)) {
      sample.over(INK, small ? 0.7 : 0.55);
    }
  }
  // Blips.
  const blips = small
    ? [
        [0.62, 0.34, 0.052],
        [0.64, 0.61, 0.046],
        [0.38, 0.62, 0.042],
      ]
    : [
        [0.61, 0.35, 0.036],
        [0.63, 0.57, 0.028],
        [0.4, 0.62, 0.024],
      ];
  for (const [bx, by, br] of blips) {
    if (inCircle(x, y, bx, by, br)) sample.over(INK, 1);
  }
  // Centre dot.
  if (inCircle(x, y, cx, cy, small ? 0.038 : 0.026)) {
    sample.over(INK, 1);
  }
}

// ---------------------------------------------------------------- raster

function renderIconPixels(kind, size) {
  const samples = size <= 128 ? 4 : size <= 256 ? 3 : 2;
  const palette = PALETTES[kind];
  const pixels = Buffer.alloc(size * size * 4);
  const drawGlyph = (sample, x, y) =>
    kind === "desktop"
      ? drawDesktopGlyph(sample, x, y, size, palette)
      : drawAdminGlyph(sample, x, y, size);

  for (let py = 0; py < size; py += 1) {
    for (let px = 0; px < size; px += 1) {
      let r = 0;
      let g = 0;
      let b = 0;
      let a = 0;
      const sample = {
        over(color, alpha) {
          if (alpha <= 0) return;
          const nextAlpha = alpha + a * (1 - alpha);
          r = (color[0] / 255) * alpha + r * (1 - alpha);
          g = (color[1] / 255) * alpha + g * (1 - alpha);
          b = (color[2] / 255) * alpha + b * (1 - alpha);
          a = nextAlpha;
        },
      };
      for (let sy = 0; sy < samples; sy += 1) {
        for (let sx = 0; sx < samples; sx += 1) {
          const x = (px + (sx + 0.5) / samples) / size;
          const y = (py + (sy + 0.5) / samples) / size;
          drawBackground(sample, x, y, palette);
          drawGlyph(sample, x, y);
        }
      }
      const count = samples * samples;
      const alpha = a / count;
      const offset = (py * size + px) * 4;
      if (alpha > 0.0001) {
        pixels[offset] = Math.round(clamp01(r / a) * 255);
        pixels[offset + 1] = Math.round(clamp01(g / a) * 255);
        pixels[offset + 2] = Math.round(clamp01(b / a) * 255);
        pixels[offset + 3] = Math.round(clamp01(alpha) * 255);
      }
    }
  }
  return pixels;
}

function renderIcon(kind, size) {
  return encodePng(size, size, renderIconPixels(kind, size));
}

// ---------------------------------------------------------------- containers

function encodeIco(images) {
  const count = images.length;
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(count, 4);
  const entries = Buffer.alloc(16 * count);
  let offset = 6 + 16 * count;
  images.forEach(({ size, png }, index) => {
    const entry = index * 16;
    entries[entry] = size >= 256 ? 0 : size;
    entries[entry + 1] = size >= 256 ? 0 : size;
    entries[entry + 2] = 0;
    entries[entry + 3] = 0;
    entries.writeUInt16LE(1, entry + 4);
    entries.writeUInt16LE(32, entry + 6);
    entries.writeUInt32LE(png.length, entry + 8);
    entries.writeUInt32LE(offset, entry + 12);
    offset += png.length;
  });
  return Buffer.concat([header, entries, ...images.map((image) => image.png)]);
}

const ICNS_TYPES = [
  [32, "ic11"],
  [64, "ic12"],
  [128, "ic07"],
  [256, "ic13"],
  [256, "ic08"],
  [512, "ic14"],
  [512, "ic09"],
  [1024, "ic10"],
];

function encodeIcns(bySize) {
  const chunks = ICNS_TYPES.map(([size, type]) => {
    const png = bySize.get(size);
    if (!png) throw new Error(`missing ${size}px image for ${type}`);
    const header = Buffer.alloc(8);
    header.write(type, 0, "ascii");
    header.writeUInt32BE(8 + png.length, 4);
    return Buffer.concat([header, png]);
  });
  const body = Buffer.concat(chunks);
  const header = Buffer.alloc(8);
  header.write("icns", 0, "ascii");
  header.writeUInt32BE(8 + body.length, 4);
  return Buffer.concat([header, body]);
}

// ---------------------------------------------------------------- main

mkdirSync(OUT_DIR, { recursive: true });

for (const kind of ["desktop", "admin"]) {
  const bySize = new Map();
  for (const size of SIZES) {
    const png = renderIcon(kind, size);
    bySize.set(size, png);
    writeFileSync(join(OUT_DIR, `twg-${kind}-${size}.png`), png);
    process.stdout.write(`twg-${kind}-${size}.png\n`);
  }
  writeFileSync(join(OUT_DIR, `twg-${kind}.png`), bySize.get(1024));
  // Raw RGBA used by the window icon at runtime (winit::window::Icon).
  const uiSize = 256;
  writeFileSync(
    join(OUT_DIR, `twg-${kind}-ui.rgba`),
    renderIconPixels(kind, uiSize),
  );
  writeFileSync(
    join(OUT_DIR, `twg-${kind}.ico`),
    encodeIco(
      [16, 24, 32, 48, 64, 128, 256].map((size) => ({
        size,
        png: bySize.get(size),
      })),
    ),
  );
  writeFileSync(join(OUT_DIR, `twg-${kind}.icns`), encodeIcns(bySize));
  process.stdout.write(`twg-${kind}.ico\ntwg-${kind}.icns\n`);
}
