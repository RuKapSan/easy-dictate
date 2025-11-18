const { listen } = window.__TAURI__.event;
const { getCurrentWindow } = window.__TAURI__.window;

const container = document.getElementById('overlay-container');
const textEl = document.getElementById('transcription-text');
const appWindow = getCurrentWindow();

function log(msg) {
    console.log(`[Overlay] ${msg}`);
    window.__TAURI__.core.invoke('frontend_log', { level: 'info', message: `[Overlay] ${msg}` }).catch(() => { });
}

let hideTimeout = null;
let animationTimeout = null;

async function showOverlay() {
    if (hideTimeout) {
        clearTimeout(hideTimeout);
        hideTimeout = null;
    }
    if (animationTimeout) {
        clearTimeout(animationTimeout);
        animationTimeout = null;
    }
    container.classList.remove('hidden');
    log("Showing overlay window");
    // Use custom command to show without stealing focus
    await window.__TAURI__.core.invoke('show_overlay_no_focus');
}

function hideOverlay(delay = 2000) {
    if (hideTimeout) clearTimeout(hideTimeout);
    if (animationTimeout) clearTimeout(animationTimeout);

    hideTimeout = setTimeout(async () => {
        container.classList.add('hidden');
        // Wait for animation to finish before hiding window
        animationTimeout = setTimeout(() => {
            log("Hiding overlay window (system)");
            appWindow.hide();
            animationTimeout = null;
        }, 300);
    }, delay);
}

function updateText(text) {
    textEl.textContent = text;
    textEl.classList.remove('updating');
    void textEl.offsetWidth; // Trigger reflow
    textEl.classList.add('updating');
}

function setStatus(status) {
    container.classList.remove('success', 'error');
    if (status === 'success') container.classList.add('success');
    if (status === 'error') container.classList.add('error');
}

async function init() {
    log("Overlay initialized");
    // Ensure window is hidden on startup (since we set visible: true in config)
    // appWindow.hide();
    // Enable click-through
    // await appWindow.setIgnoreCursorEvents(true);

    // Listen for status changes
    await listen('transcription://status', (event) => {
        log(`Status event: ${JSON.stringify(event.payload)}`);
        const { phase } = event.payload;

        if (phase === 'recording') {
            setStatus('recording');
            updateText(''); // Clear text, rely on waveform
            showOverlay();
        } else if (phase === 'transcribing') {
            setStatus('transcribing');
            // Keep showing overlay
        } else if (phase === 'success') {
            setStatus('success');
            hideOverlay(500); // Hide quickly after success
        } else if (phase === 'error') {
            setStatus('error');
            updateText('Ошибка');
            hideOverlay(3000);
        } else if (phase === 'idle') {
            // Only hide if we are NOT in success state (let success timeout handle it)
            if (!container.classList.contains('success')) {
                hideOverlay(0);
            }
        }
    });

    // Listen for partial results
    await listen('transcription://partial', (event) => {
        // log(`Partial: ${event.payload?.text}`); // Commented out to avoid spam
        if (event.payload?.text) {
            updateText(event.payload.text);
            showOverlay();
        }
    });

    // Listen for complete results
    await listen('transcription://complete', (event) => {
        if (event.payload?.text) {
            updateText(event.payload.text);
            setStatus('success');
            hideOverlay(2500);
        }
    });
}

init().catch(console.error);
