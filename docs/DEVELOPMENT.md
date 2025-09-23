# Development Guide

## Архитектура проекта

```
easy-dictate/
├── src-tauri/          # Rust backend
│   ├── src/
│   │   ├── core/       # Основные модули
│   │   │   ├── commands.rs      # Tauri команды
│   │   │   ├── state.rs         # Управление состоянием
│   │   │   ├── hotkey.rs        # Горячие клавиши
│   │   │   ├── transcription.rs # Логика транскрибации
│   │   │   ├── events.rs        # События
│   │   │   └── tray.rs          # Системный трей
│   │   ├── audio.rs              # Запись аудио
│   │   ├── input.rs              # Эмуляция ввода
│   │   ├── settings.rs           # Настройки
│   │   ├── openai.rs             # OpenAI API
│   │   ├── groq.rs               # Groq Whisper API
│   │   ├── groq_llm.rs           # Groq LLM API
│   │   ├── lib.rs                # Точка входа библиотеки
│   │   └── main.rs               # Точка входа приложения
│   ├── Cargo.toml                # Зависимости Rust
│   └── tauri.conf.json           # Конфигурация Tauri
├── frontend/           # Frontend
│   ├── index.html      # UI настроек
│   └── main.js         # Логика frontend
└── docs/               # Документация
```

## Настройка окружения

### Требования

- Rust 1.77.2+
- Node.js 18+
- Tauri CLI 2.0+

### Установка инструментов

```bash
# Установка Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Установка Tauri CLI
cargo install tauri-cli

# Проверка установки
cargo --version
cargo tauri --version
```

## Запуск в режиме разработки

```bash
# Основной способ
cargo tauri dev

# С логированием
RUST_LOG=debug cargo tauri dev

# С определённым портом для dev сервера
cargo tauri dev -- --port 3000
```

## Архитектура кода

### State Management

Приложение использует `AppState` для управления глобальным состоянием:

```rust
pub struct AppState {
    pub recording: Arc<Mutex<bool>>,
    pub audio_data: Arc<Mutex<Vec<u8>>>,
    pub sample_rate: Arc<Mutex<u32>>,
    pub settings_store: Arc<SettingsStore>,
    pub current_settings: Arc<Mutex<Settings>>,
}
```

### Event System

События используются для коммуникации между backend и frontend:

```rust
pub enum StatusPhase {
    Recording,
    Processing,
    Transcribing,
    Enhancing,
    Complete,
    Error,
}

pub async fn emit_status(
    app_handle: &AppHandle,
    phase: StatusPhase,
    message: String
)
```

### Command Pattern

Команды Tauri следуют паттерну:

```rust
#[tauri::command]
pub async fn command_name(
    state: State<'_, AppState>,
    param: Type
) -> Result<ReturnType, String> {
    // Логика команды
}
```

## Добавление новых функций

### 1. Добавление новой команды

**Шаг 1:** Создайте функцию в `src/core/commands.rs`:

```rust
#[tauri::command]
pub async fn my_new_command(
    state: State<'_, AppState>,
    param: String
) -> Result<String, String> {
    // Ваша логика
    Ok("Success".to_string())
}
```

**Шаг 2:** Зарегистрируйте команду в `src/lib.rs`:

```rust
.invoke_handler(tauri::generate_handler![
    // ... existing commands
    commands::my_new_command,
])
```

**Шаг 3:** Используйте в frontend:

```javascript
const result = await invoke('my_new_command', { param: 'value' });
```

### 2. Добавление нового провайдера

**Шаг 1:** Создайте модуль провайдера:

```rust
// src/my_provider.rs
use anyhow::Result;

pub struct MyProvider {
    api_key: String,
}

impl MyProvider {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }

    pub async fn transcribe(&self, audio: Vec<u8>) -> Result<String> {
        // Логика транскрибации
    }
}
```

**Шаг 2:** Добавьте в `Settings`:

```rust
// src/settings.rs
pub enum ApiProvider {
    OpenAI,
    Groq,
    MyProvider, // Новый провайдер
}
```

**Шаг 3:** Интегрируйте в `transcription.rs`:

```rust
match settings.api_provider {
    ApiProvider::MyProvider => {
        let provider = MyProvider::new(api_key);
        provider.transcribe(audio_data).await?
    }
    // ...
}
```

### 3. Добавление нового события

**Шаг 1:** Определите структуру события:

```rust
#[derive(Clone, serde::Serialize)]
struct MyEvent {
    field1: String,
    field2: i32,
}
```

**Шаг 2:** Emit события:

```rust
app_handle.emit("my-event", MyEvent {
    field1: "value".to_string(),
    field2: 42,
})?;
```

**Шаг 3:** Слушайте в frontend:

```javascript
listen('my-event', (event) => {
    console.log(event.payload.field1, event.payload.field2);
});
```

## Тестирование

### Unit тесты

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_my_function() {
        let result = my_function();
        assert_eq!(result, expected);
    }
}
```

Запуск тестов:

```bash
cargo test
```

### Integration тесты

Создайте файл в `tests/`:

```rust
// tests/integration_test.rs
#[test]
fn test_full_flow() {
    // Тест полного флоу
}
```

## Отладка

### Логирование

Используйте макросы `log`:

```rust
use log::{debug, info, warn, error};

debug!("Debug message");
info!("Info message");
warn!("Warning message");
error!("Error message");
```

### DevTools

В режиме разработки доступны Chrome DevTools:

1. Запустите приложение: `cargo tauri dev`
2. Откройте DevTools: `Ctrl+Shift+I` (или `Cmd+Option+I` на macOS)

### Отладка Rust кода

**VS Code:**

`.vscode/launch.json`:
```json
{
    "version": "0.2.0",
    "configurations": [
        {
            "type": "lldb",
            "request": "launch",
            "name": "Debug Tauri",
            "cargo": {
                "args": ["build", "--manifest-path=src-tauri/Cargo.toml"],
                "filter": {
                    "name": "app",
                    "kind": "bin"
                }
            },
            "args": [],
            "cwd": "${workspaceFolder}"
        }
    ]
}
```

## Сборка

### Development build

```bash
cargo tauri build --debug
```

### Production build

```bash
cargo tauri build
```

### Платформо-специфичные сборки

```bash
# Windows
cargo tauri build --target x86_64-pc-windows-msvc

# macOS
cargo tauri build --target x86_64-apple-darwin
cargo tauri build --target aarch64-apple-darwin

# Linux
cargo tauri build --target x86_64-unknown-linux-gnu
```

## Оптимизация производительности

### 1. Оптимизация размера

В `Cargo.toml`:

```toml
[profile.release]
opt-level = "z"     # Optimize for size
lto = true          # Link Time Optimization
codegen-units = 1   # Single codegen unit
strip = true        # Strip symbols
```

### 2. Оптимизация аудио

- Используйте буферизацию для записи
- Конвертируйте аудио в нужный формат заранее
- Минимизируйте копирование данных

### 3. Оптимизация API вызовов

- Используйте connection pooling
- Реализуйте retry логику
- Кешируйте результаты где возможно

## Code Style

### Rust

Используйте `rustfmt`:

```bash
cargo fmt
```

Конфигурация в `rustfmt.toml`:

```toml
edition = "2021"
max_width = 100
use_small_heuristics = "Max"
```

### Linting

Используйте `clippy`:

```bash
cargo clippy -- -W clippy::all
```

## CI/CD

### GitHub Actions

`.github/workflows/ci.yml`:

```yaml
name: CI

on: [push, pull_request]

jobs:
  test:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
    steps:
      - uses: actions/checkout@v2
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - run: cargo test
      - run: cargo clippy
      - run: cargo fmt -- --check
```

## Полезные команды

```bash
# Очистка проекта
cargo clean

# Обновление зависимостей
cargo update

# Проверка безопасности
cargo audit

# Генерация документации
cargo doc --open

# Профилирование
cargo build --release
perf record -g target/release/app
perf report
```

## Ресурсы

- [Tauri Documentation](https://tauri.app/v1/guides/)
- [Rust Book](https://doc.rust-lang.org/book/)
- [OpenAI API Reference](https://platform.openai.com/docs/api-reference)
- [Groq API Documentation](https://console.groq.com/docs)
- [cpal Documentation](https://docs.rs/cpal/)