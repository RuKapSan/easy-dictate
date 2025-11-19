/**
 * ElevenLabs Streaming STT Integration (Rust-based)
 * Manages Tauri backend commands for real-time transcription
 */

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
    log(`Failed to connect: ${error}`, "error");
    showToastIfAvailable(`ElevenLabs connection error: ${error}`, "error");
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
    log(`Failed to disconnect: ${error}`, "error");
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
 * Setup event listeners for ElevenLabs streaming events from Rust backend
 */
function setupElevenLabsEventListeners() {
  const listen = window.__TAURI__?.event?.listen;
  if (!listen) {
    log("Tauri listen not available", "error");
    return;
  }

  // Listen for transcript events (both partial and committed)
  listen("elevenlabs://transcript", ({ payload }) => {
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

  // Auto-reconnect on connection close
  listen("elevenlabs://connection-closed", (event) => {
    const payload = event.payload || {};
    const code = payload.code;
    const reason = payload.reason;

    log(`Connection closed. Code: ${code}, Reason: ${reason}`, "info");

    // Reconnect on context reset (4001) and normal closure (1000) as well as any other code
    isConnected = false;
    const delayMs = code === 4001 ? 100 : 1000;
    log(`Scheduling reconnect in ${delayMs}ms...`);
    setTimeout(() => {
      if (currentProvider === "elevenlabs" && lastApiKey) {
        log("Reconnecting...");
        connectElevenLabsStreaming(lastApiKey, lastSampleRate, lastLanguageCode);
      }
    }, delayMs);
  });

  // Handle errors
  listen("elevenlabs://error", ({ payload }) => {
    log(`Error from backend: ${payload.error}`, "error");
    // If error indicates connection loss, we might want to reset isConnected
    if (payload.error.includes("Connection is dead") || payload.error.includes("closed")) {
      isConnected = false;
    }
  });

  log("Event listeners registered");
}

// Export functions for use in main.js
window.ElevenLabsSTT = {
  init: initElevenLabsStreaming,
  connect: connectElevenLabsStreaming,
  disconnect: disconnectElevenLabsStreaming,
  isConnected: isStreamingConnected,
  setupEventListeners: setupElevenLabsEventListeners,
};

log("Module loaded");
