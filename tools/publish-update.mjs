#!/usr/bin/env node
// Signs and lays out an auto-update release for twg-desktop.
//
// First-time key generation (commit the public key, keep the private key safe):
//   node tools/publish-update.mjs --keygen
//
// Publish a release after building the platform artifacts:
//   node tools/publish-update.mjs \
//     --version 0.2.0 --build 42 --notes "Perbaikan ..." \
//     --windows dist/windows/twg-desktop.exe \
//     --linux dist/linux/TacticalGame3D.AppImage \
//     --macos-arm dist/TacticalGame3D-macos.zip \
//     --out dist/updates/twg-desktop

import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { execFileSync } from "node:child_process";
import { createHash, createPrivateKey, createPublicKey, generateKeyPairSync, sign } from "node:crypto";
import { basename, dirname, join } from "node:path";

const args = parseArgs(process.argv.slice(2));
const DEFAULT_BASE_URL = "http://103.179.218.76:8122/updates/twg-desktop";
const PUBLIC_KEY_FILE = "assets/update-public-key.txt";
const PRIVATE_KEY_FILE =
  args["private-key"] ??
  process.env.TWG_UPDATE_SIGNING_KEY ??
  "dist/keys/twg-update-signing-key.pem";

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const value = argv[index];
    if (!value.startsWith("--")) continue;
    const [key, inline] = value.slice(2).split("=", 2);
    if (inline !== undefined) {
      parsed[key] = inline;
    } else if (argv[index + 1] && !argv[index + 1].startsWith("--")) {
      parsed[key] = argv[index + 1];
      index += 1;
    } else {
      parsed[key] = true;
    }
  }
  return parsed;
}

function fail(message) {
  console.error(`error: ${message}`);
  process.exit(1);
}

function rawPublicKey(keyObject) {
  const jwk = keyObject.export({ format: "jwk" });
  if (!jwk.x) fail("private key is not an Ed25519 key");
  return Buffer.from(jwk.x, "base64url");
}

function keygen() {
  const publicPath = PUBLIC_KEY_FILE;
  const privatePath = PRIVATE_KEY_FILE;
  if (!args.force && (existsSync(privatePath) || existsSync(publicPath))) {
    fail(
      `${privatePath} atau ${publicPath} sudah ada; pakai --force hanya jika memang ingin mengganti kunci (semua client lama harus dibangun ulang).`,
    );
  }
  const { privateKey, publicKey } = generateKeyPairSync("ed25519");
  mkdirSync(dirname(privatePath), { recursive: true });
  mkdirSync(dirname(publicPath), { recursive: true });
  writeFileSync(privatePath, privateKey.export({ format: "pem", type: "pkcs8" }));
  try {
    chmodSync(privatePath, 0o600);
  } catch {
    // Windows may ignore POSIX permissions.
  }
  const raw = rawPublicKey(publicKey);
  writeFileSync(publicPath, `${raw.toString("base64")}\n`);
  console.log(`private key: ${privatePath}`);
  console.log(`public key : ${publicPath} (${raw.toString("base64")})`);
  console.log("Simpan private key di tempat aman/secret CI. Jangan commit.");
}

function workspaceVersion() {
  const text = readFileSync("Cargo.toml", "utf8");
  const match = text.match(/^version = "([^"]+)"/m);
  return match ? match[1] : "0.1.0";
}

function gitBuildNumber() {
  try {
    const count = execFileSync("git", ["rev-list", "--count", "HEAD"], {
      encoding: "utf8",
    }).trim();
    return Number.parseInt(count, 10) || 0;
  } catch {
    return 0;
  }
}

function sha256(path) {
  const hash = createHash("sha256");
  hash.update(readFileSync(path));
  return hash.digest("hex");
}

function publish() {
  const version = args.version ?? workspaceVersion();
  const build = Number.parseInt(args.build ?? String(gitBuildNumber()), 10);
  if (!Number.isFinite(build) || build <= 0) {
    fail("--build harus angka > 0 (mis. nomor build CI)");
  }
  const notes = args.notes ?? "";
  const baseUrl = (args["base-url"] ?? DEFAULT_BASE_URL).replace(/\/+$/, "");
  const outDir = args.out ?? "dist/updates/twg-desktop";
  const minSupported = Number.parseInt(args["min-supported-build"] ?? "0", 10) || 0;
  const privateKeyPath = PRIVATE_KEY_FILE;

  if (!existsSync(privateKeyPath)) {
    fail(`private key tidak ditemukan di ${privateKeyPath}; jalankan --keygen dulu`);
  }
  if (!existsSync(PUBLIC_KEY_FILE)) {
    fail(`public key tidak ditemukan di ${PUBLIC_KEY_FILE}; jalankan --keygen dulu`);
  }
  const privateKey = createPrivateKey(readFileSync(privateKeyPath));
  const derivedPublic = rawPublicKey(createPublicKey(privateKey)).toString("base64");
  const committedPublic = readFileSync(PUBLIC_KEY_FILE, "utf8").trim();
  if (derivedPublic !== committedPublic) {
    fail(
      `private key tidak cocok dengan ${PUBLIC_KEY_FILE}; update akan ditolak client`,
    );
  }

  const versionDir = join(outDir, version);
  mkdirSync(versionDir, { recursive: true });

  const previousManifest = join(outDir, "stable.json");
  if (!args.force && existsSync(previousManifest)) {
    try {
      const previous = JSON.parse(readFileSync(previousManifest, "utf8"));
      if (Number(previous.build) >= build) {
        fail(
          `build ${build} harus lebih besar dari build terakhir (${previous.build}); pakai --build baru atau --force`,
        );
      }
    } catch (error) {
      if (error.code !== "ENOENT") fail(`manifest sebelumnya tidak terbaca: ${error}`);
    }
  }

  const platforms = {};
  const artifacts = [
    ["windows", args.windows, "windows-x86_64", "exe"],
    ["linux", args.linux, "linux-x86_64-binary", "binary"],
    [
      "linux-appimage",
      args["linux-appimage"],
      "linux-x86_64-appimage",
      "appimage",
    ],
    ["macos-arm", args["macos-arm"], "macos-aarch64", "app-zip"],
    ["macos-x64", args["macos-x64"], "macos-x86_64", "app-zip"],
  ];
  for (const [flag, source, platform, kind] of artifacts) {
    if (!source) continue;
    if (!existsSync(source)) fail(`artefak tidak ditemukan: ${source}`);
    if (
      kind === "binary" &&
      (source.toLowerCase().endsWith(".tar.gz") ||
        source.toLowerCase().endsWith(".zip"))
    ) {
      fail(
        `--${flag} harus binary mentah (mis. target/release/twg-desktop), bukan arsip`,
      );
    }
    const sourceName = basename(source);
    const extension =
      kind === "exe"
        ? ".exe"
        : kind === "appimage"
          ? ".AppImage"
          : kind === "binary"
            ? ""
            : ".app.zip";
    const filename = `twg-desktop-${version}-${platform}${extension}`;
    const target = join(versionDir, filename);
    copyFileSync(source, target);
    platforms[platform] = {
      kind,
      url: `${baseUrl}/${version}/${filename}`,
      size: readFileSync(target).length,
      sha256: sha256(target),
    };
    console.log(`${platform}: ${target} (${sourceName})`);
  }
  if (Object.keys(platforms).length === 0) {
    fail(
      "tidak ada artefak; isi --windows / --linux / --linux-appimage / --macos-arm / --macos-x64",
    );
  }

  const manifest = {
    schema: 1,
    channel: "stable",
    version,
    build,
    min_supported_build: minSupported,
    mandatory: true,
    published_at: new Date().toISOString(),
    notes,
    platforms,
  };
  const manifestBytes = Buffer.from(`${JSON.stringify(manifest, null, 2)}\n`);
  const manifestPath = join(outDir, "stable.json");
  writeFileSync(manifestPath, manifestBytes);
  writeFileSync(
    join(outDir, "stable.json.sig"),
    `${sign(null, manifestBytes, privateKey).toString("base64")}\n`,
  );
  console.log(`manifest: ${manifestPath}`);
  console.log(`signature: ${manifestPath}.sig`);
  console.log("");
  console.log("Upload ke VPS (manifest terakhir otomatis menimpa):");
  console.log(
    `  scp -r ${outDir}/* user@103.179.218.76:/var/lib/twg/updates/twg-desktop/`,
  );
}

if (args.keygen) {
  keygen();
} else {
  publish();
}
