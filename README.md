# Easy Dictate

[![GitHub release](https://img.shields.io/github/v/release/RuKapSan/easy-dictate)](https://github.com/RuKapSan/easy-dictate/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

**[Русская версия](README.ru.md)**

Desktop voice-to-text application with AI transcription. Press hotkey, speak, text appears in any app.

![Easy Dictate Main](assets/main.png)

## Features

- **Push-to-talk dictation** — hold hotkey and speak
- **3 transcription providers** — OpenAI Whisper, Groq (free), ElevenLabs (realtime streaming)
- **Auto-translation** — automatic translation to target language after transcription
- **Custom instructions** — post-process text via LLM (summarize, format, etc.)
- **Custom vocabulary** — auto-correct technical terms (Groq, Tauri, WebSocket, etc.)
- **Transcription history** — browse, copy, and manage past transcriptions
- **Typing simulation** — text is typed into active window as if from keyboard
- **Background operation** — system tray, auto-start, global hotkeys
- **Auto-updates** — automatic updates from GitHub releases

![Easy Dictate Settings](assets/settings.png)

## Installation

### Pre-built binaries

Download the latest version for your OS:

- **Windows**: `.msi` or `.exe` installer
- **macOS**: `.dmg` (Intel and Apple Silicon)
- **Linux**: `.deb` or `.AppImage`

[Download Latest Release](https://github.com/RuKapSan/easy-dictate/releases/latest)

### Build from source

```bash
git clone https://github.com/RuKapSan/easy-dictate.git
cd easy-dictate/src-tauri
cargo tauri build
```

Requirements: Rust 1.77+, Node.js 18+

## Quick Start

1. Install the application
2. Select a provider (Groq is free)
3. Enter your API key
4. Press the hotkey (default: `Ctrl+Shift+Space`)
5. Speak — text will appear in the active window

## Hotkeys

| Hotkey | Action |
|--------|--------|
| Main | Record and transcribe |
| With translation | Record + force translation |
| Toggle translation | Toggle auto-translate on/off |

All hotkeys are configurable in Settings.

## Providers

| Provider | Speed | Price | Features |
|----------|-------|-------|----------|
| **Groq** | Fast | Free | Whisper Large v3 |
| **OpenAI** | Medium | Paid | GPT-4o Transcribe |
| **ElevenLabs** | Realtime | Paid | Text streaming during speech |

## Tech Stack

- **Tauri v2** + Rust (backend)
- **HTML/CSS/JS** (frontend, no frameworks)
- **cpal** (audio capture)

## License

MIT

## Links

- [Releases](https://github.com/RuKapSan/easy-dictate/releases)
- [Issues](https://github.com/RuKapSan/easy-dictate/issues)
