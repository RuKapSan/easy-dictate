/**
 * Hotkey testing utilities for Easy Dictate
 *
 * Uses persistent PowerShell session with Windows APIs for global hotkey simulation
 * Avoids process spawn overhead by keeping session alive
 */

import { spawn, ChildProcess } from 'child_process';
import * as path from 'path';
import * as fs from 'fs';

// Virtual key codes for Windows
const VK_CODES: Record<string, number> = {
  // Modifiers
  'Ctrl': 0x11,
  'Control': 0x11,
  'Shift': 0x10,
  'Alt': 0x12,
  'Win': 0x5B,
  'Meta': 0x5B,

  // Letters
  'A': 0x41, 'B': 0x42, 'C': 0x43, 'D': 0x44, 'E': 0x45,
  'F': 0x46, 'G': 0x47, 'H': 0x48, 'I': 0x49, 'J': 0x4A,
  'K': 0x4B, 'L': 0x4C, 'M': 0x4D, 'N': 0x4E, 'O': 0x4F,
  'P': 0x50, 'Q': 0x51, 'R': 0x52, 'S': 0x53, 'T': 0x54,
  'U': 0x55, 'V': 0x56, 'W': 0x57, 'X': 0x58, 'Y': 0x59, 'Z': 0x5A,

  // Numbers
  '0': 0x30, '1': 0x31, '2': 0x32, '3': 0x33, '4': 0x34,
  '5': 0x35, '6': 0x36, '7': 0x37, '8': 0x38, '9': 0x39,

  // Function keys
  'F1': 0x70, 'F2': 0x71, 'F3': 0x72, 'F4': 0x73, 'F5': 0x74,
  'F6': 0x75, 'F7': 0x76, 'F8': 0x77, 'F9': 0x78, 'F10': 0x79,
  'F11': 0x7A, 'F12': 0x7B,

  // Special keys
  'Space': 0x20,
  'Enter': 0x0D,
  'Escape': 0x1B,
  'Esc': 0x1B,
  'Tab': 0x09,
  'Backspace': 0x08,
  'Delete': 0x2E,
  'Insert': 0x2D,
  'Home': 0x24,
  'End': 0x23,
  'PageUp': 0x21,
  'PageDown': 0x22,

  // Arrow keys
  'Up': 0x26,
  'Down': 0x28,
  'Left': 0x25,
  'Right': 0x27,

  // Numpad
  'Numpad0': 0x60, 'Numpad1': 0x61, 'Numpad2': 0x62, 'Numpad3': 0x63,
  'Numpad4': 0x64, 'Numpad5': 0x65, 'Numpad6': 0x66, 'Numpad7': 0x67,
  'Numpad8': 0x68, 'Numpad9': 0x69,

  // Punctuation
  '-': 0xBD, '=': 0xBB, '[': 0xDB, ']': 0xDD, '\\': 0xDC,
  ';': 0xBA, "'": 0xDE, ',': 0xBC, '.': 0xBE, '/': 0xBF,
  '`': 0xC0
};

const MODIFIERS = new Set(['Ctrl', 'Control', 'Shift', 'Alt', 'Win', 'Meta']);

/**
 * Parse hotkey string into components
 */
export function parseHotkey(hotkey: string): { modifiers: string[]; mainKey: string } {
  const parts = hotkey.split('+').map(p => p.trim());
  const modifiers: string[] = [];
  let mainKey = '';

  for (const part of parts) {
    let normalized = part;
    if (part.toLowerCase() === 'control') normalized = 'Ctrl';
    if (part.toLowerCase() === 'meta' || part.toLowerCase() === 'cmd') normalized = 'Win';

    if (MODIFIERS.has(normalized)) {
      modifiers.push(normalized);
    } else {
      mainKey = part;
    }
  }

  return { modifiers, mainKey };
}

/**
 * Persistent PowerShell session for efficient hotkey simulation
 * Avoids 500ms-2s startup overhead per hotkey press
 */
class PowerShellSession {
  private ps: ChildProcess | null = null;
  private ready = false;
  private responseBuffer = '';
  private pendingResolve: ((value: void) => void) | null = null;

  async init(): Promise<void> {
    if (this.ready) return;

    return new Promise((resolve, reject) => {
      this.ps = spawn('powershell.exe', ['-NoProfile', '-Command', '-'], {
        stdio: ['pipe', 'pipe', 'pipe'],
        windowsHide: true
      });

      this.ps.stdout?.setEncoding('utf8');
      this.ps.stderr?.setEncoding('utf8');

      this.ps.stdout?.on('data', (data: string) => {
        this.responseBuffer += data;
        if (this.responseBuffer.includes('__DONE__')) {
          this.responseBuffer = '';
          if (this.pendingResolve) {
            const r = this.pendingResolve;
            this.pendingResolve = null;
            r();
          }
        }
        if (this.responseBuffer.includes('__READY__')) {
          this.responseBuffer = '';
          this.ready = true;
          resolve();
        }
      });

      this.ps.stderr?.on('data', (data: string) => {
        console.error('[PowerShell Error]', data);
      });

      this.ps.on('error', reject);
      this.ps.on('close', () => {
        this.ready = false;
        this.ps = null;
      });

      // Initialize the C# type definition once
      const initScript = `
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public class KeyHelper {
    [DllImport("user32.dll")]
    public static extern void keybd_event(byte bVk, byte bScan, int dwFlags, int dwExtraInfo);
    public const int KEYEVENTF_KEYUP = 0x0002;
}
"@
Write-Host "__READY__"
`;
      this.ps.stdin?.write(initScript);
    });
  }

  async pressKeys(keyCodes: number[], holdMs: number = 100): Promise<void> {
    if (!this.ready || !this.ps) {
      await this.init();
    }

    return new Promise((resolve) => {
      this.pendingResolve = resolve;

      const keyCodesStr = keyCodes.join(',');
      // Use 50ms delays for CI stability (30ms can be too fast on loaded runners)
      const command = `
$codes = @(${keyCodesStr})
foreach ($c in $codes) { [KeyHelper]::keybd_event($c, 0, 0, 0); Start-Sleep -Milliseconds 50 }
Start-Sleep -Milliseconds ${holdMs}
$rev = $codes[($codes.Length-1)..0]
foreach ($c in $rev) { [KeyHelper]::keybd_event($c, 0, [KeyHelper]::KEYEVENTF_KEYUP, 0); Start-Sleep -Milliseconds 50 }
Write-Host "__DONE__"
`;
      this.ps!.stdin?.write(command);
    });
  }

  cleanup(): void {
    if (this.ps) {
      this.ps.stdin?.write('exit\n');
      this.ps.kill();
      this.ps = null;
      this.ready = false;
    }
  }
}

// Singleton session
let psSession: PowerShellSession | null = null;

// Ensure cleanup on process exit (handles SIGINT, SIGTERM, uncaught exceptions)
function setupProcessCleanup() {
  const cleanup = () => {
    if (psSession) {
      try {
        psSession.cleanup();
        psSession = null;
      } catch (e) {
        // Ignore errors during cleanup
      }
    }
  };

  process.on('exit', cleanup);
  process.on('SIGINT', () => { cleanup(); process.exit(130); });
  process.on('SIGTERM', () => { cleanup(); process.exit(143); });
  process.on('uncaughtException', (err) => {
    console.error('Uncaught exception:', err);
    cleanup();
    process.exit(1);
  });
}

// Setup cleanup handlers once
setupProcessCleanup();

async function getSession(): Promise<PowerShellSession> {
  if (!psSession) {
    psSession = new PowerShellSession();
    await psSession.init();
  }
  return psSession;
}

/**
 * Press a global hotkey using persistent PowerShell session
 */
export async function pressGlobalHotkey(hotkey: string): Promise<void> {
  const { modifiers, mainKey } = parseHotkey(hotkey);

  const keyCodes: number[] = [];

  for (const mod of modifiers) {
    const code = VK_CODES[mod];
    if (code) keyCodes.push(code);
  }

  const mainCode = VK_CODES[mainKey] ?? VK_CODES[mainKey.toUpperCase()];
  if (mainCode) keyCodes.push(mainCode);

  if (keyCodes.length === 0) {
    throw new Error(`Invalid hotkey: ${hotkey}`);
  }

  const session = await getSession();
  await session.pressKeys(keyCodes, 100);
  console.log(`⌨️ Pressed hotkey: ${hotkey}`);
}

/**
 * Press and hold a hotkey for specified duration
 */
export async function pressAndHoldHotkey(hotkey: string, durationMs: number): Promise<void> {
  const { modifiers, mainKey } = parseHotkey(hotkey);

  const keyCodes: number[] = [];

  for (const mod of modifiers) {
    const code = VK_CODES[mod];
    if (code) keyCodes.push(code);
  }

  const mainCode = VK_CODES[mainKey] ?? VK_CODES[mainKey.toUpperCase()];
  if (mainCode) keyCodes.push(mainCode);

  const session = await getSession();
  await session.pressKeys(keyCodes, durationMs);
  console.log(`⌨️ Held hotkey ${hotkey} for ${durationMs}ms`);
}

/**
 * Cleanup PowerShell session (call in afterAll)
 */
export function cleanupHotkeySession(): void {
  if (psSession) {
    psSession.cleanup();
    psSession = null;
  }
}

/**
 * Test class for systematic hotkey testing
 */
export class HotkeyTester {
  private results: HotkeyTestResult[] = [];

  async testHotkey(
    browser: WebdriverIO.Browser,
    hotkey: string,
    expectedBehavior: () => Promise<boolean>,
    description: string
  ): Promise<HotkeyTestResult> {
    const startTime = Date.now();
    let success = false;
    let error: string | undefined;

    try {
      await pressGlobalHotkey(hotkey);

      // Wait for the action to take effect
      await browser.pause(300);

      success = await expectedBehavior();
    } catch (e: any) {
      error = e.message;
      success = false;
    }

    const result: HotkeyTestResult = {
      hotkey,
      description,
      success,
      error,
      duration: Date.now() - startTime,
      timestamp: new Date().toISOString()
    };

    this.results.push(result);
    console.log(`${success ? '✅' : '❌'} Hotkey test: ${description} (${hotkey})`);

    return result;
  }

  async testHotkeySequence(
    browser: WebdriverIO.Browser,
    hotkeys: string[],
    delayMs: number = 500
  ): Promise<void> {
    for (const hotkey of hotkeys) {
      await pressGlobalHotkey(hotkey);
      await browser.pause(delayMs);
    }
  }

  getResults(): HotkeyTestResult[] {
    return this.results;
  }

  getFailedTests(): HotkeyTestResult[] {
    return this.results.filter(r => !r.success);
  }

  saveResults(outputPath: string): void {
    const dir = path.dirname(outputPath);
    if (!fs.existsSync(dir)) {
      fs.mkdirSync(dir, { recursive: true });
    }
    fs.writeFileSync(outputPath, JSON.stringify(this.results, null, 2));
    console.log(`💾 Hotkey test results saved to: ${outputPath}`);
  }
}

interface HotkeyTestResult {
  hotkey: string;
  description: string;
  success: boolean;
  error?: string;
  duration: number;
  timestamp: string;
}

/**
 * Common hotkey test scenarios
 */
export const HOTKEY_TEST_SCENARIOS = [
  { hotkey: 'Ctrl+Shift+Space', description: 'Default recording hotkey' },
  { hotkey: 'Ctrl+Shift+S', description: 'Alternative hotkey Ctrl+Shift+S' },
  { hotkey: 'Alt+Space', description: 'Simple Alt+Space' },
  { hotkey: 'Ctrl+Alt+R', description: 'Ctrl+Alt+R combination' },
  { hotkey: 'F9', description: 'Function key only' },
  { hotkey: 'Ctrl+F9', description: 'Ctrl + Function key' },
];
