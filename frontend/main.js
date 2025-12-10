let invoke = null;
let listen = null;
let emit = null;
let tauriApp = null;

function dbg(msg, level = "info") {
  try {
    const m = `[UI] ${msg}`;
    if (level === "error") console.error(m);
    else if (level === "warn") console.warn(m);
    else console.log(m);
  } catch { }
  try {
    if (invoke) invoke("frontend_log", { level, message: msg }).catch(() => { });
  } catch { }
}

function hydrateTauriApis() {
  const tauri = window.__TAURI__;
  if (!tauri) {
    dbg("__TAURI__ is undefined at hydrate", "warn");
    return;
  }
  invoke = tauri.core?.invoke ?? null;
  listen = tauri.event?.listen ?? null;
  emit = tauri.event?.emit ?? null;
  tauriApp = tauri.app ?? null;
  dbg(`TAURI wired: invoke=${!!invoke}, listen=${!!listen}, app=${!!tauriApp}`);
}

hydrateTauriApis();

// ============================================================================
// DOM Elements
// ============================================================================

// Tab navigation
const tabBtns = document.querySelectorAll('.tab-btn');
const tabContents = document.querySelectorAll('.tab-content');

// Status elements
const statusOrb = document.getElementById("status-orb");
const statusText = document.getElementById("status-text");
const statusHint = document.getElementById("status-hint");
const progressEl = document.getElementById("progress");
const resultEl = document.getElementById("last-result");
const toastEl = document.getElementById("toast");

// Form and settings
const form = document.getElementById("settings-form");
const providerRadios = document.querySelectorAll('input[name="provider"]');
const apiKeyInput = document.getElementById("apiKey");
const groqApiKeyInput = document.getElementById("groqApiKey");
const elevenlabsApiKeyInput = document.getElementById("elevenlabsApiKey");
const openaiApiKeyField = document.getElementById("openai-api-key-field");
const groqApiKeyField = document.getElementById("groq-api-key-field");
const elevenlabsApiKeyField = document.getElementById("elevenlabs-api-key-field");
const modelSelect = document.getElementById("model");

// Hotkeys
const hotkeyHiddenInput = document.getElementById("hotkey");
const hotkeyDisplay = document.getElementById("hotkeyDisplay");
const hotkeyClearBtn = document.getElementById("hotkeyClear");
const translateHotkeyHiddenInput = document.getElementById("translateHotkey");
const translateHotkeyDisplay = document.getElementById("translateHotkeyDisplay");
const translateHotkeyClearBtn = document.getElementById("translateHotkeyClear");
const toggleTranslateHotkeyHiddenInput = document.getElementById("toggleTranslateHotkey");
const toggleTranslateHotkeyDisplay = document.getElementById("toggleTranslateHotkeyDisplay");
const toggleTranslateHotkeyClearBtn = document.getElementById("toggleTranslateHotkeyClear");

// Behavior toggles
const simulateTypingInput = document.getElementById("simulateTyping");
const copyToClipboardInput = document.getElementById("copyToClipboard");
const autoStartInput = document.getElementById("autoStart");
const startMinimizedInput = document.getElementById("startMinimized");
const autoUpdateInput = document.getElementById("autoUpdate");
const useStreamingInput = document.getElementById("useStreaming");

// Translation
const autoTranslateInput = document.getElementById("autoTranslate");
const targetLanguageSelect = document.getElementById("targetLanguage");
const translationOptions = document.getElementById("translationOptions");
const llmProviderSelect = document.getElementById("llmProvider");

// Custom instructions
const useCustomInstructionsInput = document.getElementById("useCustomInstructions");
const customInstructionsWrapper = document.getElementById("customInstructionsWrapper");
const customInstructionsInput = document.getElementById("customInstructions");

// Vocabulary
const useVocabularyInput = document.getElementById("useVocabulary");
const vocabularyWrapper = document.getElementById("vocabularyWrapper");
const customVocabularyInput = document.getElementById("customVocabulary");
const vocabularyCountEl = document.getElementById("vocabularyCount");
const importVocabularyBtn = document.getElementById("importVocabulary");
const exportVocabularyBtn = document.getElementById("exportVocabulary");

// Actions
const revertBtn = document.getElementById("revertBtn");

// History
const historyListEl = document.getElementById("historyList");
const clearHistoryBtn = document.getElementById("clearHistoryBtn");

// UI Language
const uiLanguageSelect = document.getElementById("uiLanguage");

// ============================================================================
// Constants
// ============================================================================

const DEFAULT_HOTKEY = "Ctrl+Shift+Space";
const MODIFIER_ORDER = ["Ctrl", "Shift", "Alt", "Win"];
const MODIFIER_NAMES = new Set(MODIFIER_ORDER);
const MODIFIER_CODES = new Set([
  "ControlLeft", "ControlRight",
  "ShiftLeft", "ShiftRight",
  "AltLeft", "AltRight",
  "MetaLeft", "MetaRight",
]);
const KEY_CODE_LABELS = {
  Space: "Space", Escape: "Esc", Enter: "Enter", Tab: "Tab",
  Backspace: "Backspace", Delete: "Delete",
  ArrowUp: "Up", ArrowDown: "Down", ArrowLeft: "Left", ArrowRight: "Right",
  CapsLock: "CapsLock", PageUp: "PageUp", PageDown: "PageDown",
  Home: "Home", End: "End", Insert: "Insert",
  Pause: "Pause", PrintScreen: "PrintScreen", ScrollLock: "ScrollLock",
  ContextMenu: "ContextMenu",
  Backquote: "`", Minus: "-", Equal: "=",
  BracketLeft: "[", BracketRight: "]", Backslash: "\\",
  IntlBackslash: "IntlBackslash", Semicolon: ";", Quote: "'",
  Comma: ",", Period: ".", Slash: "/",
};
const MOUSE_BUTTON_NAMES = {
  0: "MouseLeft", 1: "MouseMiddle", 2: "MouseRight",
  3: "MouseButton4", 4: "MouseButton5",
};

let initialSettings = null;
let isCapturingHotkey = false;
let hotkeyBeforeCapture = "";
const pressedModifiers = new Set();
let currentCapturingTarget = null;

// ============================================================================
// Tab Navigation
// ============================================================================

function initTabs() {
  tabBtns.forEach(btn => {
    btn.addEventListener('click', () => {
      const tabName = btn.dataset.tab;

      // Update buttons
      tabBtns.forEach(b => b.classList.remove('active'));
      btn.classList.add('active');

      // Update content
      tabContents.forEach(content => {
        content.classList.toggle('active', content.dataset.tab === tabName);
      });
    });
  });
}

// ============================================================================
// Toast
// ============================================================================

function showToast(message, type = "info") {
  if (!toastEl) return;
  toastEl.textContent = message;
  toastEl.dataset.type = type;
  toastEl.hidden = false;
  setTimeout(() => { toastEl.hidden = true; }, 2800);
}

// ============================================================================
// Settings Persistence
// ============================================================================

async function persistSettings(payload, successMessage = null) {
  dbg(`persistSettings: ${JSON.stringify(payload)}`);
  if (!invoke) {
    dbg("invoke is not available in persistSettings", "error");
    return false;
  }
  try {
    await invoke("save_settings", { settings: payload });
    dbg("invoke(save_settings) ok");
    initialSettings = { ...payload };
    if (emit) emit('settings://changed', {});
    if (successMessage) showToast(successMessage);
    else if (successMessage !== false) showToast(t('toast.saved'));
    return true;
  } catch (error) {
    console.error("[PERSIST] save_settings failed:", error);
    dbg(`save_settings failed: ${String(error)}`, "error");
    showToast(t('toast.error.save'), "error");
    return false;
  }
}

// Helper function to get translation (uses window.i18n if available)
function t(key, params = {}) {
  if (window.i18n && window.i18n.t) {
    return window.i18n.t(key, params);
  }
  return key;
}

// ============================================================================
// Status
// ============================================================================

function setStatus(state, text, hint = null) {
  if (statusOrb) {
    statusOrb.className = `status-orb ${state}`;
  }
  if (statusText) statusText.textContent = text;
  if (statusHint) {
    statusHint.textContent = hint ?? getDefaultHint(state);
  }
}

function getDefaultHint(state) {
  switch (state) {
    case 'recording': return t('status.hint.recording');
    case 'transcribing': return t('status.hint.transcribing');
    case 'success': return t('status.hint.success');
    case 'error': return t('status.hint.error');
    default: return t('status.hint.ready');
  }
}

// ============================================================================
// Hotkey Handling
// ============================================================================

function normalizeHotkeyValue(value) {
  return (value ?? "").trim();
}

function getHotkeyElements(target) {
  if (target === 'translate') {
    return { hidden: translateHotkeyHiddenInput, display: translateHotkeyDisplay, clearBtn: translateHotkeyClearBtn };
  } else if (target === 'toggle') {
    return { hidden: toggleTranslateHotkeyHiddenInput, display: toggleTranslateHotkeyDisplay, clearBtn: toggleTranslateHotkeyClearBtn };
  } else {
    return { hidden: hotkeyHiddenInput, display: hotkeyDisplay, clearBtn: hotkeyClearBtn };
  }
}

function renderHotkey(value, target = currentCapturingTarget || 'main') {
  const normalized = normalizeHotkeyValue(value);
  const elements = getHotkeyElements(target);
  if (elements.hidden) elements.hidden.value = normalized;
  if (!elements.display) return;
  if (!normalized) {
    elements.display.textContent = t('hotkeys.notset');
    elements.display.dataset.empty = "true";
  } else {
    elements.display.textContent = normalized;
    elements.display.dataset.empty = "false";
  }
}

function applyHotkeyRecordingStyles(active, previewText, target = currentCapturingTarget || 'main') {
  const elements = getHotkeyElements(target);
  if (elements.display) {
    elements.display.classList.toggle("capturing", active);
    if (active) {
      elements.display.textContent = previewText ?? t('hotkeys.press');
      elements.display.dataset.empty = "false";
    }
  }
}

function beginHotkeyCapture(target = 'main') {
  if (isCapturingHotkey) return;
  isCapturingHotkey = true;
  currentCapturingTarget = target;
  const elements = getHotkeyElements(target);
  hotkeyBeforeCapture = normalizeHotkeyValue(elements.hidden?.value);
  pressedModifiers.clear();
  applyHotkeyRecordingStyles(true, null, target);
}

function cancelHotkeyCapture() {
  if (!isCapturingHotkey) return;
  const target = currentCapturingTarget;
  isCapturingHotkey = false;
  currentCapturingTarget = null;
  pressedModifiers.clear();
  applyHotkeyRecordingStyles(false, null, target);
  renderHotkey(hotkeyBeforeCapture, target);
  hotkeyBeforeCapture = "";
}

function finishHotkeyCapture(binding) {
  if (!isCapturingHotkey) return;
  const target = currentCapturingTarget;
  isCapturingHotkey = false;
  currentCapturingTarget = null;
  pressedModifiers.clear();
  applyHotkeyRecordingStyles(false, null, target);
  const normalized = normalizeHotkeyValue(binding);
  if (normalized) {
    if (!bindingHasMainKey(normalized)) {
      showToast(t('toast.error.hotkey.key'), "error");
      renderHotkey(hotkeyBeforeCapture, target);
      return;
    }
    if (bindingUsesMouse(normalized)) {
      showToast(t('toast.error.hotkey.mouse'), "error");
      renderHotkey(hotkeyBeforeCapture, target);
      return;
    }
    renderHotkey(normalized, target);
    const payload = currentSettings();
    const successMsg = target === 'translate' ? t('toast.hotkey.translate.saved')
      : target === 'toggle' ? t('toast.hotkey.toggle.saved')
      : t('toast.hotkey.saved');
    persistSettings(payload, successMsg);
  } else {
    renderHotkey(hotkeyBeforeCapture, target);
  }
}

function modifierLabelFromCode(code) {
  if (!code) return null;
  if (code.startsWith("Control")) return "Ctrl";
  if (code.startsWith("Shift")) return "Shift";
  if (code.startsWith("Alt")) return "Alt";
  if (code.startsWith("Meta")) return "Win";
  return null;
}

function normalizeModifiers(modifiers) {
  const unique = new Set(modifiers);
  return MODIFIER_ORDER.filter(name => unique.has(name));
}

function updateHotkeyPreview() {
  if (!isCapturingHotkey) return;
  const modifiers = normalizeModifiers(Array.from(pressedModifiers));
  const preview = modifiers.length ? `${modifiers.join("+")} + …` : t('hotkeys.hold');
  applyHotkeyRecordingStyles(true, preview);
}

function isModifierKey(code) {
  return MODIFIER_CODES.has(code);
}

function keyCodeToHotkeyName(code) {
  if (!code) return null;
  if (/^F\d{1,2}$/i.test(code)) return code.toUpperCase();
  if (code.startsWith("Key")) return code.slice(3).toUpperCase();
  if (code.startsWith("Digit")) return code.slice(5);
  if (code.startsWith("Numpad")) {
    const suffix = code.slice(6);
    if (!suffix) return null;
    return `Numpad${suffix}`;
  }
  return KEY_CODE_LABELS[code] ?? null;
}

function formatKeyboardHotkey(event) {
  const keyName = keyCodeToHotkeyName(event.code);
  if (!keyName || MODIFIER_NAMES.has(keyName)) return "";
  const modifiers = normalizeModifiers(Array.from(pressedModifiers));
  return [...modifiers, keyName].join("+");
}

function formatMouseHotkey(event) {
  const buttonName = MOUSE_BUTTON_NAMES[event.button];
  if (!buttonName) return "";
  const modifiers = normalizeModifiers(Array.from(pressedModifiers));
  return [...modifiers, buttonName].join("+");
}

function bindingUsesMouse(binding) {
  return /Mouse/i.test(binding);
}

function bindingHasMainKey(binding) {
  return binding.split("+").map(p => p.trim()).filter(Boolean).some(p => !MODIFIER_NAMES.has(p));
}

// ============================================================================
// Provider & UI Sync
// ============================================================================

function getSelectedProvider() {
  for (const radio of providerRadios) {
    if (radio.checked) return radio.value;
  }
  return 'openai';
}

function setSelectedProvider(value) {
  for (const radio of providerRadios) {
    radio.checked = radio.value === value;
  }
}

function updateProviderFields() {
  const provider = getSelectedProvider();

  // Show only relevant API key field
  if (openaiApiKeyField) openaiApiKeyField.hidden = provider !== 'openai';
  if (groqApiKeyField) groqApiKeyField.hidden = provider !== 'groq';
  if (elevenlabsApiKeyField) elevenlabsApiKeyField.hidden = provider !== 'elevenlabs';

  // Update model options
  const currentModel = modelSelect?.value || '';
  if (!modelSelect) return;

  modelSelect.innerHTML = '';

  if (provider === 'groq') {
    modelSelect.innerHTML = `
      <option value="groq/whisper-large-v3-turbo">Whisper Large v3 Turbo</option>
      <option value="groq/whisper-large-v3">Whisper Large v3</option>
    `;
    modelSelect.value = currentModel.startsWith("groq/") ? currentModel : "groq/whisper-large-v3-turbo";
  } else if (provider === 'elevenlabs') {
    modelSelect.innerHTML = `<option value="scribe_v2_realtime">Scribe v2 Realtime</option>`;
    modelSelect.value = "scribe_v2_realtime";
  } else {
    modelSelect.innerHTML = `
      <option value="gpt-4o-transcribe">gpt-4o-transcribe</option>
      <option value="gpt-4o-mini-transcribe">gpt-4o-mini-transcribe</option>
      <option value="whisper-1">whisper-1 (fallback)</option>
    `;
    modelSelect.value = currentModel.startsWith("groq/") ? "gpt-4o-transcribe" : currentModel;
  }
}

function syncTranslationUi() {
  const enabled = autoTranslateInput?.checked;
  if (translationOptions) {
    translationOptions.classList.toggle('disabled', !enabled);
  }
}

function syncCustomInstructionsUi() {
  if (!customInstructionsWrapper || !customInstructionsInput) return;
  const enabled = Boolean(useCustomInstructionsInput?.checked);
  customInstructionsWrapper.hidden = !enabled;
  customInstructionsInput.disabled = !enabled;
}

// ============================================================================
// Vocabulary UI
// ============================================================================

function syncVocabularyUi() {
  if (!vocabularyWrapper || !customVocabularyInput) return;
  const enabled = Boolean(useVocabularyInput?.checked);
  vocabularyWrapper.hidden = !enabled;
  customVocabularyInput.disabled = !enabled;
  updateVocabularyCount();
}

function getVocabularyArray() {
  const text = customVocabularyInput?.value ?? "";
  return text.split('\n')
    .map(line => line.trim())
    .filter(line => line.length > 0);
}

function updateVocabularyCount() {
  if (!vocabularyCountEl) return;
  const count = getVocabularyArray().length;
  const termsText = vocabularyCountEl.querySelector('[data-i18n="vocabulary.terms"]');
  vocabularyCountEl.firstChild.textContent = count + ' ';
  if (termsText) termsText.textContent = t('vocabulary.terms');
}

async function importVocabulary() {
  const input = document.createElement('input');
  input.type = 'file';
  input.accept = '.txt';
  input.onchange = async (e) => {
    const file = e.target.files[0];
    if (!file) return;
    const text = await file.text();
    const existing = getVocabularyArray();
    const newTerms = text.split('\n').map(l => l.trim()).filter(l => l.length > 0);
    const merged = [...new Set([...existing, ...newTerms])];
    if (customVocabularyInput) customVocabularyInput.value = merged.join('\n');
    updateVocabularyCount();
  };
  input.click();
}

function exportVocabulary() {
  const terms = getVocabularyArray();
  if (terms.length === 0) return;
  const blob = new Blob([terms.join('\n')], { type: 'text/plain' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = 'vocabulary.txt';
  a.click();
  URL.revokeObjectURL(url);
}

// ============================================================================
// Toggle Password Visibility
// ============================================================================

function initPasswordToggles() {
  document.querySelectorAll('.toggle-password').forEach(btn => {
    btn.addEventListener('click', () => {
      const targetId = btn.dataset.target;
      const input = document.getElementById(targetId);
      if (!input) return;

      const isPassword = input.type === 'password';
      input.type = isPassword ? 'text' : 'password';
      btn.classList.toggle('active', !isPassword);
    });
  });
}

// ============================================================================
// Settings Load/Save
// ============================================================================

async function loadSettings() {
  dbg("loadSettings() called");
  if (!invoke) {
    dbg("invoke is not available in loadSettings", "warn");
    return;
  }
  try {
    const settings = await invoke("get_settings");
    dbg("invoke(get_settings) ok");
    initialSettings = { ...settings };

    // Provider
    setSelectedProvider(settings.provider ?? "openai");
    updateProviderFields();

    // API Keys
    if (apiKeyInput) apiKeyInput.value = settings.api_key ?? "";
    if (groqApiKeyInput) groqApiKeyInput.value = settings.groq_api_key ?? "";
    if (elevenlabsApiKeyInput) elevenlabsApiKeyInput.value = settings.elevenlabs_api_key ?? "";

    // Model
    if (modelSelect) modelSelect.value = settings.model ?? "gpt-4o-transcribe";

    // Hotkeys
    renderHotkey(settings.hotkey ?? DEFAULT_HOTKEY, 'main');
    renderHotkey(settings.translate_hotkey ?? "", 'translate');
    renderHotkey(settings.toggle_translate_hotkey ?? "", 'toggle');

    // Behavior
    if (simulateTypingInput) simulateTypingInput.checked = Boolean(settings.simulate_typing);
    if (copyToClipboardInput) copyToClipboardInput.checked = Boolean(settings.copy_to_clipboard);
    if (useStreamingInput) useStreamingInput.checked = Boolean(settings.use_streaming);

    // System
    if (autoStartInput) autoStartInput.checked = Boolean(settings.auto_start);
    if (startMinimizedInput) startMinimizedInput.checked = Boolean(settings.start_minimized);
    if (autoUpdateInput) autoUpdateInput.checked = Boolean(settings.auto_update ?? true);

    // Translation
    if (autoTranslateInput) autoTranslateInput.checked = Boolean(settings.auto_translate);
    if (targetLanguageSelect) targetLanguageSelect.value = settings.target_language ?? "русский";
    if (llmProviderSelect) llmProviderSelect.value = settings.llm_provider ?? "groq";

    // Custom instructions
    if (useCustomInstructionsInput) useCustomInstructionsInput.checked = Boolean(settings.use_custom_instructions);
    if (customInstructionsInput) customInstructionsInput.value = settings.custom_instructions ?? "";

    // Vocabulary
    if (useVocabularyInput) useVocabularyInput.checked = Boolean(settings.use_vocabulary);
    if (customVocabularyInput) {
      const vocab = settings.custom_vocabulary ?? [];
      customVocabularyInput.value = vocab.join('\n');
    }

    syncTranslationUi();
    syncCustomInstructionsUi();
    syncVocabularyUi();

    // Initialize ElevenLabs streaming if needed
    if (window.ElevenLabsSTT?.init) {
      await window.ElevenLabsSTT.init(settings);
    }
  } catch (error) {
    console.error(error);
    showToast(t('toast.error.load'), "error");
  }
}

function currentSettings() {
  return {
    provider: getSelectedProvider(),
    llm_provider: llmProviderSelect?.value ?? "groq",
    api_key: apiKeyInput?.value.trim() ?? "",
    groq_api_key: groqApiKeyInput?.value.trim() ?? "",
    elevenlabs_api_key: elevenlabsApiKeyInput?.value.trim() ?? "",
    model: modelSelect?.value ?? "gpt-4o-transcribe",
    hotkey: normalizeHotkeyValue(hotkeyHiddenInput?.value),
    translate_hotkey: normalizeHotkeyValue(translateHotkeyHiddenInput?.value),
    toggle_translate_hotkey: normalizeHotkeyValue(toggleTranslateHotkeyHiddenInput?.value),
    simulate_typing: simulateTypingInput?.checked ?? false,
    copy_to_clipboard: copyToClipboardInput?.checked ?? false,
    auto_start: autoStartInput?.checked ?? false,
    start_minimized: startMinimizedInput?.checked ?? false,
    auto_update: autoUpdateInput?.checked ?? true,
    use_streaming: useStreamingInput?.checked ?? false,
    auto_translate: autoTranslateInput?.checked ?? false,
    target_language: targetLanguageSelect?.value ?? "русский",
    use_custom_instructions: useCustomInstructionsInput?.checked ?? false,
    custom_instructions: (customInstructionsInput?.value ?? "").trim(),
    use_vocabulary: useVocabularyInput?.checked ?? false,
    custom_vocabulary: getVocabularyArray(),
  };
}

// ============================================================================
// Event Handlers
// ============================================================================

// Form submit
form?.addEventListener("submit", async (event) => {
  event.preventDefault();
  if (isCapturingHotkey) cancelHotkeyCapture();

  const payload = currentSettings();
  if (!payload.hotkey) {
    showToast(t('toast.error.hotkey.main'), "error");
    return;
  }
  if (!bindingHasMainKey(payload.hotkey)) {
    showToast(t('toast.error.hotkey.key'), "error");
    return;
  }

  const saved = await persistSettings(payload, t('toast.settings.saved'));
  if (saved && window.ElevenLabsSTT?.init) {
    await window.ElevenLabsSTT.init(payload);
  }
});

// Revert button
revertBtn?.addEventListener("click", () => {
  if (!initialSettings) return;
  cancelHotkeyCapture();

  setSelectedProvider(initialSettings.provider ?? "openai");
  updateProviderFields();

  if (apiKeyInput) apiKeyInput.value = initialSettings.api_key ?? "";
  if (groqApiKeyInput) groqApiKeyInput.value = initialSettings.groq_api_key ?? "";
  if (elevenlabsApiKeyInput) elevenlabsApiKeyInput.value = initialSettings.elevenlabs_api_key ?? "";
  if (modelSelect) modelSelect.value = initialSettings.model ?? "gpt-4o-transcribe";

  renderHotkey(initialSettings.hotkey ?? DEFAULT_HOTKEY, 'main');
  renderHotkey(initialSettings.translate_hotkey ?? "", 'translate');
  renderHotkey(initialSettings.toggle_translate_hotkey ?? "", 'toggle');

  if (simulateTypingInput) simulateTypingInput.checked = Boolean(initialSettings.simulate_typing);
  if (copyToClipboardInput) copyToClipboardInput.checked = Boolean(initialSettings.copy_to_clipboard);
  if (useStreamingInput) useStreamingInput.checked = Boolean(initialSettings.use_streaming);
  if (autoStartInput) autoStartInput.checked = Boolean(initialSettings.auto_start);
  if (startMinimizedInput) startMinimizedInput.checked = Boolean(initialSettings.start_minimized);
  if (autoUpdateInput) autoUpdateInput.checked = Boolean(initialSettings.auto_update ?? true);
  if (autoTranslateInput) autoTranslateInput.checked = Boolean(initialSettings.auto_translate);
  if (targetLanguageSelect) targetLanguageSelect.value = initialSettings.target_language ?? "русский";
  if (llmProviderSelect) llmProviderSelect.value = initialSettings.llm_provider ?? "groq";
  if (useCustomInstructionsInput) useCustomInstructionsInput.checked = Boolean(initialSettings.use_custom_instructions);
  if (customInstructionsInput) customInstructionsInput.value = initialSettings.custom_instructions ?? "";

  syncTranslationUi();
  syncCustomInstructionsUi();
  showToast(t('toast.changes.reverted'));
});

// Hotkey field clicks
hotkeyDisplay?.addEventListener("click", () => {
  isCapturingHotkey ? cancelHotkeyCapture() : beginHotkeyCapture('main');
});

translateHotkeyDisplay?.addEventListener("click", () => {
  isCapturingHotkey ? cancelHotkeyCapture() : beginHotkeyCapture('translate');
});

toggleTranslateHotkeyDisplay?.addEventListener("click", () => {
  isCapturingHotkey ? cancelHotkeyCapture() : beginHotkeyCapture('toggle');
});

// Clear hotkey buttons
function clearHotkey(target) {
  const elements = getHotkeyElements(target);
  if (elements.hidden) elements.hidden.value = "";
  renderHotkey("", target);
}

hotkeyClearBtn?.addEventListener("click", (e) => { e.stopPropagation(); clearHotkey('main'); });
translateHotkeyClearBtn?.addEventListener("click", (e) => { e.stopPropagation(); clearHotkey('translate'); });
toggleTranslateHotkeyClearBtn?.addEventListener("click", (e) => { e.stopPropagation(); clearHotkey('toggle'); });

// Provider change
providerRadios.forEach(radio => {
  radio.addEventListener("change", updateProviderFields);
});

// Translation toggle
autoTranslateInput?.addEventListener("change", syncTranslationUi);

// Custom instructions toggle
useCustomInstructionsInput?.addEventListener("change", syncCustomInstructionsUi);

// Vocabulary toggle and buttons
useVocabularyInput?.addEventListener("change", syncVocabularyUi);
customVocabularyInput?.addEventListener("input", updateVocabularyCount);
importVocabularyBtn?.addEventListener("click", importVocabulary);
exportVocabularyBtn?.addEventListener("click", exportVocabulary);

// Keyboard events for hotkey capture
window.addEventListener("keydown", (event) => {
  if (!isCapturingHotkey) return;
  event.preventDefault();
  event.stopPropagation();

  if (event.code === "Escape") {
    cancelHotkeyCapture();
    return;
  }
  if (event.repeat) return;

  if (isModifierKey(event.code)) {
    const label = modifierLabelFromCode(event.code);
    if (label) {
      pressedModifiers.add(label);
      updateHotkeyPreview();
    }
    return;
  }

  const binding = formatKeyboardHotkey(event);
  if (!binding) {
    showToast(t('toast.error.hotkey.recognize'), "error");
    cancelHotkeyCapture();
    return;
  }
  finishHotkeyCapture(binding);
});

window.addEventListener("keyup", (event) => {
  if (!isCapturingHotkey) return;
  if (!isModifierKey(event.code)) return;
  const label = modifierLabelFromCode(event.code);
  if (label) pressedModifiers.delete(label);
  updateHotkeyPreview();
});

window.addEventListener("mousedown", (event) => {
  if (!isCapturingHotkey) return;
  if (event.target === hotkeyDisplay || event.target === translateHotkeyDisplay ||
      event.target === toggleTranslateHotkeyDisplay || event.target === hotkeyClearBtn ||
      event.target === translateHotkeyClearBtn || event.target === toggleTranslateHotkeyClearBtn) return;
  event.preventDefault();
  event.stopPropagation();
  const binding = formatMouseHotkey(event);
  if (!binding) {
    showToast(t('toast.error.hotkey.recognize'), "error");
    cancelHotkeyCapture();
    return;
  }
  finishHotkeyCapture(binding);
});

window.addEventListener("blur", () => {
  if (isCapturingHotkey) cancelHotkeyCapture();
});

// ============================================================================
// History
// ============================================================================

async function loadHistory() {
  if (!invoke || !historyListEl) return;
  try {
    const history = await invoke("get_history");
    renderHistory(history);
  } catch (err) {
    console.error("[History] Failed to load:", err);
  }
}

function renderHistory(entries) {
  if (!historyListEl) return;

  if (!entries || entries.length === 0) {
    historyListEl.innerHTML = `<p class="history-empty">${t('history.empty')}</p>`;
    return;
  }

  historyListEl.innerHTML = entries.map(entry => {
    const time = formatHistoryTime(entry.timestamp);
    const hasTranslation = entry.translated_text && entry.translated_text !== entry.original_text;

    // Show translated text as main if available, otherwise original
    const mainText = hasTranslation ? escapeHtml(entry.translated_text) : escapeHtml(entry.original_text);
    const originalText = hasTranslation ? escapeHtml(entry.original_text) : null;

    // Provider badges
    let providerBadges = '';
    if (entry.transcription_provider) {
      const provider = entry.transcription_provider.toLowerCase();
      providerBadges += `<span class="history-entry-provider ${provider}">${provider}</span>`;
    }
    if (entry.llm_provider) {
      const llm = entry.llm_provider.toLowerCase();
      providerBadges += `<span class="history-entry-provider ${llm}">+${llm}</span>`;
    }
    if (entry.custom_instructions_used) {
      providerBadges += `<span class="history-entry-provider custom">custom</span>`;
    }

    // Language badges
    let langBadges = '';
    if (entry.source_language) {
      langBadges += `<span class="history-entry-lang">${entry.source_language}</span>`;
    }
    if (hasTranslation && entry.target_language) {
      langBadges += `<span class="history-entry-translated">→ ${entry.target_language}</span>`;
    }

    // Text to copy - prefer translated if available
    const textToCopy = hasTranslation ? entry.translated_text : entry.original_text;

    // Build original text row with its own copy button
    const originalRow = originalText ? `
      <div class="history-entry-original-row">
        <p class="history-entry-original">${originalText}</p>
        <button type="button" class="history-entry-btn copy-original" title="Копировать оригинал" data-text="${escapeAttr(entry.original_text)}">
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
            <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
          </svg>
        </button>
      </div>
    ` : '';

    return `
      <div class="history-entry" data-id="${entry.id}">
        <div class="history-entry-content">
          <p class="history-entry-text">${mainText}</p>
          ${originalRow}
          <div class="history-entry-meta">
            <span class="history-entry-time">${time}</span>
            ${providerBadges}
            ${langBadges}
          </div>
        </div>
        <div class="history-entry-actions">
          <button type="button" class="history-entry-btn copy" title="Копировать" data-text="${escapeAttr(textToCopy)}">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
              <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
            </svg>
          </button>
          <button type="button" class="history-entry-btn delete" title="Удалить" data-id="${entry.id}">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <line x1="18" y1="6" x2="6" y2="18"></line>
              <line x1="6" y1="6" x2="18" y2="18"></line>
            </svg>
          </button>
        </div>
      </div>
    `;
  }).join('');

  // Add event handlers
  historyListEl.querySelectorAll('.history-entry-btn.delete').forEach(btn => {
    btn.addEventListener('click', async () => {
      const id = parseInt(btn.dataset.id, 10);
      await deleteHistoryEntry(id);
    });
  });

  historyListEl.querySelectorAll('.history-entry-btn.copy').forEach(btn => {
    btn.addEventListener('click', () => {
      const text = btn.dataset.text;
      navigator.clipboard.writeText(text).then(() => {
        showToast(t('toast.copied'), "success");
      });
    });
  });

  historyListEl.querySelectorAll('.history-entry-btn.copy-original').forEach(btn => {
    btn.addEventListener('click', () => {
      const text = btn.dataset.text;
      navigator.clipboard.writeText(text).then(() => {
        showToast(t('toast.copied'), "success");
      });
    });
  });
}

function formatHistoryTime(timestamp) {
  try {
    const date = new Date(timestamp);
    const now = new Date();
    const isToday = date.toDateString() === now.toDateString();
    if (isToday) {
      return date.toLocaleTimeString('ru-RU', { hour: '2-digit', minute: '2-digit' });
    }
    return date.toLocaleDateString('ru-RU', {
      day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit'
    });
  } catch { return ''; }
}

function escapeHtml(text) {
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML;
}

function escapeAttr(text) {
  return text.replace(/"/g, '&quot;').replace(/'/g, '&#39;');
}

async function deleteHistoryEntry(id) {
  if (!invoke) return;
  try {
    await invoke("delete_history_entry", { id });
    await loadHistory();
  } catch (err) {
    console.error("[History] Failed to delete:", err);
    showToast(t('toast.error.delete'), "error");
  }
}

async function clearAllHistory() {
  if (!invoke) return;
  try {
    await invoke("clear_history");
    await loadHistory();
    showToast(t('toast.history.cleared'));
  } catch (err) {
    console.error("[History] Failed to clear:", err);
    showToast(t('toast.error.clear'), "error");
  }
}

clearHistoryBtn?.addEventListener("click", clearAllHistory);

// ============================================================================
// Initialization
// ============================================================================

window.addEventListener("DOMContentLoaded", async () => {
  if (!invoke || !listen || !tauriApp) {
    hydrateTauriApis();
  }

  // Initialize i18n
  if (window.i18n?.initI18n) {
    window.i18n.initI18n();
    window.i18n.applyTranslations();

    // Set UI language selector to current language
    if (uiLanguageSelect) {
      uiLanguageSelect.value = window.i18n.getLanguage();
    }
  }

  // UI language change handler
  uiLanguageSelect?.addEventListener('change', () => {
    if (window.i18n?.setLanguage) {
      window.i18n.setLanguage(uiLanguageSelect.value);
      // Re-render dynamic content
      renderHotkey(hotkeyHiddenInput?.value || '', 'main');
      renderHotkey(translateHotkeyHiddenInput?.value || '', 'translate');
      renderHotkey(toggleTranslateHotkeyHiddenInput?.value || '', 'toggle');
      loadHistory();
    }
  });

  // Initialize tabs
  initTabs();

  // Initialize password toggles
  initPasswordToggles();

  // Ping backend
  if (invoke) {
    try {
      const pong = await invoke("ping");
      dbg(`ping -> ${pong}`);
    } catch (e) {
      dbg(`ping failed: ${String(e)}`, "error");
    }
  }

  // Setup ElevenLabs
  if (window.ElevenLabsSTT?.setupEventListeners) {
    window.ElevenLabsSTT.setupEventListeners();
  }

  // Load settings
  await loadSettings();

  // Set version
  if (tauriApp?.getVersion) {
    try {
      const versionEl = document.getElementById("app-version");
      if (versionEl) versionEl.textContent = await tauriApp.getVersion();
    } catch (error) {
      console.error(error);
    }
  }

  // Setup event listeners
  if (listen) {
    await listen("transcription://status", ({ payload }) => {
      const { phase, message } = payload;
      if (phase === "recording") {
        setStatus("recording", message ?? t('status.recording'));
        if (progressEl) { progressEl.hidden = false; progressEl.removeAttribute("value"); }
      } else if (phase === "transcribing") {
        setStatus("transcribing", message ?? t('status.transcribing'));
        if (progressEl) { progressEl.hidden = false; progressEl.value = 0; }
      } else if (phase === "idle") {
        if (progressEl) progressEl.hidden = true;
        setStatus("idle", message ?? t('status.ready'));
      } else if (phase === "error") {
        if (progressEl) progressEl.hidden = true;
        setStatus("error", message ?? t('status.error'));
        showToast(message ?? t('toast.error'), "error");
      } else if (phase === "success") {
        if (progressEl) progressEl.hidden = true;
        setStatus("success", message ?? t('status.success'));
      }
    });

    await listen("transcription://partial", ({ payload }) => {
      if (!payload?.text || !resultEl) return;
      resultEl.hidden = false;
      resultEl.classList.add("partial");
      resultEl.textContent = payload.text;
      setStatus("recording", t('status.transcribing'));
    });

    await listen("transcription://complete", ({ payload }) => {
      if (resultEl) {
        resultEl.classList.remove("partial");
        if (payload?.text) {
          resultEl.hidden = false;
          resultEl.textContent = payload.text;
        }
      }
      showToast(t('status.success'), "success");
      setStatus("success", t('status.success'));
      loadHistory();
    });

    await listen("settings://changed", ({ payload }) => {
      const { auto_translate, target_language } = payload;
      if (autoTranslateInput && typeof auto_translate === 'boolean') {
        autoTranslateInput.checked = auto_translate;
      }
      if (targetLanguageSelect && target_language) {
        const options = targetLanguageSelect.options;
        for (let i = 0; i < options.length; i++) {
          if (options[i].value.toLowerCase() === target_language.toLowerCase()) {
            targetLanguageSelect.selectedIndex = i;
            break;
          }
        }
      }
      syncTranslationUi();
    });
  }

  // Load history
  await loadHistory();
});
