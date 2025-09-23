# API Documentation

## Tauri Commands

### Recording Control

#### `start_recording()`
Начинает запись аудио с микрофона по умолчанию.

**Returns:** `Result<(), String>`

**Example:**
```javascript
await invoke('start_recording');
```

#### `stop_and_transcribe()`
Останавливает запись и запускает процесс транскрибации.

**Returns:** `Result<(), String>`

**Example:**
```javascript
await invoke('stop_and_transcribe');
```

#### `is_recording()`
Проверяет, идёт ли запись в данный момент.

**Returns:** `bool`

**Example:**
```javascript
const recording = await invoke('is_recording');
```

### Settings Management

#### `get_settings()`
Получает текущие настройки приложения.

**Returns:** `Settings`

**Response Structure:**
```typescript
interface Settings {
  api_provider: 'openai' | 'groq';
  openai_api_key?: string;
  groq_api_key?: string;
  language: string;
  llm_prompt?: string;
  hotkey: string;
  transcription_model: string;
  llm_model?: string;
  autostart_enabled: boolean;
}
```

#### `save_settings(settings: Settings)`
Сохраняет настройки приложения.

**Parameters:**
- `settings: Settings` - объект с настройками

**Returns:** `Result<(), String>`

**Example:**
```javascript
await invoke('save_settings', {
  settings: {
    api_provider: 'openai',
    openai_api_key: 'sk-...',
    language: 'ru',
    hotkey: 'Ctrl+Shift+Space',
    transcription_model: 'whisper-1',
    autostart_enabled: false
  }
});
```

#### `validate_api_key(provider: string, api_key: string)`
Проверяет валидность API ключа.

**Parameters:**
- `provider: string` - провайдер ('openai' или 'groq')
- `api_key: string` - API ключ для проверки

**Returns:** `Result<bool, String>`

**Example:**
```javascript
const isValid = await invoke('validate_api_key', {
  provider: 'openai',
  apiKey: 'sk-...'
});
```

### Hotkey Management

#### `register_hotkey(hotkey: string)`
Регистрирует глобальную горячую клавишу.

**Parameters:**
- `hotkey: string` - комбинация клавиш (например, "Ctrl+Shift+Space")

**Returns:** `Result<(), String>`

**Example:**
```javascript
await invoke('register_hotkey', { hotkey: 'Ctrl+Shift+Space' });
```

#### `validate_hotkey(hotkey: string)`
Проверяет корректность комбинации клавиш.

**Parameters:**
- `hotkey: string` - комбинация для проверки

**Returns:** `Result<bool, String>`

**Example:**
```javascript
const isValid = await invoke('validate_hotkey', { hotkey: 'Ctrl+Space' });
```

### Window Management

#### `show_main_window()`
Показывает главное окно приложения.

**Returns:** `Result<(), String>`

#### `hide_main_window()`
Скрывает главное окно приложения.

**Returns:** `Result<(), String>`

## Events

### `transcription-status`
Событие статуса транскрибации.

**Payload:**
```typescript
interface StatusEvent {
  phase: 'recording' | 'processing' | 'transcribing' |
         'enhancing' | 'complete' | 'error';
  message: string;
  error?: string;
}
```

**Example:**
```javascript
import { listen } from '@tauri-apps/api/event';

const unlisten = await listen('transcription-status', (event) => {
  const { phase, message, error } = event.payload;

  switch(phase) {
    case 'recording':
      console.log('Recording...');
      break;
    case 'complete':
      console.log('Transcription complete:', message);
      break;
    case 'error':
      console.error('Error:', error);
      break;
  }
});

// Отписка
unlisten();
```

## Error Handling

Все команды возвращают `Result<T, String>`, где ошибки представлены как строки.

### Common Errors

- **API Key Missing**: "API key not configured"
- **Recording Error**: "Failed to start recording: [details]"
- **Transcription Error**: "Transcription failed: [details]"
- **Settings Error**: "Failed to save settings: [details]"
- **Hotkey Error**: "Invalid hotkey combination"

### Error Handling Example

```javascript
try {
  await invoke('start_recording');
} catch (error) {
  console.error('Failed to start recording:', error);
  // Показать пользователю сообщение об ошибке
}
```

## Rate Limits

### OpenAI
- Whisper API: зависит от вашего плана
- GPT API: зависит от вашего плана

### Groq
- Whisper: до 7,000 requests/min
- LLM: зависит от модели

## Best Practices

1. **Проверяйте API ключи** перед использованием команд транскрибации
2. **Обрабатывайте ошибки** для всех команд
3. **Подписывайтесь на события** для отслеживания статуса
4. **Валидируйте горячие клавиши** перед сохранением
5. **Сохраняйте настройки** только после успешной валидации