# Builds the Windows release bundles for the desktop and online-admin apps.
# Run from the repository root:
#   powershell -ExecutionPolicy Bypass -File tools/package-windows.ps1
param(
    [string]$OutDir = "dist/windows"
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

cargo build --release -p twg-desktop -p twg-online-admin
if ($LASTEXITCODE -ne 0) {
    throw "cargo build failed"
}

Remove-Item -Recurse -Force $OutDir -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
Copy-Item "target/release/twg-desktop.exe" $OutDir
Copy-Item "target/release/twg-online-admin.exe" $OutDir
Copy-Item "assets/icons/twg-desktop.ico" $OutDir
Copy-Item "assets/icons/twg-admin.ico" $OutDir

@"
Tactical Game 3D + TWG Online Admin (Windows x64)
================================================

Jalankan twg-desktop.exe untuk aplikasi utama, atau
twg-online-admin.exe untuk mengelola sesi user online.

- Ikon sudah tertanam di masing-masing .exe; file .ico disertakan
  untuk membuat shortcut manual.
- Build release tidak membuka jendela console.
- Konfigurasi koneksi diatur dari halaman login/Pengaturan aplikasi.
"@ | Set-Content (Join-Path $OutDir "README.txt") -Encoding UTF8

$zip = Join-Path (Split-Path -Parent $OutDir) "twg-windows-x64.zip"
Compress-Archive -Path (Join-Path $OutDir "*") -DestinationPath $zip -Force
Write-Output "Bundle: $zip"
