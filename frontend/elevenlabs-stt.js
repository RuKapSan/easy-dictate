/**
 * ElevenLabs Streaming STT Integration (Rust-based)
 * Manages Tauri backend commands for real-time transcription
 */

/** Extract message from a command error ({ code, message } or string). */
function elErrMsg(err) {
  if (err && typeof err === "object" && err.message) return err.message;
  return String(err);
}

function log(msg, level = "info") {
  console.log(`[ElevenLabs STT] ${msg}`);
  if (window.__TAURI__?.core?.invoke) {
    window.__TAURI__.core.invoke("frontend_log", {
      level,
      message: `[ElevenLabs STT] ${msg}`
    }).catch(() => { });
  }
}

let isConnected = false;
let currentProvider = null;

// Store last config for reconnection
let lastApiKey = "";
let lastSampleRate = 48000;
let lastLanguageCode = "auto";

// Store event listener unsubscribe functions for cleanup
let eventListenerCleanup = [];

/**
 * Connect to ElevenLabs streaming STT via Rust backend
 */
async function connectElevenLabsStreaming(apiKey, sampleRate = 48000, languageCode = "auto") {
  if (!window.__TAURI__?.core?.invoke) {
    console.error("[ElevenLabs STT] Tauri invoke not available");
    return false;
  }

  // Update last config
  lastApiKey = apiKey;
  lastSampleRate = sampleRate;
  lastLanguageCode = languageCode;

  if (isConnected) {
    // Check if backend agrees
    try {
      const backendConnected = await window.__TAURI__.core.invoke("elevenlabs_streaming_is_connected");
      if (backendConnected) {
        log("Already connected");
        return true;
      }
    } catch (e) {
      // ignore
    }
  }

  try {
    log(`Connecting to streaming (lang: ${languageCode}, rate: ${sampleRate}Hz)...`);
    await window.__TAURI__.core.invoke("elevenlabs_streaming_connect", {
      apiKey,
      sampleRate,
      languageCode,
    });

    isConnected = true;
    log("Connected to streaming successfully");
    return true;
  } catch (error) {
    log(`Failed to connect: ${elErrMsg(error)}`, "error");
    showToastIfAvailable(`ElevenLabs connection error: ${elErrMsg(error)}`, "error");
    isConnected = false;
    return false;
  }
}

/**
 * Disconnect from ElevenLabs streaming STT
 */
async function disconnectElevenLabsStreaming() {
  if (!window.__TAURI__?.core?.invoke) {
    return;
  }

  try {
    log("Disconnecting...");
    await window.__TAURI__.core.invoke("elevenlabs_streaming_disconnect");
    isConnected = false;
    log("Disconnected");
  } catch (error) {
    log(`Failed to disconnect: ${elErrMsg(error)}`, "error");
  }
}

/**
 * Check if currently connected
 */
async function isStreamingConnected() {
  if (!window.__TAURI__?.core?.invoke) {
    return false;
  }

  try {
    return await window.__TAURI__.core.invoke("elevenlabs_streaming_is_connected");
  } catch {
    return false;
  }
}

/**
 * Helper to show toast if element exists
 */
function showToastIfAvailable(message, type = "info") {
  const toastEl = document.getElementById("toast");
  if (toastEl) {
    toastEl.textContent = message;
    toastEl.dataset.type = type;
    toastEl.hidden = false;
    setTimeout(() => {
      toastEl.hidden = true;
    }, 3000);
  }
}

/**
 * Initialize ElevenLabs streaming based on settings
 */
async function initElevenLabsStreaming(settings) {
  log(`init called. Provider: ${settings.provider}, Connected: ${isConnected}`);
  const isElevenLabs = settings.provider === "elevenlabs";

  // Disconnect if switching away from ElevenLabs
  if (currentProvider === "elevenlabs" && !isElevenLabs) {
    log("Provider changed, disconnecting...");
    await disconnectElevenLabsStreaming();
    currentProvider = null;
    return;
  }

  // Connect if ElevenLabs is selected
  if (isElevenLabs) {
    const apiKey = settings.elevenlabs_api_key?.trim();

    if (!apiKey) {
      log("No API key configured", "warn");
      return;
    }

    // Prefer automatic language detection for streaming
    let languageCode = "auto";

    // Always try to connect/reconnect if needed
    log(`Initializing with language: ${languageCode}`);
    const connected = await connectElevenLabsStreaming(apiKey, 48000, languageCode);

    if (connected) {
      currentProvider = "elevenlabs";
      showToastIfAvailable("ElevenLabs streaming подключён", "success");
    }
  }
}

/**
 * Cleanup previously registered event listeners to prevent memory leaks
 */
function cleanupEventListeners() {
  if (eventListenerCleanup.length > 0) {
    log(`Cleaning up ${eventListenerCleanup.length} event listener(s)`);
    for (const unsubscribe of eventListenerCleanup) {
      if (typeof unsubscribe === "function") {
        unsubscribe();
      }
    }
    eventListenerCleanup = [];
  }
}

/**
 * Setup event listeners for ElevenLabs streaming events from Rust backend
 */
async function setupElevenLabsEventListeners() {
  const listen = window.__TAURI__?.event?.listen;
  if (!listen) {
    log("Tauri listen not available", "error");
    return;
  }

  // Clean up any existing listeners first to prevent accumulation
  cleanupEventListeners();

  // Listen for transcript events (both partial and committed)
  const unsubTranscript = await listen("elevenlabs://transcript", ({ payload }) => {
    if (!payload) return;

    const resultEl = document.getElementById("last-result");
    if (resultEl && payload.text) {
      resultEl.hidden = false;
      resultEl.textContent = payload.text;

      // Visual indicator for partial vs committed
      if (payload.is_partial) {
        // Partial transcript - show as preview
        resultEl.style.opacity = "0.6";
        resultEl.style.fontStyle = "italic";
        resultEl.style.color = "#888";
        log(`Partial: "${payload.text}"`, "debug");
      } else {
        // Committed transcript - show as final
        resultEl.style.opacity = "1";
        resultEl.style.fontStyle = "normal";
        resultEl.style.color = "";
        log(`Committed: "${payload.text}"`);
      }
    }
  });
  eventListenerCleanup.push(unsubTranscript);

  // Connection closed: for ContextReset (4001) auto-reconnect to keep next press instant
  const unsubClosed = await listen("elevenlabs://connection-closed", (event) => {
    const payload = event.payload || {};
    const code = payload.code;
    const reason = payload.reason;

    log(`Connection closed. Code: ${code}, Reason: ${reason}`, "info");
    isConnected = false;
    if (code === 4001 && currentProvider === "elevenlabs") {
      setTimeout(async () => {
        // Re-read current settings to avoid using stale cached values
        try {
          const settings = await window.__TAURI__.core.invoke("get_settings");
          const apiKey = settings.elevenlabs_api_key || lastApiKey;
          if (!apiKey) return;
          log("Reconnecting after context reset with fresh settings...");
          connectElevenLabsStreaming(apiKey, lastSampleRate, lastLanguageCode);
        } catch (e) {
          log(`Failed to get settings for reconnect: ${elErrMsg(e)}`, "error");
          if (lastApiKey) connectElevenLabsStreaming(lastApiKey, lastSampleRate, lastLanguageCode);
        }
      }, 100);
    }
  });
  eventListenerCleanup.push(unsubClosed);

  // Handle errors
  const unsubError = await listen("elevenlabs://error", ({ payload }) => {
    log(`Error from backend: ${payload.error}`, "error");
    // If error indicates connection loss, we might want to reset isConnected
    if (payload.error.includes("Connection is dead") || payload.error.includes("closed")) {
      isConnected = false;
    }
  });
  eventListenerCleanup.push(unsubError);

  log("Event listeners registered");
}

// Export functions for use in main.js
window.ElevenLabsSTT = {
  init: initElevenLabsStreaming,
  connect: connectElevenLabsStreaming,
  disconnect: disconnectElevenLabsStreaming,
  isConnected: isStreamingConnected,
  setupEventListeners: setupElevenLabsEventListeners,
  cleanupEventListeners: cleanupEventListeners,
};

log("Module loaded");
