# Claude Code Memory - Easy Dictate Project

This document contains important project context, conventions, and guidelines for Claude Code when working on Easy Dictate.

## Project Overview

**Easy Dictate** is a desktop voice-to-text application built with Tauri v2 and Rust. It provides push-to-talk dictation with AI-powered transcription and optional translation/custom instructions via LLMs.

### Tech Stack
- **Backend**: Rust, Tauri v2.8+
- **Frontend**: Static HTML/CSS/JavaScript (no framework)
- **Audio**: cpal (cross-platform audio capture)
- **AI Services**: OpenAI Whisper, Groq, ElevenLabs Conversational AI (streaming)
- **Testing**: WebdriverIO + tauri-driver for E2E tests

---

## Versioning Strategy

### Version Format: Semantic Versioning (SemVer)
```
MAJOR.MINOR.PATCH (e.g., 1.2.3)
```

- **MAJOR**: Breaking changes, incompatible API changes
- **MINOR**: New features, backwards-compatible
- **PATCH**: Bug fixes, backwards-compatible

### Current Version
**v0.1.0** - Initial development phase

### Version Locations (must be updated together)
1. **`src-tauri/Cargo.toml`** - line 3: `version = "0.1.0"`
2. **`src-tauri/tauri.conf.json`** - line 4: `"version": "0.1.0"`

**CRITICAL**: When bumping version, update BOTH files to the same version.

### Release Process
1. Decide on new version number based on changes
2. Update version in both `Cargo.toml` and `tauri.conf.json`
3. Commit version bump: `git commit -m "chore: bump version to vX.Y.Z"`
4. Create and push git tag: `git tag vX.Y.Z && git push origin vX.Y.Z`
5. GitHub Actions will automatically:
   - Run E2E tests (Windows only)
   - Build for 4 platforms (Windows x64, Linux x64, macOS Intel, macOS ARM)
   - Generate update manifest (`latest.json`)
   - Create GitHub Release with installers
   - Publish release with changelog

### Pre-release Versions
For beta/alpha releases, use:
```
v1.0.0-beta.1
v1.0.0-alpha.2
```

### Changelog Generation
Automatically generated from git commits between tags.
Use conventional commits for better changelogs:
- `feat:` - New features
- `fix:` - Bug fixes
- `perf:` - Performance improvements
- `docs:` - Documentation changes
- `chore:` - Maintenance tasks
- `test:` - Test updates

---

## CI/CD Workflows

### Build & E2E Tests (`.github/workflows/e2e-tests.yml`)
- **Trigger**: Push to `master` or `feat/*` branches, PRs to master
- **Jobs**:
  - Matrix build: Windows x64, Linux x64, macOS Intel, macOS ARM
  - E2E tests: Windows only (has microphone access)
  - Caching: Rust deps, cargo binaries, npm modules
- **Artifacts**: Build outputs, E2E screenshots/videos/logs

### Release (`.github/workflows/release.yml`)
- **Trigger**: Git tags matching `v*`
- **Jobs**:
  1. **Test**: Run E2E tests on Windows (blocks release if tests fail)
  2. **Build**: Build for all 4 platforms in parallel
  3. **Release**: Create GitHub Release with:
     - Changelog from git commits
     - `latest.json` update manifest
     - Platform installers (.msi, .exe, .deb, .AppImage, .dmg)

---

## Auto-Update System

### How It Works
1. App checks GitHub Releases on startup (if `auto_update` setting is enabled)
2. Downloads `latest.json` from latest release
3. Compares current version with available version
4. Auto-downloads and installs update in background
5. Update applies on next app restart

### Update Manifest Format (`latest.json`)
```json
{
  "version": "1.0.0",
  "date": "2025-01-01T00:00:00Z",
  "platforms": {
    "windows-x86_64": {
      "signature": "",
      "url": "https://github.com/RuKapSan/easy-dictate/releases/download/v1.0.0/Easy_Dictate_1.0.0_x64.msi"
    },
    "linux-x86_64": { ... },
    "darwin-x86_64": { ... },
    "darwin-aarch64": { ... }
  }
}
```

### User Control
- **Setting**: `auto_update` (bool, default: `true`)
- **Location**: Settings UI → Auto-update toggle
- **Behavior**: When disabled, app logs "Auto-update disabled in settings"

### Dependencies
- `tauri-plugin-updater = "2"`
- Updater endpoint: `https://github.com/RuKapSan/easy-dictate/releases/latest/download/latest.json`

---

## Testing Infrastructure

### E2E Test Framework
- **Tool**: WebdriverIO v9 + tauri-driver
- **Location**: `tests/e2e/`
- **Config**: `tests/e2e/wdio.conf.ts`

### Test Modes
1. **Mock Provider**: Use `provider: "mock"` for tests without API keys
   - Returns: `"Mock transcription result for E2E testing"`
   - Delay: 500ms simulated processing

2. **Test Commands** (only available in test mode):
   - `inject_test_audio(audioData)` - Inject WAV audio
   - `simulate_hotkey_press()` - Start recording
   - `simulate_hotkey_release()` - Stop recording & trigger transcription
   - `get_test_state()` - Get app internal state
   - `show_main_window()` - Force show main window

### Running Tests Locally
```bash
# Terminal 1: Build release binary
cd src-tauri
cargo tauri build

# Terminal 2: Run tests
cd tests/e2e
npm test
```

### Test Artifacts
- **Screenshots**: `tests/e2e/screenshots/`
- **Videos**: `tests/e2e/videos/` (wdio-video-reporter)
- **Logs**: `tests/e2e/logs/`
- **CI Artifacts**: Downloaded to `tests/e2e/ci-artifacts/` (gitignored)

### Current Test Status
- **Total**: 21 tests
- **Passing**: 18 locally
- **Skipped**: 3 (flaky PowerShell hotkey, console error detection)
- **CI**: Some tests fail due to missing microphone/API keys

---

## Project Structure

```
easy-dictate/
├── src-tauri/              # Rust backend
│   ├── src/
│   │   ├── lib.rs          # Main app entry, plugin setup
│   │   ├── settings.rs     # AppSettings struct
│   │   ├── core/           # Core app logic
│   │   │   ├── commands.rs # Tauri commands
│   │   │   ├── hotkey.rs   # Global hotkey handling
│   │   │   ├── transcription.rs # Transcription service
│   │   │   └── state.rs    # App state management
│   │   ├── audio/          # Audio capture
│   │   ├── openai/         # OpenAI integration
│   │   ├── groq/           # Groq integration
│   │   └── elevenlabs/     # ElevenLabs streaming
│   ├── Cargo.toml          # Rust dependencies + VERSION
│   └── tauri.conf.json     # Tauri config + VERSION
├── frontend/               # Static frontend
│   ├── index.html          # Main window UI
│   └── overlay.html        # Transparent overlay
├── tests/e2e/              # E2E test suite
├── .github/workflows/      # CI/CD
│   ├── e2e-tests.yml       # Build + test workflow
│   └── release.yml         # Release workflow
└── claude.md               # This file - Claude Code memory
```

---

## Key Settings & Configurations

### AppSettings Fields (src-tauri/src/settings.rs)
```rust
pub struct AppSettings {
    pub provider: TranscriptionProvider,  // openai, groq, elevenlabs, mock
    pub llm_provider: LLMProvider,        // openai, groq
    pub api_key: String,
    pub groq_api_key: String,
    pub elevenlabs_api_key: String,
    pub model: String,                     // Default: "gpt-4o-transcribe"
    pub hotkey: String,                    // Default: "Ctrl+Shift+Space"
    pub simulate_typing: bool,             // Default: true
    pub copy_to_clipboard: bool,           // Default: true
    pub auto_start: bool,                  // Default: false
    pub auto_update: bool,                 // Default: true
    pub use_streaming: bool,               // Default: true (ElevenLabs)
    pub auto_translate: bool,              // Default: false
    pub target_language: String,           // Default: "English"
    pub use_custom_instructions: bool,     // Default: false
    pub custom_instructions: String,
}
```

### Tauri Permissions (src-tauri/capabilities/default.json)
- Core: window management, logging
- Plugins: global-shortcut, autostart, clipboard-manager, updater
- Custom: elevenlabs_streaming_*, show_overlay_no_focus, test-mode commands

---

## Common Tasks for Claude Code

### Adding a New Setting
1. Add field to `AppSettings` struct in `src-tauri/src/settings.rs`
2. Add default value in `Default` impl
3. Update frontend UI to display/edit the setting
4. Use setting in relevant backend code

### Creating a Release
1. Update version in `Cargo.toml` and `tauri.conf.json`
2. Commit: `git commit -m "chore: bump version to vX.Y.Z"`
3. Tag: `git tag vX.Y.Z && git push origin vX.Y.Z`
4. Wait for GitHub Actions to complete
5. Verify release on https://github.com/RuKapSan/easy-dictate/releases

### Adding a New Tauri Command
1. Define command in `src-tauri/src/core/commands.rs`
2. Register in `invoke_handler!` in `src-tauri/src/lib.rs`
3. Add ACL permission in `src-tauri/capabilities/default.json`
4. Call from frontend: `window.__TAURI__.core.invoke('command_name', args)`

### Debugging E2E Tests
1. Check screenshots in `tests/e2e/screenshots/`
2. Watch videos in `tests/e2e/videos/`
3. Review logs in `tests/e2e/logs/`
4. For CI failures: Download artifacts from GitHub Actions

---

## Important Notes & Gotchas

### Tauri v2 Specifics
- **ACL System**: All commands need explicit permissions in `capabilities/*.json`
- **camelCase in JS**: Tauri v2 converts snake_case Rust args to camelCase in JavaScript
- **Error Handling**: Errors can be strings or objects, handle both
- **Window Management**: Windows default to hidden, must call `.show()`

### Cross-Platform Considerations
- **Linux**: Requires system dependencies (webkit2gtk, libappindicator3, etc.)
- **macOS**: Two targets needed (Intel x86_64, ARM aarch64)
- **Windows**: PowerShell vs Bash - always use `shell: bash` in CI

### Git Worktrees
- Claude Code may create temporary worktrees (e.g., "upbeat-bhabha")
- Clean up with: `git worktree list` → `git worktree remove <name>`

### Audio Capture
- Requires microphone permission
- CI environments have no microphone → use Mock provider or inject_test_audio
- Audio format: 16kHz mono WAV for transcription

---

## Contact & Resources

- **Repository**: https://github.com/RuKapSan/easy-dictate
- **Releases**: https://github.com/RuKapSan/easy-dictate/releases
- **CI/CD**: https://github.com/RuKapSan/easy-dictate/actions
- **Tauri Docs**: https://v2.tauri.app/
- **WebdriverIO**: https://webdriver.io/

---

**Last Updated**: 2025-12-09 by Claude Code
**Version**: Initial documentation for v0.1.0
