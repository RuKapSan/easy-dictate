# Configuration Guide

## Файл конфигурации

Настройки приложения хранятся в JSON файле в директории конфигурации:

- **Windows**: `%APPDATA%\app\settings.json`
- **macOS**: `~/Library/Application Support/app/settings.json`
- **Linux**: `~/.config/app/settings.json`

## Структура настроек

```json
{
  "api_provider": "openai",
  "openai_api_key": "sk-...",
  "groq_api_key": "gsk_...",
  "language": "ru",
  "llm_prompt": "Исправь грамматику и пунктуацию",
  "hotkey": "Ctrl+Shift+Space",
  "transcription_model": "whisper-1",
  "llm_model": "gpt-3.5-turbo",
  "autostart_enabled": false
}
```

## Параметры конфигурации

### `api_provider`
**Тип:** `string`
**Значения:** `"openai"` | `"groq"`
**По умолчанию:** `"openai"`

Выбор провайдера API для транскрибации и обработки текста.

### `openai_api_key`
**Тип:** `string`
**Обязательный:** Да (если `api_provider` = `"openai"`)

API ключ от OpenAI. Получить можно на [platform.openai.com](https://platform.openai.com/api-keys).

### `groq_api_key`
**Тип:** `string`
**Обязательный:** Да (если `api_provider` = `"groq"`)

API ключ от Groq. Получить можно на [console.groq.com](https://console.groq.com/keys).

### `language`
**Тип:** `string`
**По умолчанию:** `"ru"`

Язык для транскрибации. Поддерживаемые значения:
- `"ru"` - Русский
- `"en"` - English
- `"es"` - Español
- `"fr"` - Français
- `"de"` - Deutsch
- `"it"` - Italiano
- `"pt"` - Português
- `"zh"` - 中文
- `"ja"` - 日本語
- `"ko"` - 한국어

### `llm_prompt`
**Тип:** `string` (опционально)
**По умолчанию:** `null`

Промпт для LLM модели. Если указан, текст после транскрибации будет обработан через GPT/Groq LLM.

Примеры промптов:
- "Исправь грамматику и пунктуацию"
- "Переформулируй в официальном стиле"
- "Сделай текст более кратким"
- "Переведи на английский"

### `hotkey`
**Тип:** `string`
**По умолчанию:** `"Ctrl+Shift+Space"`

Глобальная горячая клавиша для старта/остановки записи.

Формат: `"Modifier+Modifier+Key"`

Поддерживаемые модификаторы:
- `Ctrl` (или `Cmd` на macOS)
- `Shift`
- `Alt` (или `Option` на macOS)

Примеры:
- `"Ctrl+Shift+Space"`
- `"Alt+R"`
- `"Cmd+Shift+D"` (macOS)

### `transcription_model`
**Тип:** `string`
**По умолчанию:** `"whisper-1"` для OpenAI, `"whisper-large-v3"` для Groq

Модель для транскрибации.

**OpenAI модели:**
- `"whisper-1"` - стандартная модель Whisper

**Groq модели:**
- `"whisper-large-v3"` - последняя версия Whisper
- `"distil-whisper-large-v3-en"` - оптимизированная версия для английского

### `llm_model`
**Тип:** `string` (опционально)
**По умолчанию:** `"gpt-3.5-turbo"` для OpenAI, `"llama-3.1-8b-instant"` для Groq

Модель для обработки текста (если указан `llm_prompt`).

**OpenAI модели:**
- `"gpt-3.5-turbo"` - быстрая и экономичная
- `"gpt-4"` - более точная
- `"gpt-4-turbo"` - последняя версия GPT-4

**Groq модели:**
- `"llama-3.1-8b-instant"` - быстрая модель
- `"llama-3.1-70b-versatile"` - более мощная модель
- `"mixtral-8x7b-32768"` - Mixtral модель

### `autostart_enabled`
**Тип:** `boolean`
**По умолчанию:** `false`

Автоматический запуск приложения при старте системы.

## Настройка через UI

1. **Открыть настройки:**
   - Клик по иконке в системном трее
   - Выбрать "Settings"

2. **Выбор провайдера:**
   - Переключатель между OpenAI и Groq
   - Введите соответствующий API ключ

3. **Настройка транскрибации:**
   - Выберите язык
   - Выберите модель (зависит от провайдера)

4. **LLM обработка (опционально):**
   - Введите промпт для обработки текста
   - Выберите модель LLM

5. **Горячие клавиши:**
   - Введите комбинацию клавиш
   - Нажмите "Validate" для проверки
   - Система покажет, доступна ли комбинация

6. **Автозапуск:**
   - Включите переключатель для запуска с системой

7. **Сохранение:**
   - Нажмите "Save Settings"
   - Настройки применятся немедленно

## Примеры конфигураций

### Минимальная конфигурация (OpenAI)
```json
{
  "api_provider": "openai",
  "openai_api_key": "sk-...",
  "language": "ru",
  "hotkey": "Ctrl+Shift+Space"
}
```

### Конфигурация с LLM обработкой
```json
{
  "api_provider": "openai",
  "openai_api_key": "sk-...",
  "language": "ru",
  "llm_prompt": "Исправь грамматику и сделай текст более формальным",
  "llm_model": "gpt-4-turbo",
  "hotkey": "Ctrl+Shift+Space"
}
```

### Конфигурация с Groq
```json
{
  "api_provider": "groq",
  "groq_api_key": "gsk_...",
  "language": "en",
  "transcription_model": "distil-whisper-large-v3-en",
  "llm_prompt": "Fix grammar and punctuation",
  "llm_model": "llama-3.1-70b-versatile",
  "hotkey": "Alt+D"
}
```

### Конфигурация с автозапуском
```json
{
  "api_provider": "openai",
  "openai_api_key": "sk-...",
  "language": "ru",
  "hotkey": "Ctrl+Shift+Space",
  "autostart_enabled": true
}
```

## Переменные окружения

Для безопасности можно использовать переменные окружения вместо хранения ключей в конфиге:

```bash
# Windows (PowerShell)
$env:OPENAI_API_KEY = "sk-..."
$env:GROQ_API_KEY = "gsk_..."

# macOS/Linux
export OPENAI_API_KEY="sk-..."
export GROQ_API_KEY="gsk_..."
```

Приложение проверит переменные окружения, если ключи не указаны в настройках.

## Миграция настроек

При обновлении версии приложения настройки автоматически мигрируются. Старые настройки сохраняются в файле `settings.json.backup`.

## Сброс настроек

Для сброса настроек:

1. **Через UI:**
   - Откройте настройки
   - Нажмите "Reset to Defaults"

2. **Вручную:**
   - Удалите файл `settings.json`
   - Перезапустите приложение

## Troubleshooting

### Проблема: Горячие клавиши не работают
**Решение:**
- Убедитесь, что комбинация не занята другим приложением
- Попробуйте другую комбинацию
- Перезапустите приложение после изменения

### Проблема: API ключ не принимается
**Решение:**
- Проверьте правильность ключа
- Убедитесь, что у ключа есть необходимые права
- Проверьте баланс аккаунта

### Проблема: Настройки не сохраняются
**Решение:**
- Проверьте права доступа к папке конфигурации
- Убедитесь, что диск не заполнен
- Проверьте логи приложения