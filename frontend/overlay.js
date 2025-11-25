const { listen } = window.__TAURI__.event;
const { invoke } = window.__TAURI__.core;
const { getCurrentWindow } = window.__TAURI__.window;

const container = document.getElementById('overlay-container');
const textEl = document.getElementById('transcription-text');
const appWindow = getCurrentWindow();

function log(msg) {
    console.log('[Overlay] ' + msg);
    invoke('frontend_log', { level: 'info', message: '[Overlay] ' + msg }).catch(() => { });
}

let hideTimeout = null;
let animationTimeout = null;
let showRealtimeText = true;

async function loadSettings() {
    try {
        const settings = await invoke('get_settings');
        showRealtimeText = settings.use_streaming !== false;
        log('Settings loaded: showRealtimeText=' + showRealtimeText);
    } catch (e) {
        log('Failed to load settings: ' + e);
    }
}

async function showOverlay() {
    if (hideTimeout) {
        clearTimeout(hideTimeout);
        hideTimeout = null;
    }
    if (animationTimeout) {
        clearTimeout(animationTimeout);
        animationTimeout = null;
    }
    await invoke('show_overlay_no_focus').catch(e => log('show_overlay_no_focus error: ' + e));
    container.classList.remove('hidden');
    log('Showing overlay');
}

function hideOverlay(delay = 2000) {
    if (hideTimeout) clearTimeout(hideTimeout);
    if (animationTimeout) clearTimeout(animationTimeout);

    hideTimeout = setTimeout(async () => {
        container.classList.add('hidden');
        setTimeout(() => {
            appWindow.hide().catch(() => {});
        }, 300);
        log('Hiding overlay');
    }, delay);
}

function updateText(text) {
    textEl.textContent = text;
    textEl.classList.remove('updating');
    void textEl.offsetWidth;
    textEl.classList.add('updating');
}

function setStatus(status) {
    container.classList.remove('success', 'error');
    if (status === 'success') container.classList.add('success');
    if (status === 'error') container.classList.add('error');
}

async function init() {
    log('Overlay initialized');
    
    await loadSettings();

    await listen('settings://changed', async () => {
        log('Settings changed, reloading...');
        await loadSettings();
    });

    await listen('transcription://status', (event) => {
        log('Status event: ' + JSON.stringify(event.payload));
        const { phase } = event.payload;

        if (phase === 'recording') {
            setStatus('recording');
            updateText('');
            showOverlay();
        } else if (phase === 'transcribing') {
            setStatus('transcribing');
        } else if (phase === 'success') {
            setStatus('success');
            hideOverlay(500);
        } else if (phase === 'error') {
            setStatus('error');
            updateText('Ошибка');
            hideOverlay(3000);
        } else if (phase === 'idle') {
            if (!container.classList.contains('success')) {
                hideOverlay(0);
            }
        }
    });

    await listen('transcription://partial', (event) => {
        if (!showRealtimeText) {
            return;
        }
        if (event.payload?.text) {
            updateText(event.payload.text);
            showOverlay();
        }
    });

    await listen('transcription://complete', (event) => {
        if (event.payload?.text) {
            updateText(event.payload.text);
            setStatus('success');
            hideOverlay(2500);
        }
    });
}

init().catch(console.error);
