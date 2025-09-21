import { HOTKEY_LABELS } from "./constants.js";

export class StatusController {
  constructor({ indicator, text, progress, result }) {
    this.indicator = indicator;
    this.text = text;
    this.progress = progress;
    this.result = result;
  }

  set(phase, message) {
    if (this.indicator) {
      this.indicator.className = pill ;
      this.indicator.textContent = HOTKEY_LABELS[phase] ?? HOTKEY_LABELS.idle;
    }
    if (this.text) {
      this.text.textContent = message;
    }
  }

  showProgress(state) {
    if (!this.progress) {
      return;
    }
    if (!state) {
      this.progress.hidden = true;
      this.progress.removeAttribute("value");
      return;
    }
    this.progress.hidden = false;
    if (state.indeterminate) {
      this.progress.removeAttribute("value");
    } else if (typeof state.value === "number") {
      this.progress.value = state.value;
    }
  }

  showResult(text) {
    if (!this.result) {
      return;
    }
    this.result.hidden = false;
    this.result.textContent = text;
  }

  clearResult() {
    if (!this.result) {
      return;
    }
    this.result.hidden = true;
    this.result.textContent = "";
  }
}
