@echo off
REM Easy Dictate E2E Test Runner - Windows Batch
REM Usage: run-tests.bat [options]
REM Options:
REM   -debug    Run with debug logging
REM   -setup    Only setup audio files
REM   -report   Open report after tests
REM   -clean    Clean test artifacts

cd /d "%~dp0"

if "%1"=="-clean" (
    powershell -ExecutionPolicy Bypass -File run-tests.ps1 -Clean
) else if "%1"=="-setup" (
    powershell -ExecutionPolicy Bypass -File run-tests.ps1 -SetupOnly
) else if "%1"=="-debug" (
    powershell -ExecutionPolicy Bypass -File run-tests.ps1 -Debug
) else if "%1"=="-report" (
    powershell -ExecutionPolicy Bypass -File run-tests.ps1 -Report
) else (
    powershell -ExecutionPolicy Bypass -File run-tests.ps1
)
