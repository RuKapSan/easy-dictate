/**
 * Internationalization (i18n) module for Easy Dictate
 */

const translations = {
  ru: {
    // Tab navigation
    'tab.dictation': 'Диктовка',
    'tab.settings': 'Настройки',

    // Status section
    'status.ready': 'Готово к записи',
    'status.recording': 'Идёт запись...',
    'status.transcribing': 'Распознавание...',
    'status.success': 'Готово',
    'status.error': 'Ошибка',
    'status.hint.ready': 'Нажмите горячую клавишу для начала',
    'status.hint.recording': 'Отпустите клавишу для завершения',
    'status.hint.transcribing': 'Обработка аудио...',
    'status.hint.success': 'Текст скопирован',
    'status.hint.error': 'Попробуйте ещё раз',

    // History section
    'history.title': 'История',
    'history.empty': 'Нет записей',
    'history.clear': 'Очистить историю',
    'history.copy': 'Копировать',
    'history.delete': 'Удалить',

    // Provider section
    'provider.title': 'Провайдер распознавания',
    'provider.openai.name': 'OpenAI',
    'provider.openai.desc': 'Whisper, GPT-4o',
    'provider.groq.name': 'Groq',
    'provider.groq.desc': 'Быстро и бесплатно',
    'provider.elevenlabs.name': 'ElevenLabs',
    'provider.elevenlabs.desc': 'Реалтайм стриминг',

    // API keys
    'apikey.openai': 'API ключ OpenAI',
    'apikey.groq': 'API ключ Groq',
    'apikey.elevenlabs': 'API ключ ElevenLabs',
    'apikey.show': 'Показать/скрыть',

    // Model
    'model.label': 'Модель',

    // Hotkeys section
    'hotkeys.title': 'Горячие клавиши',
    'hotkeys.main': 'Основная',
    'hotkeys.translate': 'С переводом',
    'hotkeys.toggle': 'Вкл/выкл перевод',
    'hotkeys.notset': 'Не задано',
    'hotkeys.reset': 'Сбросить',
    'hotkeys.press': 'Нажмите...',
    'hotkeys.hold': 'Удерживайте...',

    // Processing section (Translation + Instructions)
    'processing.title': 'Обработка',

    // Translation subsection
    'translation.title': 'Перевод',
    'translation.enable': 'Переводить результат',
    'translation.enable.hint': 'Автоперевод после распознавания',
    'translation.language': 'Язык',
    'translation.llm': 'LLM',

    // Target languages
    'lang.russian': 'Русский',
    'lang.english': 'Английский',
    'lang.german': 'Немецкий',
    'lang.french': 'Французский',
    'lang.spanish': 'Испанский',
    'lang.italian': 'Итальянский',
    'lang.portuguese': 'Португальский',
    'lang.chinese': 'Китайский',
    'lang.japanese': 'Японский',
    'lang.korean': 'Корейский',

    // Custom instructions section
    'instructions.title': 'Инструкции',
    'instructions.enable': 'Свои инструкции',
    'instructions.enable.hint': 'Постобработка текста через LLM',
    'instructions.placeholder': 'Например: Сделай краткое резюме, исправь грамматику, отформатируй как список...',

    // Vocabulary section
    'vocabulary.enable': 'Словарь терминов',
    'vocabulary.enable.hint': 'Исправление терминов через LLM',
    'vocabulary.placeholder': 'По одному термину на строку:\nGroq\nTauri\nWebSocket',
    'vocabulary.terms': 'терминов',
    'vocabulary.import': 'Импорт',
    'vocabulary.export': 'Экспорт',

    // Behavior section
    'behavior.title': 'Поведение',
    'behavior.typing': 'Эмуляция ввода',
    'behavior.typing.hint': 'Печатает текст в активное окно',
    'behavior.clipboard': 'В буфер обмена',
    'behavior.clipboard.hint': 'Копировать результат',
    'behavior.streaming': 'Реалтайм текст',
    'behavior.streaming.hint': 'Показывать текст во время записи',

    // System section
    'system.title': 'Система',
    'system.autostart': 'Автозапуск',
    'system.autostart.hint': 'Запускать при старте Windows',
    'system.tray': 'Запуск в трее',
    'system.tray.hint': 'Сворачивать при старте',
    'system.autoupdate': 'Автообновления',
    'system.autoupdate.hint': 'Проверять новые версии',
    'system.language': 'Язык интерфейса',

    // Actions
    'actions.revert': 'Отменить',
    'actions.save': 'Сохранить',

    // Toast messages
    'toast.saved': 'Сохранено',
    'toast.settings.saved': 'Настройки сохранены',
    'toast.hotkey.saved': 'Горячая клавиша сохранена',
    'toast.hotkey.translate.saved': 'Клавиша перевода сохранена',
    'toast.hotkey.toggle.saved': 'Клавиша переключения сохранена',
    'toast.changes.reverted': 'Изменения отменены',
    'toast.copied': 'Скопировано',
    'toast.history.cleared': 'История очищена',
    'toast.error': 'Ошибка',
    'toast.error.save': 'Ошибка при сохранении',
    'toast.error.load': 'Не удалось загрузить настройки',
    'toast.error.delete': 'Не удалось удалить',
    'toast.error.clear': 'Не удалось очистить',
    'toast.error.hotkey.main': 'Выберите горячую клавишу',
    'toast.error.hotkey.key': 'Нужна основная клавиша',
    'toast.error.hotkey.mouse': 'Мышь не поддерживается',
    'toast.error.hotkey.recognize': 'Не удалось распознать',
  },

  en: {
    // Tab navigation
    'tab.dictation': 'Dictation',
    'tab.settings': 'Settings',

    // Status section
    'status.ready': 'Ready to record',
    'status.recording': 'Recording...',
    'status.transcribing': 'Transcribing...',
    'status.success': 'Done',
    'status.error': 'Error',
    'status.hint.ready': 'Press hotkey to start',
    'status.hint.recording': 'Release key to finish',
    'status.hint.transcribing': 'Processing audio...',
    'status.hint.success': 'Text copied',
    'status.hint.error': 'Try again',

    // History section
    'history.title': 'History',
    'history.empty': 'No records',
    'history.clear': 'Clear history',
    'history.copy': 'Copy',
    'history.delete': 'Delete',

    // Provider section
    'provider.title': 'Transcription Provider',
    'provider.openai.name': 'OpenAI',
    'provider.openai.desc': 'Whisper, GPT-4o',
    'provider.groq.name': 'Groq',
    'provider.groq.desc': 'Fast and free',
    'provider.elevenlabs.name': 'ElevenLabs',
    'provider.elevenlabs.desc': 'Realtime streaming',

    // API keys
    'apikey.openai': 'OpenAI API Key',
    'apikey.groq': 'Groq API Key',
    'apikey.elevenlabs': 'ElevenLabs API Key',
    'apikey.show': 'Show/hide',

    // Model
    'model.label': 'Model',

    // Hotkeys section
    'hotkeys.title': 'Hotkeys',
    'hotkeys.main': 'Main',
    'hotkeys.translate': 'With translation',
    'hotkeys.toggle': 'Toggle translation',
    'hotkeys.notset': 'Not set',
    'hotkeys.reset': 'Reset',
    'hotkeys.press': 'Press...',
    'hotkeys.hold': 'Hold...',

    // Processing section (Translation + Instructions)
    'processing.title': 'Processing',

    // Translation subsection
    'translation.title': 'Translation',
    'translation.enable': 'Translate result',
    'translation.enable.hint': 'Auto-translate after transcription',
    'translation.language': 'Language',
    'translation.llm': 'LLM',

    // Target languages
    'lang.russian': 'Russian',
    'lang.english': 'English',
    'lang.german': 'German',
    'lang.french': 'French',
    'lang.spanish': 'Spanish',
    'lang.italian': 'Italian',
    'lang.portuguese': 'Portuguese',
    'lang.chinese': 'Chinese',
    'lang.japanese': 'Japanese',
    'lang.korean': 'Korean',

    // Custom instructions section
    'instructions.title': 'Instructions',
    'instructions.enable': 'Custom instructions',
    'instructions.enable.hint': 'Post-process text via LLM',
    'instructions.placeholder': 'E.g.: Make a brief summary, fix grammar, format as list...',

    // Vocabulary section
    'vocabulary.enable': 'Custom vocabulary',
    'vocabulary.enable.hint': 'Fix technical terms via LLM',
    'vocabulary.placeholder': 'One term per line:\nGroq\nTauri\nWebSocket',
    'vocabulary.terms': 'terms',
    'vocabulary.import': 'Import',
    'vocabulary.export': 'Export',

    // Behavior section
    'behavior.title': 'Behavior',
    'behavior.typing': 'Simulate typing',
    'behavior.typing.hint': 'Type text into active window',
    'behavior.clipboard': 'Copy to clipboard',
    'behavior.clipboard.hint': 'Copy result to clipboard',
    'behavior.streaming': 'Realtime text',
    'behavior.streaming.hint': 'Show text while recording',

    // System section
    'system.title': 'System',
    'system.autostart': 'Auto-start',
    'system.autostart.hint': 'Launch on Windows startup',
    'system.tray': 'Start minimized',
    'system.tray.hint': 'Minimize to tray on launch',
    'system.autoupdate': 'Auto-update',
    'system.autoupdate.hint': 'Check for new versions',
    'system.language': 'Interface language',

    // Actions
    'actions.revert': 'Revert',
    'actions.save': 'Save',

    // Toast messages
    'toast.saved': 'Saved',
    'toast.settings.saved': 'Settings saved',
    'toast.hotkey.saved': 'Hotkey saved',
    'toast.hotkey.translate.saved': 'Translate hotkey saved',
    'toast.hotkey.toggle.saved': 'Toggle hotkey saved',
    'toast.changes.reverted': 'Changes reverted',
    'toast.copied': 'Copied',
    'toast.history.cleared': 'History cleared',
    'toast.error': 'Error',
    'toast.error.save': 'Failed to save',
    'toast.error.load': 'Failed to load settings',
    'toast.error.delete': 'Failed to delete',
    'toast.error.clear': 'Failed to clear',
    'toast.error.hotkey.main': 'Select a hotkey',
    'toast.error.hotkey.key': 'Need a main key',
    'toast.error.hotkey.mouse': 'Mouse not supported',
    'toast.error.hotkey.recognize': 'Could not recognize',
  }
};

// Current language
let currentLang = 'ru';

/**
 * Get translation for a key
 * @param {string} key - Translation key
 * @param {Object} params - Optional parameters for interpolation
 * @returns {string} Translated string
 */
function t(key, params = {}) {
  const langTranslations = translations[currentLang] || translations['en'];
  let text = langTranslations[key] || translations['en'][key] || key;

  // Simple parameter interpolation: {param}
  Object.keys(params).forEach(param => {
    text = text.replace(new RegExp(`\\{${param}\\}`, 'g'), params[param]);
  });

  return text;
}

/**
 * Set current language
 * @param {string} lang - Language code ('ru' or 'en')
 */
function setLanguage(lang) {
  if (translations[lang]) {
    currentLang = lang;
    localStorage.setItem('ui_language', lang);
    applyTranslations();
  }
}

/**
 * Get current language
 * @returns {string} Current language code
 */
function getLanguage() {
  return currentLang;
}

/**
 * Get available languages
 * @returns {Array} Array of {code, name} objects
 */
function getAvailableLanguages() {
  return [
    { code: 'ru', name: 'Русский' },
    { code: 'en', name: 'English' }
  ];
}

/**
 * Initialize i18n - load saved language preference
 */
function initI18n() {
  const saved = localStorage.getItem('ui_language');
  if (saved && translations[saved]) {
    currentLang = saved;
  }
}

/**
 * Apply translations to all elements with data-i18n attribute
 */
function applyTranslations() {
  // Translate elements with data-i18n attribute
  document.querySelectorAll('[data-i18n]').forEach(el => {
    const key = el.getAttribute('data-i18n');
    if (key) {
      el.textContent = t(key);
    }
  });

  // Translate elements with data-i18n-placeholder attribute
  document.querySelectorAll('[data-i18n-placeholder]').forEach(el => {
    const key = el.getAttribute('data-i18n-placeholder');
    if (key) {
      el.placeholder = t(key);
    }
  });

  // Translate elements with data-i18n-title attribute
  document.querySelectorAll('[data-i18n-title]').forEach(el => {
    const key = el.getAttribute('data-i18n-title');
    if (key) {
      el.title = t(key);
    }
  });

  // Update HTML lang attribute
  document.documentElement.lang = currentLang;
}

// Export for use in main.js
window.i18n = {
  t,
  setLanguage,
  getLanguage,
  getAvailableLanguages,
  initI18n,
  applyTranslations
};
