# Easy Dictate E2E Tests

Автоматизированные E2E тесты для Tauri приложения Easy Dictate.

## Возможности

- **WebdriverIO + Tauri Driver** - полноценные E2E тесты через WebDriver
- **Скриншоты** - автоматические скриншоты на каждом шаге и при ошибках
- **Логирование** - детальные логи каждого теста в JSON формате
- **Горячие клавиши** - тестирование глобальных хоткеев через Windows API
- **Аудио моки** - инъекция тестового аудио без микрофона
- **Allure отчёты** - красивые HTML отчёты с историей

## Установка

### Предварительные требования

1. **Tauri Driver**
```bash
cargo install tauri-driver
```

2. **Node.js зависимости**
```bash
cd tests/e2e
npm install
```

3. **Сборка приложения** (debug)
```bash
cargo tauri build --debug
```

4. **(Опционально) Виртуальный аудио кабель**

Для тестирования транскрипции без микрофона установите [VB-CABLE](https://vb-audio.com/Cable/).

## Запуск тестов

### Основной запуск
```bash
npm test
```

### С отладкой
```bash
npm run test:debug
```

### Генерация отчёта
```bash
npm run report
```

## Структура

```
tests/e2e/
├── specs/              # Тестовые спецификации
│   └── app.spec.ts     # Основные тесты
├── utils/              # Утилиты
│   ├── test-utils.ts   # Логирование, скриншоты
│   ├── hotkey-tester.ts # Тестирование хоткеев
│   └── audio-mock.ts   # Аудио моки
├── scripts/            # Скрипты
│   └── setup-audio.ts  # Подготовка аудио
├── screenshots/        # Скриншоты (автосоздание)
├── logs/               # Логи тестов
├── audio-mocks/        # Тестовые аудио файлы
└── wdio.conf.ts        # Конфигурация WebdriverIO
```

## Написание тестов

### Базовый тест
```typescript
describe('My Feature', () => {
  it('should work correctly', async () => {
    // Дождаться готовности Tauri
    await browser.waitForTauri();

    // Вызвать Tauri команду
    const result = await browser.tauriInvoke('get_settings');

    // Сделать скриншот
    await browser.screenshotWithTimestamp('my_test');

    // Проверить UI элемент
    const element = await browser.$('#my-element');
    expect(await element.isExisting()).toBe(true);
  });
});
```

### Тест горячих клавиш
```typescript
import { pressGlobalHotkey, HotkeyTester } from '../utils/hotkey-tester';

it('should handle hotkey', async () => {
  await pressGlobalHotkey('Ctrl+Shift+Space');
  await browser.waitForStatus('recording', 5000);
});
```

### Тест с аудио инъекцией
```typescript
import { AudioInjector, setupTestAudioFiles } from '../utils/audio-mock';

it('should transcribe audio', async () => {
  const audioFiles = await setupTestAudioFiles();
  const injector = new AudioInjector();

  // Начать запись
  await pressGlobalHotkey('Ctrl+Shift+Space');

  // Инжектить аудио
  await injector.injectAudio(audioFiles['SHORT_PHRASE']);

  // Остановить запись
  await pressGlobalHotkey('Ctrl+Shift+Space');
});
```

## Настройка виртуального аудио

Для полноценного тестирования транскрипции без микрофона:

1. Установите [VB-CABLE](https://vb-audio.com/Cable/)
2. В настройках Windows: **Input device** = `CABLE Output (VB-Audio Virtual Cable)`
3. Тесты будут автоматически находить виртуальный кабель и проигрывать туда тестовое аудио

## Артефакты тестов

После запуска тестов:

- `screenshots/` - все скриншоты с временными метками
- `logs/` - JSON логи каждого теста
- `allure-results/` - данные для Allure отчёта

## CI/CD интеграция

```yaml
# GitHub Actions пример
- name: Install tauri-driver
  run: cargo install tauri-driver

- name: Build app (debug)
  run: cargo tauri build --debug

- name: Run E2E tests
  run: |
    cd tests/e2e
    npm install
    npm test

- name: Upload screenshots
  uses: actions/upload-artifact@v4
  if: failure()
  with:
    name: test-screenshots
    path: tests/e2e/screenshots/
```

## Troubleshooting

### tauri-driver не найден
```bash
cargo install tauri-driver
# или добавьте в PATH: ~/.cargo/bin
```

### Тесты зависают
- Проверьте что приложение собрано: `cargo tauri build --debug`
- Проверьте путь к exe в `wdio.conf.ts`

### Горячие клавиши не работают
- Запустите тесты от имени администратора
- Проверьте что фокус на приложении
