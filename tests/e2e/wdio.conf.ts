/**
 * WebdriverIO configuration for Tauri E2E tests
 *
 * Prerequisites:
 * 1. Install tauri-driver: cargo install tauri-driver
 * 2. Build app in debug mode: cargo tauri build --debug --no-bundle
 * 3. Ensure msedgedriver.exe is in PATH or in ./drivers/
 * 4. Run tests: npx wdio run wdio.conf.ts
 */

import type { Options } from '@wdio/types';
import * as path from 'path';
import * as fs from 'fs';
import { spawn, ChildProcess } from 'child_process';
import treeKill from 'tree-kill';
import { cleanupHotkeySession } from './utils/hotkey-tester';

const isCI = process.env.CI === 'true';

// tauri-driver process
let tauriDriver: ChildProcess | null = null;

// Create directories for test artifacts
const screenshotsDir = path.join(__dirname, 'screenshots');
const logsDir = path.join(__dirname, 'logs');
const audioMocksDir = path.join(__dirname, 'audio-mocks');
const videosDir = path.join(__dirname, 'videos');

[screenshotsDir, logsDir, audioMocksDir, videosDir].forEach(dir => {
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }
});

// Add msedgedriver to PATH if it exists in ./drivers/
const driversDir = path.join(__dirname, 'drivers');
if (fs.existsSync(driversDir)) {
  process.env.PATH = `${driversDir}${path.delimiter}${process.env.PATH}`;
}

export const config: Options.Testrunner = {
  // Test specs (relative to this config file)
  specs: ['./specs/**/*.spec.ts'],
  exclude: [],

  // =====================
  // Server Configuration
  // =====================
  // WDIO connects to tauri-driver on port 4444
  // tauri-driver manages msedgedriver internally (usually on port 9515)
  hostname: '127.0.0.1',
  port: 4444,
  path: '/',
  connectionRetryTimeout: 120000,
  connectionRetryCount: 3,

  // ============
  // Capabilities
  // ============
  maxInstances: 1,
  capabilities: [{
    maxInstances: 1,
    browserName: 'wry',
    'tauri:options': {
      // Use release build which has embedded frontend assets
      // Debug builds use devUrl which requires a running dev server
      application: process.env.TAURI_APP_PATH || path.join(__dirname, '../../src-tauri/target/release/app.exe'),
      args: []
    }
  } as any],

  // Test framework
  framework: 'mocha',
  mochaOpts: {
    ui: 'bdd',
    timeout: 60000,
    retries: isCI ? 2 : 0
  },

  // Reporters
  reporters: [
    'spec',
    ['allure', {
      outputDir: path.join(__dirname, 'allure-results'),
      disableWebdriverStepsReporting: false,
      disableWebdriverScreenshotsReporting: false,
    }],
    ['json', {
      outputDir: logsDir,
      outputFileFormat: (opts: any) => `results-${opts.cid}.${opts.capabilities}.json`
    }],
    ['video', {
      saveAllVideos: true,
      videoSlowdownMultiplier: 1,
      videoRenderTimeout: 5,
      outputDir: videosDir,
      maxTestNameCharacters: 100
    } as any]
  ],

  // Log level
  logLevel: 'info',
  outputDir: logsDir,

  // No external services - we manage tauri-driver ourselves
  services: [],

  // ===================
  // Hooks
  // ===================

  // 1. Start tauri-driver before tests
  onPrepare: async function () {
    console.log('🚀 Starting tauri-driver on port 4444...');

    const tauriDriverPath = process.env.TAURI_DRIVER_PATH || 'tauri-driver';

    // Spawn tauri-driver on port 4444
    // tauri-driver will automatically manage msedgedriver internally
    tauriDriver = spawn(tauriDriverPath, ['--port', '4444'], {
      stdio: ['ignore', 'pipe', 'pipe'],
      shell: true
    });

    tauriDriver.stdout?.on('data', (data) => {
      console.log(`[tauri-driver] ${data.toString().trim()}`);
    });

    tauriDriver.stderr?.on('data', (data) => {
      console.error(`[tauri-driver] ${data.toString().trim()}`);
    });

    tauriDriver.on('error', (err) => {
      console.error('Failed to start tauri-driver:', err);
      process.exit(1);
    });

    // Wait for tauri-driver to be ready
    await new Promise<void>((resolve, reject) => {
      const timeout = setTimeout(() => {
        console.log('⏰ Timeout waiting for tauri-driver, proceeding anyway...');
        resolve();
      }, 5000);

      tauriDriver!.stdout?.on('data', (data) => {
        const msg = data.toString();
        // Check for ready indicators
        if (msg.includes('listening') || msg.includes('started') || msg.includes('msedgedriver')) {
          clearTimeout(timeout);
          resolve();
        }
      });

      // Fallback: wait 3 seconds
      setTimeout(() => {
        clearTimeout(timeout);
        resolve();
      }, 3000);
    });

    console.log('✅ tauri-driver started');
  },

  // 2. Clean up processes (use tree-kill to kill all child processes)
  onComplete: async function () {
    // Cleanup PowerShell session
    cleanupHotkeySession();

    // Kill tauri-driver and all its children (msedgedriver, app.exe)
    if (tauriDriver && tauriDriver.pid) {
      console.log('🛑 Stopping tauri-driver and children...');
      await new Promise<void>((resolve) => {
        treeKill(tauriDriver!.pid!, 'SIGKILL', (err) => {
          if (err) console.error('Error killing tauri-driver:', err);
          resolve();
        });
      });
      tauriDriver = null;
    }

    console.log('✅ Test session completed');
  },

  beforeSession: async function (_config, _capabilities, _specs) {
    console.log('🚀 Starting test session...');
  },

  // 3. Register custom commands BEFORE tests run
  before: async function (_capabilities, _specs, browser) {
    // First, add all custom commands
    addCustomCommands(browser);

    // Then inject console error trap into browser context
    try {
      await browser.execute(() => {
        (window as any).__testConsoleErrors = [];
        (window as any).__testConsoleWarnings = [];

        const originalError = console.error;
        const originalWarn = console.warn;

        console.error = (...args: any[]) => {
          (window as any).__testConsoleErrors.push({
            message: args.map(a => String(a)).join(' '),
            timestamp: new Date().toISOString()
          });
          originalError.apply(console, args);
        };

        console.warn = (...args: any[]) => {
          (window as any).__testConsoleWarnings.push({
            message: args.map(a => String(a)).join(' '),
            timestamp: new Date().toISOString()
          });
          originalWarn.apply(console, args);
        };

        window.onerror = (message, source, lineno, colno, error) => {
          (window as any).__testConsoleErrors.push({
            message: `Unhandled: ${message}`,
            source,
            lineno,
            colno,
            stack: error?.stack,
            timestamp: new Date().toISOString()
          });
        };

        window.onunhandledrejection = (event) => {
          (window as any).__testConsoleErrors.push({
            message: `Unhandled Promise: ${event.reason}`,
            timestamp: new Date().toISOString()
          });
        };
      });
    } catch (e) {
      console.error('Failed to inject console trap:', e);
    }
  },

  afterTest: async function (test, _context, { error, passed }) {
    const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
    const testName = test.title.replace(/[^a-zA-Z0-9]/g, '_');

    // Always take screenshot after test
    const screenshotPath = path.join(
      screenshotsDir,
      `${testName}_${passed ? 'PASS' : 'FAIL'}_${timestamp}.png`
    );

    try {
      await browser.saveScreenshot(screenshotPath);
      console.log(`📸 Screenshot saved: ${screenshotPath}`);
    } catch (e) {
      console.error('Failed to save screenshot:', e);
    }

    // Get console errors from browser
    try {
      const consoleData = await browser.execute(() => ({
        errors: (window as any).__testConsoleErrors || [],
        warnings: (window as any).__testConsoleWarnings || []
      }));

      if (consoleData.errors.length > 0 || consoleData.warnings.length > 0) {
        fs.appendFileSync(
          path.join(logsDir, 'console-errors.json'),
          JSON.stringify({
            test: test.title,
            timestamp: new Date().toISOString(),
            errors: consoleData.errors,
            warnings: consoleData.warnings
          }) + '\n'
        );
      }

      // Clear for next test
      await browser.execute(() => {
        (window as any).__testConsoleErrors = [];
        (window as any).__testConsoleWarnings = [];
      });
    } catch (e) {
      console.error('Failed to get console errors:', e);
    }

    // Log error details
    if (error) {
      const errorLog = {
        test: test.title,
        error: error.message,
        stack: error.stack,
        timestamp: new Date().toISOString()
      };

      fs.appendFileSync(
        path.join(logsDir, 'errors.json'),
        JSON.stringify(errorLog) + '\n'
      );
    }
  },

  after: async function (_result, _capabilities, _specs) {
    console.log('✅ Test suite completed');
  }
};

/**
 * Add custom browser commands for testing
 * These are registered SYNCHRONOUSLY before any browser interaction
 */
function addCustomCommands(browser: WebdriverIO.Browser) {
  // Screenshot with timestamp
  browser.addCommand('screenshotWithTimestamp', async function (name: string) {
    const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
    const filename = `${name}_${timestamp}.png`;
    const filepath = path.join(screenshotsDir, filename);
    await browser.saveScreenshot(filepath);
    console.log(`📸 Screenshot: ${filepath}`);
    return filepath;
  });

  // Wait for Tauri invoke to be ready and show main window
  browser.addCommand('waitForTauri', async function (timeout = 10000) {
    await browser.waitUntil(
      async () => {
        const result = await browser.execute(() => {
          return typeof window.__TAURI__ !== 'undefined' &&
                 typeof window.__TAURI__.core?.invoke === 'function';
        });
        return result;
      },
      { timeout, timeoutMsg: 'Tauri API not available' }
    );

    // Switch to main window if we're on the overlay window
    // tauri-driver may connect to overlay window by default
    const currentUrl = await browser.getUrl();
    console.log('[waitForTauri] Current URL:', currentUrl);

    if (currentUrl.includes('overlay.html')) {
      console.log('[waitForTauri] On overlay window, switching to main window...');
      const handles = await browser.getWindowHandles();
      console.log('[waitForTauri] Window handles:', handles.length);

      // Try each window to find the main one
      for (const handle of handles) {
        await browser.switchToWindow(handle);
        const url = await browser.getUrl();
        console.log('[waitForTauri] Checking window:', url);
        if (!url.includes('overlay.html')) {
          console.log('[waitForTauri] Found main window!');
          break;
        }
      }
    }

    // Show the main window (it starts hidden by default)
    // This is necessary for E2E tests to find UI elements
    try {
      await browser.executeAsync(async (done) => {
        try {
          await window.__TAURI__.core.invoke('show_main_window');
          done({ success: true });
        } catch (error: any) {
          done({ success: false, error: error?.message || String(error) });
        }
      });
      // Give window time to render
      await browser.pause(500);
    } catch (e) {
      console.warn('[waitForTauri] Failed to show main window:', e);
    }
  });

  // Show main window explicitly
  browser.addCommand('showMainWindow', async function () {
    const response = await (browser as any).tauriInvoke('show_main_window');
    if (!response.success) throw new Error(response.error);
  });

  // Invoke Tauri command from test
  browser.addCommand('tauriInvoke', async function (cmd: string, args?: any) {
    return browser.executeAsync(async (command, commandArgs, done) => {
      try {
        const result = await window.__TAURI__.core.invoke(command, commandArgs);
        done({ success: true, result });
      } catch (error: any) {
        // In Tauri v2, errors from Result::Err can be strings or objects
        let errorMessage: string;
        if (typeof error === 'string') {
          errorMessage = error;
        } else if (error && typeof error === 'object') {
          errorMessage = error.message || JSON.stringify(error);
        } else {
          errorMessage = String(error);
        }
        done({ success: false, error: errorMessage });
      }
    }, cmd, args);
  });

  // Get current settings
  browser.addCommand('getSettings', async function () {
    const response = await (browser as any).tauriInvoke('get_settings');
    if (!response.success) throw new Error(response.error);
    return response.result;
  });

  // Save settings
  browser.addCommand('saveSettings', async function (settings: any) {
    const response = await (browser as any).tauriInvoke('save_settings', { settings });
    if (!response.success) throw new Error(response.error);
    return response.result;
  });

  // Get test state (recording, transcribing, etc.)
  browser.addCommand('getTestState', async function () {
    const response = await (browser as any).tauriInvoke('get_test_state');
    if (!response.success) throw new Error(response.error);
    return response.result;
  });

  // Inject test audio (bypasses microphone)
  browser.addCommand('injectTestAudio', async function (audioPath: string) {
    const audioBuffer = fs.readFileSync(audioPath);
    const audioData = Array.from(audioBuffer);
    // Note: Tauri v2 uses camelCase for command arguments in JavaScript
    const response = await (browser as any).tauriInvoke('inject_test_audio', { audioData });
    if (!response.success) throw new Error(response.error);
    return response.result;
  });

  // Simulate hotkey press via backend (starts recording)
  browser.addCommand('simulateHotkeyPress', async function () {
    const response = await (browser as any).tauriInvoke('simulate_hotkey_press');
    if (!response.success) throw new Error(response.error);
  });

  // Simulate hotkey release via backend (stops recording, triggers transcription)
  browser.addCommand('simulateHotkeyRelease', async function () {
    const response = await (browser as any).tauriInvoke('simulate_hotkey_release');
    if (!response.success) throw new Error(response.error);
  });

  // Wait for transcription status
  browser.addCommand('waitForStatus', async function (
    expectedPhase: string | string[],
    timeout = 30000
  ) {
    const phases = Array.isArray(expectedPhase) ? expectedPhase : [expectedPhase];
    await browser.waitUntil(
      async () => {
        const orbClass = await browser.$('#status-orb').getAttribute('class');
        return phases.some(phase => orbClass?.includes(phase));
      },
      {
        timeout,
        interval: 100,
        timeoutMsg: `Status did not change to any of [${phases.join(', ')}] within ${timeout}ms`
      }
    );
  });

  // Wait for recording state via backend
  browser.addCommand('waitForRecording', async function (expectedState: boolean, timeout = 10000) {
    await browser.waitUntil(
      async () => {
        const state = await (browser as any).getTestState();
        return state.is_recording === expectedState;
      },
      {
        timeout,
        interval: 100,
        timeoutMsg: `Recording state did not become ${expectedState}`
      }
    );
  });

  // Wait for transcription result
  browser.addCommand('waitForTranscriptionResult', async function (timeout = 30000) {
    let result = '';
    await browser.waitUntil(
      async () => {
        const resultEl = await browser.$('#last-result');
        if (await resultEl.isExisting() && await resultEl.isDisplayed()) {
          result = await resultEl.getText();
          return result.length > 0 && !resultEl.getAttribute('class').then(c => c?.includes('partial'));
        }
        return false;
      },
      { timeout, interval: 200, timeoutMsg: 'Transcription result not received' }
    );
    return result;
  });

  // Get console errors
  browser.addCommand('getConsoleErrors', async function () {
    return browser.execute(() => (window as any).__testConsoleErrors || []);
  });

  // Log UI state for debugging
  browser.addCommand('logUIState', async function () {
    const state = await browser.execute(() => {
      return {
        statusOrb: document.getElementById('status-orb')?.className,
        statusText: document.getElementById('status-text')?.textContent,
        lastResult: document.getElementById('last-result')?.textContent,
        provider: (document.getElementById('provider') as HTMLSelectElement)?.value,
        hotkey: (document.getElementById('hotkey') as HTMLInputElement)?.value,
        visible: document.visibilityState,
        consoleErrors: (window as any).__testConsoleErrors?.length || 0
      };
    });

    const logPath = path.join(logsDir, 'ui-state.log');
    fs.appendFileSync(logPath, JSON.stringify({
      timestamp: new Date().toISOString(),
      ...state
    }) + '\n');

    return state;
  });
}

// Type declarations for custom commands
declare global {
  namespace WebdriverIO {
    interface Browser {
      screenshotWithTimestamp(name: string): Promise<string>;
      waitForTauri(timeout?: number): Promise<void>;
      showMainWindow(): Promise<void>;
      tauriInvoke(cmd: string, args?: any): Promise<{ success: boolean; result?: any; error?: string }>;
      getSettings(): Promise<any>;
      saveSettings(settings: any): Promise<void>;
      getTestState(): Promise<any>;
      injectTestAudio(audioPath: string): Promise<string>;
      simulateHotkeyPress(): Promise<void>;
      simulateHotkeyRelease(): Promise<void>;
      waitForStatus(expectedPhase: string | string[], timeout?: number): Promise<void>;
      waitForRecording(expectedState: boolean, timeout?: number): Promise<void>;
      waitForTranscriptionResult(timeout?: number): Promise<string>;
      getConsoleErrors(): Promise<any[]>;
      logUIState(): Promise<any>;
    }
  }

  interface Window {
    __TAURI__: {
      core: {
        invoke: (cmd: string, args?: any) => Promise<any>;
      };
      event: {
        listen: (event: string, handler: any) => Promise<any>;
        emit: (event: string, payload?: any) => Promise<void>;
      };
    };
    __testConsoleErrors?: any[];
    __testConsoleWarnings?: any[];
  }
}

export default config;
