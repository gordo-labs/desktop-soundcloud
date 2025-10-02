param(
  [string]$CertificatePath = $env:SIGNING_CERTIFICATE_PATH,
  [string]$CertificatePassword = $env:SIGNING_CERTIFICATE_PASSWORD,
  [string]$TimestampUrl = $env:TIMESTAMP_URL
)

$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$projectRoot = Resolve-Path "$scriptDir/.."

Push-Location $projectRoot

$vitestCacheDir = Join-Path $projectRoot '.cache/vitest'
New-Item -ItemType Directory -Path $vitestCacheDir -Force | Out-Null
$env:VITEST_CACHE_DIR = $vitestCacheDir

npm ci
npm run test
npm run build
cargo test --workspace --manifest-path src-tauri/Cargo.toml
cargo tauri build --bundles msi @args

$bundleDir = Join-Path $projectRoot 'src-tauri/target/release/bundle/msi'
if (Test-Path $bundleDir -PathType Container) {
  $msi = Get-ChildItem -Path $bundleDir -Filter '*.msi' | Sort-Object LastWriteTime -Descending | Select-Object -First 1
  if ($msi -and $CertificatePath) {
    if (-not $TimestampUrl) {
      $TimestampUrl = 'http://timestamp.digicert.com'
    }
    & signtool sign /fd SHA256 /f $CertificatePath /p $CertificatePassword /tr $TimestampUrl /td SHA256 $msi.FullName
  }
}

Pop-Location
