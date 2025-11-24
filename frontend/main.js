let invoke = null;
let listen = null;
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
  tauriApp = tauri.app ?? null;
  dbg(`TAURI wired: invoke=${!!invoke}, listen=${!!listen}, app=${!!tauriApp}`);
}

hydrateTauriApis();

const statusIndicator = document.getElementById("status-indicator");
const statusOrb = document.getElementById("status-orb");
const statusCard = document.getElementById("statusCard");
const statusText = document.getElementById("status-text");
const progressEl = document.getElementById("progress");
const resultEl = document.getElementById("last-result");
const toastEl = document.getElementById("toast");

const form = document.getElementById("settings-form");
const providerSelect = document.getElementById("provider");
const apiKeyInput = document.getElementById("apiKey");
const groqApiKeyInput = document.getElementById("groqApiKey");
const elevenlabsApiKeyInput = document.getElementById("elevenlabsApiKey");
const openaiApiKeyLabel = document.getElementById("openai-api-key-label");
const groqApiKeyLabel = document.getElementById("groq-api-key-label");
const elevenlabsApiKeyLabel = document.getElementById("elevenlabs-api-key-label");
const modelSelect = document.getElementById("model");
const hotkeyHiddenInput = document.getElementById("hotkey");
const hotkeyDisplay = document.getElementById("hotkeyDisplay");
const hotkeyRecordBtn = document.getElementById("startHotkeyCapture");
const simulateTypingInput = document.getElementById("simulateTyping");
const copyToClipboardInput = document.getElementById("copyToClipboard");
const autoStartInput = document.getElementById("autoStart");
const useStreamingInput = document.getElementById("useStreaming");
const autoTranslateInput = document.getElementById("autoTranslate");
const targetLanguageSelect = document.getElementById("targetLanguage");
const useCustomInstructionsInput = document.getElementById("useCustomInstructions");
const customInstructionsWrapper = document.getElementById("customInstructionsWrapper");
const customInstructionsInput = document.getElementById("customInstructions");
const llmProviderLabel = document.getElementById("llm-provider-label");
const llmProviderSelect = document.getElementById("llmProvider");
const revertBtn = document.getElementById("revertBtn");

const DEFAULT_HOTKEY = "Ctrl+Shift+Space";
const MODIFIER_ORDER = ["Ctrl", "Shift", "Alt", "Win"];
const MODIFIER_NAMES = new Set(MODIFIER_ORDER);
const MODIFIER_CODES = new Set([
  "ControlLeft",
  "ControlRight",
  "ShiftLeft",
  "ShiftRight",
  "AltLeft",
  "AltRight",
  "MetaLeft",
  "MetaRight",
]);
const KEY_CODE_LABELS = {
  Space: "Space",
  Escape: "Esc",
  Enter: "Enter",
  Tab: "Tab",
  Backspace: "Backspace",
  Delete: "Delete",
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  CapsLock: "CapsLock",
  PageUp: "PageUp",
  PageDown: "PageDown",
  Home: "Home",
  End: "End",
  Insert: "Insert",
  Pause: "Pause",
  PrintScreen: "PrintScreen",
  ScrollLock: "ScrollLock",
  ContextMenu: "ContextMenu",
  Backquote: "`",
  Minus: "-",
  Equal: "=",
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  IntlBackslash: "IntlBackslash",
  Semicolon: ";",
  Quote: "'",
  Comma: ",",
  Period: ".",
  Slash: "/",
};
const MOUSE_BUTTON_NAMES = {
  0: "MouseLeft",
  1: "MouseMiddle",
  2: "MouseRight",
  3: "MouseButton4",
  4: "MouseButton5",
};

let initialSettings = null;
let isCapturingHotkey = false;
let hotkeyBeforeCapture = "";
const pressedModifiers = new Set();

function showToast(message, type = "info") {
  if (!toastEl) return;
  toastEl.textContent = message;
  toastEl.dataset.type = type;
  toastEl.hidden = false;
  setTimeout(() => {
    toastEl.hidden = true;
  }, 2800);
}

async function persistSettings(payload, successMessage = "Сохранено") {
  console.log(`[PERSIST] Calling persistSettings with:`, payload);
  dbg(`persistSettings: ${JSON.stringify(payload)}`);
  if (!invoke) {
    console.error("[PERSIST] invoke is not available!");
    dbg("invoke is not available in persistSettings", "error");
    return false;
  }
  try {
    console.log("[PERSIST] Calling invoke('save_settings')...");
    dbg("invoke(save_settings) ...");
    await invoke("save_settings", { settings: payload });
    console.log("[PERSIST] invoke('save_settings') succeeded!");
    dbg("invoke(save_settings) ok");
    initialSettings = { ...payload };
    if (successMessage) showToast(successMessage);
    return true;
  } catch (error) {
    console.error("[PERSIST] save_settings failed:", error);
    console.error(error);
    dbg(`save_settings failed: ${String(error)}`, "error");
    showToast("Ошибка при сохранении", "error");
    return false;
  }
}

function setStatus(state, text) {
  // Update orb state
  if (statusOrb) {
    statusOrb.className = `status-orb ${state}`;
  }
  // Update card active state for scan animation
  if (statusCard) {
    statusCard.classList.toggle("active", state === "recording" || state === "transcribing");
  }
  // Update pill indicator (hidden but kept for compatibility)
  if (statusIndicator) {
    statusIndicator.className = `pill ${state}`;
    statusIndicator.textContent =
      state === "recording"
        ? "Запись"
        : state === "transcribing"
          ? "Отправка"
          : state === "success"
            ? "Готово"
            : state === "error"
              ? "Ошибка"
              : "Ожидает";
  }
  if (statusText) statusText.textContent = text;
}

function normalizeHotkeyValue(value) {
  return (value ?? "").trim();
}

function renderHotkey(value) {
  const normalized = normalizeHotkeyValue(value);
  if (hotkeyHiddenInput) {
    hotkeyHiddenInput.value = normalized;
  }
  if (!hotkeyDisplay) return;
  if (!normalized) {
    hotkeyDisplay.textContent = hotkeyDisplay.dataset.placeholder ?? "";
    hotkeyDisplay.dataset.empty = "true";
  } else {
    hotkeyDisplay.textContent = normalized;
    hotkeyDisplay.dataset.empty = "false";
  }
}

function applyHotkeyRecordingStyles(active, previewText) {
  if (hotkeyRecordBtn) {
    hotkeyRecordBtn.classList.toggle("recording", active);
    hotkeyRecordBtn.textContent = active ? "Слушаю..." : "Записать";
  }
  if (active && hotkeyDisplay) {
    hotkeyDisplay.textContent = previewText ?? "Удерживайте клавиши";
    hotkeyDisplay.dataset.empty = "false";
  }
}

function beginHotkeyCapture() {
  if (isCapturingHotkey) return;
  isCapturingHotkey = true;
  hotkeyBeforeCapture = normalizeHotkeyValue(hotkeyHiddenInput?.value);
  pressedModifiers.clear();
  applyHotkeyRecordingStyles(true);
}

function cancelHotkeyCapture() {
  if (!isCapturingHotkey) return;
  isCapturingHotkey = false;
  pressedModifiers.clear();
  applyHotkeyRecordingStyles(false);
  renderHotkey(hotkeyBeforeCapture);
  hotkeyBeforeCapture = ""; // Clear to prevent memory leaks
}

function finishHotkeyCapture(binding) {
  console.log(`[HOTKEY] finishHotkeyCapture called with binding: ${binding}`);
  if (!isCapturingHotkey) return;
  isCapturingHotkey = false;
  pressedModifiers.clear();
  applyHotkeyRecordingStyles(false);
  const normalized = normalizeHotkeyValue(binding);
  console.log(`[HOTKEY] Normalized binding: ${normalized}`);
  if (normalized) {
    if (!bindingHasMainKey(normalized)) {
      showToast("Сочетание должно содержать основную клавишу", "error");
      renderHotkey(hotkeyBeforeCapture);
      return;
    }
    if (bindingUsesMouse(normalized)) {
      showToast("Глобальные шорткаты мыши не поддерживаются Windows", "error");
      renderHotkey(hotkeyBeforeCapture);
      return;
    }
    renderHotkey(normalized);
    const payload = currentSettings();
    console.log(`[HOTKEY] Calling persistSettings with payload:`, payload);
    persistSettings(payload, "Горячая клавиша обновлена");
  } else {
    renderHotkey(hotkeyBeforeCapture);
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
  return MODIFIER_ORDER.filter((name) => unique.has(name));
}

function updateHotkeyPreview() {
  if (!isCapturingHotkey) return;
  const modifiers = normalizeModifiers(Array.from(pressedModifiers));
  const preview = modifiers.length
    ? `${modifiers.join("+")} + …`
    : "Удерживайте клавиши";
  applyHotkeyRecordingStyles(true, preview);
}

function isModifierKey(code) {
  return MODIFIER_CODES.has(code);
}

function keyCodeToHotkeyName(code) {
  if (!code) return null;
  if (/^F\d{1,2}$/i.test(code)) {
    return code.toUpperCase();
  }
  if (code.startsWith("Key")) {
    return code.slice(3).toUpperCase();
  }
  if (code.startsWith("Digit")) {
    return code.slice(5);
  }
  if (code.startsWith("Numpad")) {
    const suffix = code.slice(6);
    if (!suffix) return null;
    if (/^\d$/.test(suffix)) {
      return `Numpad${suffix}`;
    }
    return `Numpad${suffix}`;
  }
  return KEY_CODE_LABELS[code] ?? null;
}

function formatKeyboardHotkey(event) {
  const keyName = keyCodeToHotkeyName(event.code);
  if (!keyName || MODIFIER_NAMES.has(keyName)) {
    return "";
  }
  const modifiers = normalizeModifiers(Array.from(pressedModifiers));
  return [...modifiers, keyName].join("+");
}

function formatMouseHotkey(event) {
  const buttonName = MOUSE_BUTTON_NAMES[event.button];
  if (!buttonName) {
    return "";
  }
  const modifiers = normalizeModifiers(Array.from(pressedModifiers));
  return [...modifiers, buttonName].join("+");
}

function bindingUsesMouse(binding) {
  return /Mouse/i.test(binding);
}

function bindingHasMainKey(binding) {
  return binding
    .split("+")
    .map((part) => part.trim())
    .filter(Boolean)
    .some((part) => !MODIFIER_NAMES.has(part));
}

function syncTranslationUi() {
  if (!targetLanguageSelect) return;
  const enabled = Boolean(autoTranslateInput?.checked);
  targetLanguageSelect.disabled = !enabled;
  targetLanguageSelect.classList.toggle("is-disabled", !enabled);
  updateLLMProviderVisibility();
}

function updateLLMProviderVisibility() {
  const needsLLM = autoTranslateInput?.checked || useCustomInstructionsInput?.checked;
  if (llmProviderLabel) {
    llmProviderLabel.hidden = !needsLLM;
  }
}

function syncCustomInstructionsUi() {
  if (!customInstructionsWrapper || !customInstructionsInput) return;
  const enabled = Boolean(useCustomInstructionsInput?.checked);
  customInstructionsWrapper.hidden = !enabled;
  customInstructionsInput.disabled = !enabled;
  updateLLMProviderVisibility();
}

function updateProviderFields() {
  const provider = providerSelect?.value;
  if (!provider) return;

  const isGroq = provider === "groq";
  const isElevenLabs = provider === "elevenlabs";

  // Update API key labels visibility
  if (openaiApiKeyLabel) openaiApiKeyLabel.hidden = isElevenLabs; // Hide OpenAI key for ElevenLabs
  if (groqApiKeyLabel) groqApiKeyLabel.hidden = !isGroq;
  if (elevenlabsApiKeyLabel) elevenlabsApiKeyLabel.hidden = !isElevenLabs;

  // Update hint for OpenAI API key
  const openaiHint = document.getElementById("openai-api-hint");
  if (openaiHint) {
    openaiHint.textContent = isGroq ? "(для перевода и инструкций)" : "";
  }

  // Update model options based on provider
  const currentModel = modelSelect.value;
  modelSelect.innerHTML = '';

  if (isGroq) {
    modelSelect.innerHTML = `
      <option value="groq/whisper-large-v3-turbo">Whisper Large v3 Turbo</option>
      <option value="groq/whisper-large-v3">Whisper Large v3</option>
    `;
    if (!currentModel.startsWith("groq/")) {
      modelSelect.value = "groq/whisper-large-v3-turbo";
    } else {
      modelSelect.value = currentModel;
    }
  } else if (isElevenLabs) {
    modelSelect.innerHTML = `
      <option value="scribe_v2_realtime">Scribe v2 Realtime</option>
    `;
    modelSelect.value = "scribe_v2_realtime";
  } else {
    modelSelect.innerHTML = `
      <option value="gpt-4o-transcribe">gpt-4o-transcribe</option>
      <option value="gpt-4o-mini-transcribe">gpt-4o-mini-transcribe</option>
      <option value="whisper-1">whisper-1 (fallback)</option>
    `;
    if (currentModel.startsWith("groq/")) {
      modelSelect.value = "gpt-4o-transcribe";
    } else {
      modelSelect.value = currentModel;
    }
  }
}

syncTranslationUi();
syncCustomInstructionsUi();

async function loadSettings() {
  dbg("loadSettings() called");
  if (!invoke) {
    dbg("invoke is not available in loadSettings", "warn");
    return;
  }
  try {
    dbg("invoke(get_settings) ...");
    const settings = await invoke("get_settings");
    dbg("invoke(get_settings) ok");
    initialSettings = { ...settings };
    providerSelect.value = settings.provider ?? "openai";
    if (llmProviderSelect) llmProviderSelect.value = settings.llm_provider ?? "openai";
    apiKeyInput.value = settings.api_key ?? "";
    groqApiKeyInput.value = settings.groq_api_key ?? "";
    elevenlabsApiKeyInput.value = settings.elevenlabs_api_key ?? "";
    modelSelect.value = settings.model ?? "gpt-4o-transcribe";
    updateProviderFields();
    renderHotkey(settings.hotkey ?? DEFAULT_HOTKEY);
    simulateTypingInput.checked = Boolean(settings.simulate_typing);
    copyToClipboardInput.checked = Boolean(settings.copy_to_clipboard);
    autoStartInput.checked = Boolean(settings.auto_start);
    useStreamingInput.checked = Boolean(settings.use_streaming);
    autoTranslateInput.checked = Boolean(settings.auto_translate);
    targetLanguageSelect.value = settings.target_language ?? "русский";
    if (useCustomInstructionsInput) {
      useCustomInstructionsInput.checked = Boolean(settings.use_custom_instructions);
    }
    if (customInstructionsInput) {
      customInstructionsInput.value = settings.custom_instructions ?? "";
    }
    syncTranslationUi();
    syncCustomInstructionsUi();

    // Initialize ElevenLabs streaming if provider is ElevenLabs
    if (window.ElevenLabsSTT?.init) {
      dbg(`Calling ElevenLabsSTT.init with provider: ${settings.provider}`);
      await window.ElevenLabsSTT.init(settings);
    } else {
      dbg("ElevenLabsSTT.init is not available", "warn");
    }
  } catch (error) {
    console.error(error);
    showToast("Не удалось загрузить настройки", "error");
  }
}

function currentSettings() {
  return {
    provider: providerSelect.value,
    llm_provider: llmProviderSelect?.value ?? "openai",
    api_key: apiKeyInput.value.trim(),
    groq_api_key: groqApiKeyInput.value.trim(),
    elevenlabs_api_key: elevenlabsApiKeyInput.value.trim(),
    model: modelSelect.value,
    hotkey: normalizeHotkeyValue(hotkeyHiddenInput?.value),
    simulate_typing: simulateTypingInput.checked,
    copy_to_clipboard: copyToClipboardInput.checked,
    auto_start: autoStartInput.checked,
    use_streaming: useStreamingInput.checked,
    auto_translate: autoTranslateInput.checked,
    target_language: targetLanguageSelect.value,
    use_custom_instructions: useCustomInstructionsInput?.checked ?? false,
    custom_instructions: (customInstructionsInput?.value ?? "").trim(),
  };
}

form?.addEventListener("submit", async (event) => {
  event.preventDefault();
  dbg("Form submit clicked");
  if (isCapturingHotkey) {
    cancelHotkeyCapture();
  }
  const payload = currentSettings();
  dbg(`currentSettings on submit: ${JSON.stringify(payload)}`);
  if (!payload.hotkey) {
    showToast("Сочетание клавиш не выбрано", "error");
    return;
  }
  if (!bindingHasMainKey(payload.hotkey)) {
    showToast("Сочетание должно содержать основную клавишу", "error");
    return;
  }
  if (bindingUsesMouse(payload.hotkey)) {
    showToast("Глобальные шорткаты мыши не поддерживаются Windows", "error");
    return;
  }
  const saved = await persistSettings(payload, "Сохранено");
  if (saved) {
    renderHotkey(payload.hotkey);
    // Reinitialize ElevenLabs streaming if settings changed
    if (window.ElevenLabsSTT?.init) {
      await window.ElevenLabsSTT.init(payload);
    }
  }
});

revertBtn?.addEventListener("click", () => {
  if (!initialSettings) return;
  cancelHotkeyCapture();
  providerSelect.value = initialSettings.provider ?? "openai";
  if (llmProviderSelect) llmProviderSelect.value = initialSettings.llm_provider ?? "openai";
  apiKeyInput.value = initialSettings.api_key ?? "";
  groqApiKeyInput.value = initialSettings.groq_api_key ?? "";
  elevenlabsApiKeyInput.value = initialSettings.elevenlabs_api_key ?? "";
  modelSelect.value = initialSettings.model ?? "gpt-4o-transcribe";
  updateProviderFields();
  renderHotkey(initialSettings.hotkey ?? DEFAULT_HOTKEY);
  simulateTypingInput.checked = Boolean(initialSettings.simulate_typing);
  copyToClipboardInput.checked = Boolean(initialSettings.copy_to_clipboard);
  autoStartInput.checked = Boolean(initialSettings.auto_start);
  useStreamingInput.checked = Boolean(initialSettings.use_streaming);
  autoTranslateInput.checked = Boolean(initialSettings.auto_translate);
  targetLanguageSelect.value = initialSettings.target_language ?? "русский";
  if (useCustomInstructionsInput) {
    useCustomInstructionsInput.checked = Boolean(initialSettings.use_custom_instructions);
  }
  if (customInstructionsInput) {
    customInstructionsInput.value = initialSettings.custom_instructions ?? "";
  }
  syncTranslationUi();
  syncCustomInstructionsUi();
  showToast("Изменения отменены");
});

hotkeyRecordBtn?.addEventListener("click", () => {
  dbg("Record button clicked");
  if (isCapturingHotkey) {
    cancelHotkeyCapture();
  } else {
    beginHotkeyCapture();
  }
});

autoTranslateInput?.addEventListener("change", () => {
  syncTranslationUi();
});

useCustomInstructionsInput?.addEventListener("change", () => {
  syncCustomInstructionsUi();
});

providerSelect?.addEventListener("change", () => {
  updateProviderFields();
});

window.addEventListener("keydown", (event) => {
  dbg(`keydown code=${event.code} ctrl=${event.ctrlKey} shift=${event.shiftKey} alt=${event.altKey} meta=${event.metaKey}`);
  if (!isCapturingHotkey) return;
  event.preventDefault();
  event.stopPropagation();

  if (event.code === "Escape") {
    cancelHotkeyCapture();
    return;
  }

  if (event.repeat) {
    return;
  }

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
    showToast("Не удалось распознать клавишу", "error");
    cancelHotkeyCapture();
    return;
  }
  finishHotkeyCapture(binding);
});

window.addEventListener("keyup", (event) => {
  if (!isCapturingHotkey) return;
  if (!isModifierKey(event.code)) return;
  const label = modifierLabelFromCode(event.code);
  if (!label) return;
  pressedModifiers.delete(label);
  updateHotkeyPreview();
});

window.addEventListener("mousedown", (event) => {
  dbg(`mousedown button=${event.button}`);
  if (!isCapturingHotkey) return;
  if (event.target === hotkeyRecordBtn) return;
  event.preventDefault();
  event.stopPropagation();
  const binding = formatMouseHotkey(event);
  if (!binding) {
    showToast("Не удалось распознать кнопку мыши", "error");
    cancelHotkeyCapture();
    return;
  }
  finishHotkeyCapture(binding);
});

window.addEventListener("blur", () => {
  if (isCapturingHotkey) {
    cancelHotkeyCapture();
  }
});

window.addEventListener("DOMContentLoaded", async () => {
  if (!invoke || !listen || !tauriApp) {
    hydrateTauriApis();
  }
  if (invoke) {
    try {
      const pong = await invoke("ping");
      dbg(`ping -> ${pong}`);
    } catch (e) {
      dbg(`ping failed: ${String(e)}`, "error");
    }
  } else {
    dbg("invoke not available at DOMContentLoaded", "warn");
  }

  // Setup ElevenLabs event listeners
  if (window.ElevenLabsSTT?.setupEventListeners) {
    window.ElevenLabsSTT.setupEventListeners();
  }

  await loadSettings();
  if (tauriApp?.getVersion) {
    try {
      const versionEl = document.getElementById("app-version");
      if (versionEl) {
        versionEl.textContent = await tauriApp.getVersion();
      }
    } catch (error) {
      console.error(error);
    }
  }

  if (listen) {
    await listen("transcription://status", ({ payload }) => {
      const { phase, message } = payload;
      if (phase === "recording") {
        setStatus("recording", message ?? "Идёт запись...");
        progressEl.hidden = false;
        progressEl.removeAttribute("value");
      } else if (phase === "transcribing") {
        setStatus("transcribing", message ?? "Отправка и распознавание...");
        progressEl.hidden = false;
        progressEl.value = 0;
      } else if (phase === "idle") {
        progressEl.hidden = true;
        setStatus("idle", message ?? "Готово к записи");
      } else if (phase === "error") {
        progressEl.hidden = true;
        setStatus("error", message ?? "Ошибка");
        showToast(message ?? "Ошибка", "error");
      } else if (phase === "success") {
        progressEl.hidden = true;
        setStatus("success", message ?? "Готово");
      }
    });

    await listen("transcription://partial", ({ payload }) => {
      if (!payload?.text || !resultEl) return;
      resultEl.hidden = false;
      resultEl.classList.add("partial");
      resultEl.textContent = payload.text;
      setStatus("recording", "Распознаю...");
    });

    await listen("transcription://complete", ({ payload }) => {
      if (resultEl) {
        resultEl.classList.remove("partial");
        if (payload?.text) {
          resultEl.hidden = false;
          resultEl.textContent = payload.text;
        }
      }
      showToast("Готово", "success");
      setStatus("success", "Обработка завершена");
    });
  }
});
