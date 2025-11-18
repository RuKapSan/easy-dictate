# ElevenLabs Streaming STT - Архитектурный дизайн

## Обзор

Реализация real-time streaming транскрипции с persistent WebSocket соединением, позволяющая:
- Держать WebSocket открытым и переиспользовать
- Отправлять аудио чанки по требованию (по кнопке)
- Получать partial/committed транскрипции в реальном времени
- Восстанавливать полный текст при изменении контекста

## Архитектура

### 1. Backend Components

#### 1.1 StreamingConnection (внутренняя структура)
```rust
struct StreamingConnection {
    write: Arc<Mutex<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>>,
    cancel_token: CancellationToken,
    reader_task: JoinHandle<()>,
    sample_rate: u32,
    audio_format: String,
    session_id: Option<String>,
}
```

**Назначение:**
- `write` - для отправки аудио чанков (потокобезопасно с Mutex)
- `cancel_token` - для остановки background task
- `reader_task` - handle задачи чтения сообщений
- `sample_rate`, `audio_format` - параметры текущей сессии
- `session_id` - ID сессии от ElevenLabs

#### 1.2 ElevenLabsStreamingClient (публичный API)
```rust
pub struct ElevenLabsStreamingClient {
    connection: Arc<Mutex<Option<StreamingConnection>>>,
}

impl ElevenLabsStreamingClient {
    pub fn new() -> Self;

    pub async fn connect(
        &self,
        api_key: String,
        sample_rate: u32,
        language_code: String,
        app_handle: AppHandle,
    ) -> Result<()>;

    pub async fn send_audio_chunk(
        &self,
        pcm_data: Vec<u8>,
        commit: bool,
    ) -> Result<()>;

    pub async fn commit(&self) -> Result<()>;

    pub async fn disconnect(&self) -> Result<()>;

    pub fn is_connected(&self) -> bool;
}
```

#### 1.3 Background Message Reader Task
```rust
async fn message_reader_task(
    mut read: SplitStream<WebSocketStream>,
    app_handle: AppHandle,
    cancel_token: CancellationToken,
) {
    let mut session_id: Option<String> = None;

    loop {
        tokio::select! {
            _ = cancel_token.cancelled() => {
                log::info!("[ElevenLabs] Reader task cancelled");
                break;
            }
            msg_result = read.next() => {
                match msg_result {
                    Some(Ok(Message::Text(text))) => {
                        handle_text_message(&text, &app_handle, &mut session_id);
                    }
                    Some(Ok(Message::Close(frame))) => {
                        emit_event(&app_handle, "connection_closed", frame);
                        break;
                    }
                    Some(Err(e)) => {
                        emit_error(&app_handle, e);
                        break;
                    }
                    None => break,
                    _ => {}
                }
            }
        }
    }
}

fn handle_text_message(
    text: &str,
    app_handle: &AppHandle,
    session_id: &mut Option<String>,
) {
    if let Ok(msg) = serde_json::from_str::<TranscriptMessage>(text) {
        match msg.message_type.as_str() {
            "session_started" => {
                *session_id = msg.session_id.clone();
                emit_event(app_handle, "session_started", msg);
            }
            "partial_transcript" => {
                emit_event(app_handle, "partial_transcript", msg);
            }
            "committed_transcript" | "committed_transcript_with_timestamps" => {
                emit_event(app_handle, "committed_transcript", msg);
            }
            "error" | "auth_error" | "quota_exceeded_error" => {
                emit_event(app_handle, "error", msg);
            }
            _ => {}
        }
    }
}
```

### 2. Tauri Events

#### 2.1 Event Payloads
```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SessionStartedEvent {
    session_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PartialTranscriptEvent {
    text: String,
    sequence: Option<u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CommittedTranscriptEvent {
    text: String,
    sequence: Option<u32>,
    segment_id: Option<String>,
    #[serde(default)]
    words: Vec<WordTimestamp>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WordTimestamp {
    word: String,
    start_ms: f64,
    end_ms: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ErrorEvent {
    error: String,
    message_type: String,
}
```

#### 2.2 Event Names
- `elevenlabs://session-started`
- `elevenlabs://partial-transcript`
- `elevenlabs://committed-transcript`
- `elevenlabs://error`
- `elevenlabs://connection-closed`

### 3. Tauri Commands

```rust
#[tauri::command]
async fn elevenlabs_connect(
    state: State<'_, AppState>,
    api_key: String,
    sample_rate: u32,
    language_code: String,
    app_handle: AppHandle,
) -> Result<(), String> {
    state
        .elevenlabs_streaming
        .connect(api_key, sample_rate, language_code, app_handle)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn elevenlabs_send_chunk(
    state: State<'_, AppState>,
    pcm_data: Vec<u8>,
    commit: bool,
) -> Result<(), String> {
    state
        .elevenlabs_streaming
        .send_audio_chunk(pcm_data, commit)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn elevenlabs_commit(
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .elevenlabs_streaming
        .commit()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn elevenlabs_disconnect(
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .elevenlabs_streaming
        .disconnect()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn elevenlabs_is_connected(
    state: State<'_, AppState>,
) -> bool {
    state.elevenlabs_streaming.is_connected()
}
```

### 4. Frontend Architecture

#### 4.1 State Management
```typescript
interface TranscriptSegment {
  id: string;
  text: string;
  sequence: number;
  isCommitted: boolean;
  timestamp: number;
}

class StreamingTranscriptManager {
  private segments: Map<string, TranscriptSegment> = new Map();
  private currentPartial: string = '';
  private sequenceCounter: number = 0;

  constructor() {
    this.setupEventListeners();
  }

  private setupEventListeners() {
    window.__TAURI__.event.listen('elevenlabs://partial-transcript', (event) => {
      this.handlePartial(event.payload);
    });

    window.__TAURI__.event.listen('elevenlabs://committed-transcript', (event) => {
      this.handleCommitted(event.payload);
    });
  }

  private handlePartial(payload: PartialTranscriptEvent) {
    this.currentPartial = payload.text;
    this.updateUI();
  }

  private handleCommitted(payload: CommittedTranscriptEvent) {
    const segmentId = payload.segment_id || `seg_${this.sequenceCounter++}`;

    // Если сегмент уже существует - это обновление контекста
    if (this.segments.has(segmentId)) {
      console.log(`[Context update] Replacing segment ${segmentId}`);
      console.log(`  Old: "${this.segments.get(segmentId)?.text}"`);
      console.log(`  New: "${payload.text}"`);
    }

    this.segments.set(segmentId, {
      id: segmentId,
      text: payload.text,
      sequence: payload.sequence || this.sequenceCounter,
      isCommitted: true,
      timestamp: Date.now(),
    });

    this.currentPartial = ''; // Очистить partial
    this.updateUI();
  }

  getFullTranscript(): string {
    // Сортировать по sequence и объединить
    return Array.from(this.segments.values())
      .sort((a, b) => a.sequence - b.sequence)
      .map(s => s.text)
      .join(' ');
  }

  private updateUI() {
    const fullText = this.getFullTranscript();
    const displayText = this.currentPartial
      ? `${fullText} ${this.currentPartial}`
      : fullText;

    // Обновить UI элементы
    updateTranscriptDisplay(displayText, this.currentPartial !== '');
  }
}
```

#### 4.2 UI Controls
```typescript
class StreamingControls {
  private manager: StreamingTranscriptManager;
  private isConnected: boolean = false;
  private audioRecorder: MediaRecorder | null = null;

  async connect() {
    try {
      await invoke('elevenlabs_connect', {
        apiKey: getApiKey(),
        sampleRate: 48000,
        languageCode: 'ru',
      });
      this.isConnected = true;
      this.updateButtonStates();
    } catch (error) {
      console.error('Connection failed:', error);
    }
  }

  async sendCurrentAudio(commit: bool = false) {
    if (!this.isConnected) {
      console.error('Not connected');
      return;
    }

    // Получить текущий аудио буфер (PCM16)
    const pcmData = await this.captureAudio();

    await invoke('elevenlabs_send_chunk', {
      pcmData: Array.from(pcmData),
      commit,
    });
  }

  async commit() {
    await invoke('elevenlabs_commit');
  }

  async disconnect() {
    await invoke('elevenlabs_disconnect');
    this.isConnected = false;
    this.updateButtonStates();
  }
}
```

### 5. Data Flow

```
User Action (button press)
    ↓
Capture audio chunk (PCM16)
    ↓
invoke('elevenlabs_send_chunk', {pcmData, commit: false})
    ↓
[Backend] ElevenLabsStreamingClient::send_audio_chunk()
    ↓
Encode to base64, send via WebSocket
    ↓
[ElevenLabs API] Process audio
    ↓
[Backend] message_reader_task receives response
    ↓
Parse message, emit Tauri event
    ↓
[Frontend] Event listener handles update
    ↓
StreamingTranscriptManager updates state
    ↓
UI updates with new transcript
```

### 6. Обработка восстановления контекста

#### 6.1 Проблема
ElevenLabs может изменить предыдущие слова когда получает больше контекста:
- "I went to the..." → "I went to the store"
- Но потом: "I went to the store yesterday" (изменилось предыдущее committed)

#### 6.2 Решение
1. **Использовать segment_id**: ElevenLabs должен отправлять уникальный ID для каждого сегмента
2. **Хранить Map сегментов**: `Map<segment_id, TranscriptSegment>`
3. **При получении committed с существующим ID**: заменить старый текст
4. **Пересобрать полный текст**: отсортировать по sequence и объединить

```typescript
// Пример обработки
handleCommitted(payload) {
  const segmentId = payload.segment_id || generateId();

  if (this.segments.has(segmentId)) {
    // Это обновление контекста!
    const old = this.segments.get(segmentId);
    console.log(`Context update: "${old.text}" → "${payload.text}"`);

    // Показать пользователю что текст изменился
    this.highlightChangedSegment(segmentId);
  }

  this.segments.set(segmentId, {
    id: segmentId,
    text: payload.text,
    sequence: payload.sequence,
    isCommitted: true,
  });

  this.rebuildFullTranscript();
}
```

### 7. Обработка ошибок и edge cases

#### 7.1 Переподключение
```rust
impl ElevenLabsStreamingClient {
    pub async fn reconnect(&self) -> Result<()> {
        // Сохранить параметры соединения
        let params = self.get_connection_params()?;

        // Закрыть старое соединение
        self.disconnect().await?;

        // Открыть новое
        self.connect(
            params.api_key,
            params.sample_rate,
            params.language_code,
            params.app_handle,
        ).await
    }
}
```

#### 7.2 Таймауты и keep-alive
```rust
// В message_reader_task добавить ping/pong
tokio::select! {
    _ = tokio::time::sleep(Duration::from_secs(30)) => {
        // Отправить ping для keep-alive
        if let Err(e) = write.send(Message::Ping(vec![])).await {
            log::error!("Failed to send ping: {}", e);
            break;
        }
    }
    // ... остальная логика
}
```

#### 7.3 Буферизация при отключении
```typescript
class OfflineBuffer {
  private pendingChunks: Array<{data: Uint8Array, commit: boolean}> = [];

  addChunk(data: Uint8Array, commit: boolean) {
    if (!isConnected()) {
      this.pendingChunks.push({data, commit});
      return;
    }

    this.sendChunk(data, commit);
  }

  async flushPending() {
    for (const chunk of this.pendingChunks) {
      await this.sendChunk(chunk.data, chunk.commit);
    }
    this.pendingChunks = [];
  }
}
```

## План реализации

### Этап 1: Backend foundation (2-3 часа)
- [ ] Создать `ElevenLabsStreamingClient` структуру
- [ ] Реализовать `connect()` метод
- [ ] Создать background task для чтения сообщений
- [ ] Настроить Tauri events
- [ ] Тестировать подключение и чтение сообщений

### Этап 2: Backend API (1-2 часа)
- [ ] Реализовать `send_audio_chunk()`
- [ ] Реализовать `commit()`
- [ ] Реализовать `disconnect()`
- [ ] Добавить обработку ошибок
- [ ] Интегрировать в AppState

### Этап 3: Tauri Commands (30 мин)
- [ ] Создать команды для всех методов
- [ ] Зарегистрировать команды в builder

### Этап 4: Frontend events (1 час)
- [ ] Создать event listeners
- [ ] Реализовать `StreamingTranscriptManager`
- [ ] Тестировать получение событий

### Этап 5: Frontend UI (2 часа)
- [ ] Добавить кнопки управления (Connect/Disconnect/Send/Commit)
- [ ] Показывать статус соединения
- [ ] Отображать partial vs committed
- [ ] Визуальные индикаторы

### Этап 6: Context restoration (1 час)
- [ ] Реализовать логику segment_id
- [ ] Обработка обновлений сегментов
- [ ] Пересборка полного текста

### Этап 7: Polish & Testing (1-2 часа)
- [ ] Обработка ошибок
- [ ] Переподключение
- [ ] Keep-alive
- [ ] Тестирование различных сценариев

**Общее время: ~8-12 часов**

## Зависимости

Новые:
```toml
tokio = { version = "1.40", features = ["macros", "rt-multi-thread", "sync", "time"] }
tokio-util = { version = "0.7", features = ["codec"] }
```

Уже есть:
- tokio-tungstenite (с native-tls)
- serde, serde_json
- anyhow
- futures

## Вопросы для уточнения

1. **Стратегия commit**: Manual или VAD?
   - Manual: пользователь вручную нажимает "завершить сегмент"
   - VAD: автоматически по паузам в речи

2. **UI для streaming режима**:
   - Отдельный режим или интегрировать в существующий?
   - Показывать partial транскрипции?

3. **Что делать с существующим non-streaming режимом**:
   - Оставить оба?
   - Переключение между ними?

4. **Длительность сессии**:
   - Держать открытым пока пользователь не закроет?
   - Автоматически закрывать по таймауту?
