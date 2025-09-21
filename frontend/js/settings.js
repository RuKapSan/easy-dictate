import { DEFAULT_HOTKEY, PROVIDER_MODEL_OPTIONS } from "./constants.js";

export class SettingsController {
  constructor({
    bridge,
    toast,
    logger,
    elements: {
      form,
      providerSelect,
      apiKeyInput,
      groqApiKeyInput,
      openaiApiKeyLabel,
      groqApiKeyLabel,
      modelSelect,
      simulateTypingInput,
      copyToClipboardInput,
      autoStartInput,
      useStreamingInput,
      autoTranslateInput,
      targetLanguageSelect,
      useCustomInstructionsInput,
      customInstructionsWrapper,
      customInstructionsInput,
      llmProviderLabel,
      llmProviderSelect,
      openaiApiHint,
      revertButton,
    },
  }) {
    this.bridge = bridge;
    this.toast = toast;
    this.logger = logger ?? (() => {});

    this.form = form;
    this.providerSelect = providerSelect;
    this.apiKeyInput = apiKeyInput;
    this.groqApiKeyInput = groqApiKeyInput;
    this.openaiApiKeyLabel = openaiApiKeyLabel;
    this.groqApiKeyLabel = groqApiKeyLabel;
    this.modelSelect = modelSelect;
    this.simulateTypingInput = simulateTypingInput;
    this.copyToClipboardInput = copyToClipboardInput;
    this.autoStartInput = autoStartInput;
    this.useStreamingInput = useStreamingInput;
    this.autoTranslateInput = autoTranslateInput;
    this.targetLanguageSelect = targetLanguageSelect;
    this.useCustomInstructionsInput = useCustomInstructionsInput;
    this.customInstructionsWrapper = customInstructionsWrapper;
    this.customInstructionsInput = customInstructionsInput;
    this.llmProviderLabel = llmProviderLabel;
    this.llmProviderSelect = llmProviderSelect;
    this.openaiApiHint = openaiApiHint;
    this.revertButton = revertButton;

    this.hotkeyController = null;
    this.initial = null;
  }

  attachHotkey(controller) {
    this.hotkeyController = controller;
  }

  async load() {
    try {
      const settings = await this.bridge.call("get_settings");
      this.initial = { ...settings };
      this.apply(settings);
      return settings;
    } catch (error) {
      console.error(error);
      this.toast.show("Unable to load settings", "error");
      throw error;
    }
  }

  current() {
    return {
      provider: this.providerSelect.value,
      llm_provider: this.llmProviderSelect?.value ?? "openai",
      api_key: this.apiKeyInput.value.trim(),
      groq_api_key: this.groqApiKeyInput.value.trim(),
      model: this.modelSelect.value,
      hotkey: this.hotkeyController?.currentHotkey() ?? DEFAULT_HOTKEY,
      simulate_typing: Boolean(this.simulateTypingInput?.checked),
      copy_to_clipboard: Boolean(this.copyToClipboardInput?.checked),
      auto_start: Boolean(this.autoStartInput?.checked),
      use_streaming: Boolean(this.useStreamingInput?.checked),
      auto_translate: Boolean(this.autoTranslateInput?.checked),
      target_language: this.targetLanguageSelect?.value ?? "English",
      use_custom_instructions: Boolean(this.useCustomInstructionsInput?.checked),
      custom_instructions: (this.customInstructionsInput?.value ?? "").trim(),
    };
  }

  apply(settings) {
    this.providerSelect.value = settings.provider ?? "openai";
    if (this.llmProviderSelect) {
      this.llmProviderSelect.value = settings.llm_provider ?? "openai";
    }
    this.apiKeyInput.value = settings.api_key ?? "";
    this.groqApiKeyInput.value = settings.groq_api_key ?? "";
    this.modelSelect.value = settings.model ?? "gpt-4o-transcribe";

    this.updateProviderFields();

    if (this.hotkeyController) {
      this.hotkeyController.render(settings.hotkey ?? DEFAULT_HOTKEY);
    }

    this.simulateTypingInput.checked = Boolean(settings.simulate_typing);
    this.copyToClipboardInput.checked = Boolean(settings.copy_to_clipboard);
    this.autoStartInput.checked = Boolean(settings.auto_start);
    this.useStreamingInput.checked = Boolean(settings.use_streaming);
    this.autoTranslateInput.checked = Boolean(settings.auto_translate);
    this.targetLanguageSelect.value = settings.target_language ?? "English";

    if (this.useCustomInstructionsInput) {
      this.useCustomInstructionsInput.checked = Boolean(settings.use_custom_instructions);
    }
    if (this.customInstructionsInput) {
      this.customInstructionsInput.value = settings.custom_instructions ?? "";
    }

    this.syncTranslationUi();
    this.syncCustomInstructionsUi();
  }

  async persist(payload, successMessage = "Settings saved") {
    try {
      await this.bridge.call("save_settings", { settings: payload });
      this.initial = { ...payload };
      this.toast.show(successMessage, "success");
      return true;
    } catch (error) {
      const message = typeof error === "string" ? error : error?.message ?? "Failed to save settings";
      this.toast.show(message, "error");
      this.logger(save_settings failed: , "error");
      return false;
    }
  }

  reset() {
    if (!this.initial) {
      return;
    }
    this.apply(this.initial);
    this.toast.show("Reverted to last saved values");
  }

  updateProviderFields() {
    const provider = this.providerSelect.value ?? "openai";
    const options = PROVIDER_MODEL_OPTIONS[provider] ?? PROVIDER_MODEL_OPTIONS.openai;

    if (this.openaiApiKeyLabel) {
      this.openaiApiKeyLabel.hidden = false;
    }
    if (this.groqApiKeyLabel) {
      this.groqApiKeyLabel.hidden = provider !== "groq";
    }

    if (this.openaiApiHint) {
      this.openaiApiHint.textContent = provider === "groq" ? "(Required if Groq is used as the LLM)" : "";
    }

    const currentModel = this.modelSelect.value;
    this.modelSelect.innerHTML = "";

    options.forEach(({ value, label }) => {
      const optionEl = document.createElement("option");
      optionEl.value = value;
      optionEl.textContent = label;
      this.modelSelect.appendChild(optionEl);
    });

    if (options.some(({ value }) => value === currentModel)) {
      this.modelSelect.value = currentModel;
    } else if (options.length > 0) {
      this.modelSelect.value = options[0].value;
    }
  }

  syncTranslationUi() {
    const enabled = Boolean(this.autoTranslateInput?.checked);
    if (this.targetLanguageSelect) {
      this.targetLanguageSelect.disabled = !enabled;
      this.targetLanguageSelect.classList.toggle("is-disabled", !enabled);
    }
    this.updateLLMProviderVisibility();
  }

  syncCustomInstructionsUi() {
    const enabled = Boolean(this.useCustomInstructionsInput?.checked);
    if (this.customInstructionsWrapper) {
      this.customInstructionsWrapper.hidden = !enabled;
    }
    if (this.customInstructionsInput) {
      this.customInstructionsInput.disabled = !enabled;
    }
    this.updateLLMProviderVisibility();
  }

  updateLLMProviderVisibility() {
    const needsLLM = Boolean(this.autoTranslateInput?.checked || this.useCustomInstructionsInput?.checked);
    if (this.llmProviderLabel) {
      this.llmProviderLabel.hidden = !needsLLM;
    }
  }
}
