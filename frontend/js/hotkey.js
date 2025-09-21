import {
  DEFAULT_HOTKEY,
  MODIFIER_NAMES,
  MODIFIER_ORDER,
  MODIFIER_CODES,
  KEY_CODE_LABELS,
  MOUSE_BUTTON_NAMES,
} from "./constants.js";

export class HotkeyController {
  constructor({ button, display, hiddenInput, toast, settings, logger }) {
    this.button = button;
    this.display = display;
    this.hiddenInput = hiddenInput;
    this.toast = toast;
    this.settings = settings;
    this.logger = logger ?? (() => {});

    this.isCapturing = false;
    this.pressedModifiers = new Set();
    this.hotkeyBeforeCapture = "";

    this.registerButtonListener();
  }

  registerButtonListener() {
    this.button?.addEventListener("click", () => {
      if (this.isCapturing) {
        this.cancel();
      } else {
        this.begin();
      }
    });
  }

  registerWindowListeners() {
    window.addEventListener("keydown", (event) => this.handleKeydown(event));
    window.addEventListener("keyup", (event) => this.handleKeyup(event));
    window.addEventListener("mousedown", (event) => this.handleMousedown(event));
    window.addEventListener("blur", () => this.handleWindowBlur());
  }

  currentHotkey() {
    return (this.hiddenInput?.value ?? DEFAULT_HOTKEY).trim() || DEFAULT_HOTKEY;
  }

  render(value) {
    const normalized = (value ?? "").trim();
    if (this.hiddenInput) {
      this.hiddenInput.value = normalized;
    }
    if (!this.display) {
      return;
    }
    if (!normalized) {
      this.display.textContent = this.display.dataset.placeholder ?? "";
      this.display.dataset.empty = "true";
    } else {
      this.display.textContent = normalized;
      this.display.dataset.empty = "false";
    }
  }

  begin() {
    if (this.isCapturing) {
      return;
    }
    this.logger("Begin hotkey capture");
    this.isCapturing = true;
    this.hotkeyBeforeCapture = this.currentHotkey();
    this.pressedModifiers.clear();
    this.applyRecordingStyles(true);
  }

  cancel() {
    if (!this.isCapturing) {
      return;
    }
    this.logger("Cancel hotkey capture");
    this.isCapturing = false;
    this.pressedModifiers.clear();
    this.applyRecordingStyles(false);
    this.render(this.hotkeyBeforeCapture);
    this.hotkeyBeforeCapture = "";
  }

  async finish(binding) {
    if (!this.isCapturing) {
      return;
    }
    this.logger(Finish hotkey capture with );
    this.isCapturing = false;
    this.pressedModifiers.clear();
    this.applyRecordingStyles(false);

    const normalized = (binding ?? "").trim();
    if (!normalized) {
      this.render(this.hotkeyBeforeCapture);
      this.hotkeyBeforeCapture = "";
      return;
    }

    if (!this.bindingHasMainKey(normalized)) {
      this.toast.show("Hotkey must contain a non-modifier key", "error");
      this.render(this.hotkeyBeforeCapture);
      this.hotkeyBeforeCapture = "";
      return;
    }

    if (this.bindingUsesMouse(normalized)) {
      this.toast.show("Mouse buttons are not supported for hotkeys", "error");
      this.render(this.hotkeyBeforeCapture);
      this.hotkeyBeforeCapture = "";
      return;
    }

    this.render(normalized);
    const payload = this.settings.current();
    payload.hotkey = normalized;
    const saved = await this.settings.persist(payload, "Hotkey saved");
    if (!saved) {
      this.render(this.hotkeyBeforeCapture);
    }
    this.hotkeyBeforeCapture = "";
  }

  applyRecordingStyles(active) {
    if (this.button) {
      this.button.classList.toggle("recording", active);
      this.button.textContent = active ? "Listening..." : this.button.dataset.labelReady ?? this.button.textContent;
    }
    if (active && this.display) {
      const modifiers = Array.from(this.pressedModifiers).sort(
        (a, b) => MODIFIER_ORDER.indexOf(a) - MODIFIER_ORDER.indexOf(b),
      );
      const prefix = modifiers.length ? ${modifiers.join("+")}+ : "";
      this.display.textContent = prefix + "…";
      this.display.dataset.empty = "false";
    }
  }

  handleKeydown(event) {
    this.logger(
      keydown code= ctrl= shift= alt= meta=,
      "debug",
    );
    if (!this.isCapturing) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();

    if (event.code === "Escape") {
      this.cancel();
      return;
    }

    if (event.repeat) {
      return;
    }

    if (this.isModifierKey(event.code)) {
      const label = this.modifierLabelFromCode(event.code);
      if (label) {
        this.pressedModifiers.add(label);
        this.updateHotkeyPreview();
      }
      return;
    }

    const binding = this.formatKeyboardHotkey(event);
    if (!binding) {
      this.toast.show("Unable to capture that key", "error");
      this.cancel();
      return;
    }
    this.finish(binding);
  }

  handleKeyup(event) {
    if (!this.isCapturing || !this.isModifierKey(event.code)) {
      return;
    }
    const label = this.modifierLabelFromCode(event.code);
    if (!label) {
      return;
    }
    this.pressedModifiers.delete(label);
    this.updateHotkeyPreview();
  }

  handleMousedown(event) {
    this.logger(mousedown button=);
    if (!this.isCapturing || event.target === this.button) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    const binding = this.formatMouseHotkey(event);
    if (!binding) {
      this.toast.show("Unable to capture that mouse button", "error");
      this.cancel();
      return;
    }
    this.finish(binding);
  }

  handleWindowBlur() {
    if (this.isCapturing) {
      this.cancel();
    }
  }

  updateHotkeyPreview() {
    if (!this.display) {
      return;
    }
    const modifiers = Array.from(this.pressedModifiers).sort(
      (a, b) => MODIFIER_ORDER.indexOf(a) - MODIFIER_ORDER.indexOf(b),
    );
    const prefix = modifiers.length ? ${modifiers.join("+")}+ : "";
    this.display.textContent = prefix || this.display.dataset.placeholder ?? "";
    this.display.dataset.empty = prefix ? "false" : "true";
  }

  isModifierKey(code) {
    return MODIFIER_CODES.has(code);
  }

  modifierLabelFromCode(code) {
    switch (code) {
      case "ControlLeft":
      case "ControlRight":
        return "Ctrl";
      case "ShiftLeft":
      case "ShiftRight":
        return "Shift";
      case "AltLeft":
      case "AltRight":
        return "Alt";
      case "MetaLeft":
      case "MetaRight":
        return "Win";
      default:
        return null;
    }
  }

  formatKeyboardHotkey(event) {
    const keyName = this.keyCodeToName(event.code);
    if (!keyName || MODIFIER_NAMES.has(keyName)) {
      return "";
    }
    const modifiers = Array.from(this.pressedModifiers).sort(
      (a, b) => MODIFIER_ORDER.indexOf(a) - MODIFIER_ORDER.indexOf(b),
    );
    return [...modifiers, keyName].join("+");
  }

  formatMouseHotkey(event) {
    const buttonName = MOUSE_BUTTON_NAMES[event.button];
    if (!buttonName) {
      return "";
    }
    const modifiers = Array.from(this.pressedModifiers).sort(
      (a, b) => MODIFIER_ORDER.indexOf(a) - MODIFIER_ORDER.indexOf(b),
    );
    return [...modifiers, buttonName].join("+");
  }

  keyCodeToName(code) {
    if (KEY_CODE_LABELS[code]) {
      return KEY_CODE_LABELS[code];
    }
    if (code.startsWith("Key")) {
      return code.slice(3);
    }
    if (code.startsWith("Digit")) {
      return code.slice(5);
    }
    if (/^F\d+$/.test(code)) {
      return code;
    }
    return "";
  }

  bindingUsesMouse(binding) {
    return /Mouse/i.test(binding);
  }

  bindingHasMainKey(binding) {
    return binding
      .split("+")
      .map((part) => part.trim())
      .filter(Boolean)
      .some((part) => !MODIFIER_NAMES.has(part));
  }
}
