$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location $projectRoot
try {
    cargo build --package zhorten --lib --target-dir target/front --target wasm32-unknown-unknown --no-default-features --features hydrate
    New-Item -ItemType Directory -Force -Path target/site/pkg | Out-Null
    wasm-bindgen --target web --out-dir target/site/pkg --out-name zhorten target/front/wasm32-unknown-unknown/debug/zhorten.wasm
    Copy-Item -Force public/style.css target/site/style.css
    cargo build --package zhorten --bin zhorten --no-default-features --features ssr
}
finally {
    Pop-Location
}

