export const DEFAULT_HOTKEY = "Ctrl+Shift+Space";

export const HOTKEY_LABELS = Object.freeze({
  recording: "Recording",
  transcribing: "Transcribing",
  success: "Success",
  error: "Error",
  idle: "Idle",
});

export const MODIFIER_ORDER = Object.freeze(["Ctrl", "Shift", "Alt", "Win"]);
export const MODIFIER_NAMES = new Set(MODIFIER_ORDER);
export const MODIFIER_CODES = new Set([
  "ControlLeft",
  "ControlRight",
  "ShiftLeft",
  "ShiftRight",
  "AltLeft",
  "AltRight",
  "MetaLeft",
  "MetaRight",
]);

export const KEY_CODE_LABELS = Object.freeze({
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
  Backquote: "",
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
});

export const MOUSE_BUTTON_NAMES = Object.freeze({
  0: "MouseLeft",
  1: "MouseMiddle",
  2: "MouseRight",
  3: "MouseButton4",
  4: "MouseButton5",
});

export const PROVIDER_MODEL_OPTIONS = Object.freeze({
  openai: [
    { value: "gpt-4o-transcribe", label: "gpt-4o-transcribe" },
    { value: "gpt-4o-mini-transcribe", label: "gpt-4o-mini-transcribe" },
    { value: "whisper-1", label: "whisper-1 (fallback)" },
  ],
  groq: [
    { value: "groq/whisper-large-v3-turbo", label: "Whisper Large v3 Turbo" },
    { value: "groq/whisper-large-v3", label: "Whisper Large v3" },
  ],
});
