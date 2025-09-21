import { TauriBridge } from "./bridge.js";
import { createLogger } from "./logger.js";
import { ToastController } from "./toast.js";
import { StatusController } from "./status.js";
import { SettingsController } from "./settings.js";
import { HotkeyController } from "./hotkey.js";

export class AppController {
  constructor() {
    this.bridge = new TauriBridge();
    this.logger = createLogger(this.bridge);

    this.toast = new ToastController(document.getElementById("toast"));
    this.status = new StatusController({
      indicator: document.getElementById("status-indicator"),
      text: document.getElementById("status-text"),
      progress: document.getElementById("progress"),
      result: document.getElementById("last-result"),
    });

    this.settings = new SettingsController({
      bridge: this.bridge,
      toast: this.toast,
      logger: this.logger,
      elements: {
        form: document.getElementById("settings-form"),
        providerSelect: document.getElementById("provider"),
        apiKeyInput: document.getElementById("apiKey"),
        groqApiKeyInput: document.getElementById("groqApiKey"),
        openaiApiKeyLabel: document.getElementById("openai-api-key-label"),
        groqApiKeyLabel: document.getElementById("groq-api-key-label"),
        modelSelect: document.getElementById("model"),
        simulateTypingInput: document.getElementById("simulateTyping"),
        copyToClipboardInput: document.getElementById("copyToClipboard"),
        autoStartInput: document.getElementById("autoStart"),
        useStreamingInput: document.getElementById("useStreaming"),
        autoTranslateInput: document.getElementById("autoTranslate"),
        targetLanguageSelect: document.getElementById("targetLanguage"),
        useCustomInstructionsInput: document.getElementById("useCustomInstructions"),
        customInstructionsWrapper: document.getElementById("customInstructionsWrapper"),
        customInstructionsInput: document.getElementById("customInstructions"),
        llmProviderLabel: document.getElementById("llm-provider-label"),
        llmProviderSelect: document.getElementById("llmProvider"),
        openaiApiHint: document.getElementById("openai-api-hint"),
        revertButton: document.getElementById("revertBtn"),
      },
    });

    this.hotkey = new HotkeyController({
      button: document.getElementById("startHotkeyCapture"),
      display: document.getElementById("hotkeyDisplay"),
      hiddenInput: document.getElementById("hotkey"),
      toast: this.toast,
      settings: this.settings,
      logger: this.logger,
    });

    this.settings.attachHotkey(this.hotkey);
    this.unlisten = [];
  }

  async start() {
    this.bridge.hydrate();
    this.hotkey.registerWindowListeners();
    this.registerUiHandlers();

    await this.performPing();
    await this.settings.load();
    await this.populateVersion();
    await this.registerTauriEvents();
  }

  registerUiHandlers() {
    this.settings.form?.addEventListener("submit", async (event) => {
      event.preventDefault();
      this.hotkey.cancel();

      const payload = this.settings.current();
      if (!payload.hotkey) {
        this.toast.show("Hotkey cannot be empty", "error");
        return;
      }
      if (!this.hotkey.bindingHasMainKey(payload.hotkey)) {
        this.toast.show("Hotkey must contain a non-modifier key", "error");
        return;
      }
      if (this.hotkey.bindingUsesMouse(payload.hotkey)) {
        this.toast.show("Mouse buttons are not supported for hotkeys", "error");
        return;
      }
      const saved = await this.settings.persist(payload);
      if (saved) {
        this.hotkey.render(payload.hotkey);
      }
    });

    this.settings.revertButton?.addEventListener("click", () => {
      this.hotkey.cancel();
      this.settings.reset();
    });

    this.settings.providerSelect?.addEventListener("change", () => {
      this.settings.updateProviderFields();
    });

    this.settings.autoTranslateInput?.addEventListener("change", () => {
      this.settings.syncTranslationUi();
    });

    this.settings.useCustomInstructionsInput?.addEventListener("change", () => {
      this.settings.syncCustomInstructionsUi();
    });

    window.addEventListener("beforeunload", () => {
      this.unlisten.forEach((dispose) => {
        try {
          dispose();
        } catch (error) {
          console.error(error);
        }
      });
    });
  }

  async performPing() {
    try {
      const pong = await this.bridge.call("ping");
      this.logger(ping -> );
    } catch (error) {
      this.logger(ping failed: , "error");
    }
  }

  async populateVersion() {
    const versionSpan = document.getElementById("app-version");
    if (!versionSpan) {
      return;
    }
    const version = await this.bridge.getVersion();
    if (version) {
      versionSpan.textContent = version;
    }
  }

  async registerTauriEvents() {
    try {
      const statusUnlisten = await this.bridge.on("transcription://status", ({ payload }) => {
        const phase = payload?.phase ?? "idle";
        const message = payload?.message ?? "";
        const fallbackMessage = {
          idle: "Готово к записи по горячей клавише",
          recording: "Идёт запись...",
          transcribing: "Отправка и распознавание...",
          success: "Готово",
          error: "Ошибка",
        };
        this.status.set(phase, message || fallbackMessage[phase] || fallbackMessage.idle);
        if (phase === "recording") {
          this.status.showProgress({ indeterminate: true });
        } else if (phase === "transcribing") {
          this.status.showProgress({ indeterminate: false, value: 0 });
        } else {
          this.status.showProgress(null);
        }
        if (phase === "error" && message) {
          this.toast.show(message, "error");
        }
      });
      this.unlisten.push(statusUnlisten);

      const partialUnlisten = await this.bridge.on("transcription://partial", ({ payload }) => {
        if (!payload?.text) {
          return;
        }
        this.status.showResult(payload.text);
      });
      this.unlisten.push(partialUnlisten);

      const completeUnlisten = await this.bridge.on("transcription://complete", ({ payload }) => {
        if (payload?.text) {
          this.status.showResult(payload.text);
        }
        this.toast.show("Готово", "success");
        this.status.set("success", "Обработка завершена");
      });
      this.unlisten.push(completeUnlisten);
    } catch (error) {
      console.error(error);
      this.toast.show("Unable to subscribe to transcription events", "error");
    }
  }
}
