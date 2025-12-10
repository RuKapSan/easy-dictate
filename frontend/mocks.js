/**
 * Mocks for running in a standard browser environment (outside Tauri).
 * This allows UI testing and development without the Rust backend.
 */
(function () {
    if (window.__TAURI__) {
        return; // Already in Tauri, do nothing
    }

    console.log("%c[Mocks] Initializing Tauri API Mocks...", "color: #4a90e2; font-weight: bold;");

    const listeners = new Map();

    // Mock Settings
    let mockSettings = {
        provider: "openai",
        api_key: "sk-mock-key-12345",
        groq_api_key: "",
        elevenlabs_api_key: "",
        model: "gpt-4o-transcribe",
        hotkey: "Ctrl+Shift+Space",
        simulate_typing: true,
        copy_to_clipboard: false,
        auto_start: false,
        use_streaming: true,
        auto_translate: false,
        target_language: "русский",
        llm_provider: "openai",
        use_custom_instructions: false,
        custom_instructions: "",
    };

    window.__TAURI__ = {
        core: {
            invoke: async (cmd, args) => {
                console.log(`%c[Invoke] ${cmd}`, "color: #bada55", args);

                switch (cmd) {
                    case "get_settings":
                        return { ...mockSettings };

                    case "save_settings":
                        if (args.settings) {
                            mockSettings = { ...args.settings };
                            console.log("[Mocks] Settings saved:", mockSettings);
                        }
                        return true;

                    case "frontend_log":
                        console.log(`[Backend Log] ${args.level}: ${args.message}`);
                        return;

                    case "elevenlabs_streaming_is_connected":
                        return false;

                    case "elevenlabs_streaming_connect":
                        console.log("[Mocks] Connecting to ElevenLabs...");
                        setTimeout(() => {
                            triggerEvent("transcription://status", { phase: "transcribing", message: "Mock: Connected" });
                        }, 500);
                        return true;

                    case "elevenlabs_streaming_disconnect":
                        console.log("[Mocks] Disconnecting ElevenLabs...");
                        return true;

                    case "ping":
                        return "pong";

                    default:
                        console.warn(`[Mocks] Unknown command: ${cmd}`);
                        return null;
                }
            },
        },
        event: {
            listen: async (event, handler) => {
                console.log(`%c[Listen] ${event}`, "color: #d480aa");
                if (!listeners.has(event)) {
                    listeners.set(event, []);
                }
                listeners.get(event).push(handler);

                // Return unlisten function
                return () => {
                    const handlers = listeners.get(event);
                    const idx = handlers.indexOf(handler);
                    if (idx > -1) handlers.splice(idx, 1);
                };
            },
        },
        app: {
            getVersion: async () => "1.0.0-MOCK",
        },
    };

    // Helper to trigger events from the browser console
    window.mockTrigger = (event, payload) => {
        const handlers = listeners.get(event);
        if (handlers) {
            handlers.forEach(h => h({ payload }));
            console.log(`[Mocks] Triggered ${event}`, payload);
        } else {
            console.warn(`[Mocks] No listeners for ${event}`);
        }
    };

    // Auto-trigger some status after load to show it works
    setTimeout(() => {
        window.mockTrigger("transcription://status", {
            phase: "idle",
            message: "Готово к записи (Mock)"
        });
    }, 1000);

})();
