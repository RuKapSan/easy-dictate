# Easy Dictate E2E Test Runner
# PowerShell script for Windows

param(
    [switch]$Debug,
    [switch]$SetupOnly,
    [switch]$Report,
    [switch]$Clean
)

$ErrorActionPreference = "Stop"
$TestDir = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $TestDir

Write-Host "=================================" -ForegroundColor Cyan
Write-Host " Easy Dictate E2E Test Runner" -ForegroundColor Cyan
Write-Host "=================================" -ForegroundColor Cyan
Write-Host ""

# Clean mode
if ($Clean) {
    Write-Host "[*] Cleaning test artifacts..." -ForegroundColor Yellow
    Remove-Item -Recurse -Force "screenshots", "logs", "allure-results", "allure-report", "dist" -ErrorAction SilentlyContinue
    Write-Host "[+] Clean complete!" -ForegroundColor Green
    exit 0
}

# Check prerequisites
Write-Host "[*] Checking prerequisites..." -ForegroundColor Yellow

# Check tauri-driver
$tauriDriver = Get-Command "tauri-driver" -ErrorAction SilentlyContinue
if (-not $tauriDriver) {
    Write-Host "[-] tauri-driver not found!" -ForegroundColor Red
    Write-Host "    Install with: cargo install tauri-driver" -ForegroundColor Yellow
    exit 1
}
Write-Host "[+] tauri-driver found" -ForegroundColor Green

# Check if app is built
$appPath = Join-Path $TestDir "..\..\src-tauri\target\debug\app.exe"
if (-not (Test-Path $appPath)) {
    Write-Host "[-] App not built!" -ForegroundColor Red
    Write-Host "    Build with: cargo tauri build --debug" -ForegroundColor Yellow

    $response = Read-Host "Build now? (y/n)"
    if ($response -eq "y") {
        Write-Host "[*] Building app..." -ForegroundColor Yellow
        Set-Location (Join-Path $TestDir "../..")
        cargo tauri build --debug
        Set-Location $TestDir
    } else {
        exit 1
    }
}
Write-Host "[+] App built: $appPath" -ForegroundColor Green

# Check node_modules
if (-not (Test-Path "node_modules")) {
    Write-Host "[*] Installing dependencies..." -ForegroundColor Yellow
    npm install
}
Write-Host "[+] Dependencies installed" -ForegroundColor Green

# Setup audio files
if ($SetupOnly -or -not (Test-Path "audio-mocks\short_phrase.wav")) {
    Write-Host "[*] Generating test audio files..." -ForegroundColor Yellow
    npx ts-node scripts/setup-audio.ts
    if ($SetupOnly) {
        Write-Host "[+] Setup complete!" -ForegroundColor Green
        exit 0
    }
}

# Create output directories
New-Item -ItemType Directory -Force -Path "screenshots", "logs" | Out-Null

Write-Host ""
Write-Host "[*] Starting tests..." -ForegroundColor Yellow
Write-Host ""

# Run tests
if ($Debug) {
    npx wdio run wdio.conf.ts --logLevel=debug
} else {
    npx wdio run wdio.conf.ts
}

$exitCode = $LASTEXITCODE

# Generate report
if ($Report -or $exitCode -ne 0) {
    Write-Host ""
    Write-Host "[*] Generating Allure report..." -ForegroundColor Yellow
    npx allure generate allure-results --clean -o allure-report
    npx allure open allure-report
}

Write-Host ""
if ($exitCode -eq 0) {
    Write-Host "=================================" -ForegroundColor Green
    Write-Host " All tests passed!" -ForegroundColor Green
    Write-Host "=================================" -ForegroundColor Green
} else {
    Write-Host "=================================" -ForegroundColor Red
    Write-Host " Some tests failed!" -ForegroundColor Red
    Write-Host " Check screenshots/ and logs/" -ForegroundColor Red
    Write-Host "=================================" -ForegroundColor Red
}

exit $exitCode
