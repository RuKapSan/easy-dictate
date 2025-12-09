/**
 * Test utilities for Easy Dictate E2E tests
 */

import * as fs from 'fs';
import * as path from 'path';

const ARTIFACTS_DIR = path.join(__dirname, '../artifacts');
const SCREENSHOTS_DIR = path.join(ARTIFACTS_DIR, 'screenshots');
const LOGS_DIR = path.join(ARTIFACTS_DIR, 'logs');
const VIDEOS_DIR = path.join(ARTIFACTS_DIR, 'videos');

// Ensure directories exist
[SCREENSHOTS_DIR, LOGS_DIR, VIDEOS_DIR].forEach(dir => {
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }
});

/**
 * Test logger with file output
 */
export class TestLogger {
  private logFile: string;
  private testName: string;
  private logs: LogEntry[] = [];

  constructor(testName: string) {
    this.testName = testName.replace(/[^a-zA-Z0-9]/g, '_');
    this.logFile = path.join(LOGS_DIR, `${this.testName}_${Date.now()}.json`);
  }

  log(level: 'info' | 'warn' | 'error' | 'debug', message: string, data?: any) {
    const entry: LogEntry = {
      timestamp: new Date().toISOString(),
      level,
      message,
      data
    };
    this.logs.push(entry);

    const prefix = {
      info: '📝',
      warn: '⚠️',
      error: '❌',
      debug: '🔍'
    }[level];

    console.log(`${prefix} [${this.testName}] ${message}`, data ?? '');
  }

  info(message: string, data?: any) { this.log('info', message, data); }
  warn(message: string, data?: any) { this.log('warn', message, data); }
  error(message: string, data?: any) { this.log('error', message, data); }
  debug(message: string, data?: any) { this.log('debug', message, data); }

  save() {
    fs.writeFileSync(this.logFile, JSON.stringify(this.logs, null, 2));
    console.log(`📄 Logs saved to: ${this.logFile}`);
  }
}

interface LogEntry {
  timestamp: string;
  level: string;
  message: string;
  data?: any;
}

/**
 * Screenshot manager with comparison capabilities
 */
export class ScreenshotManager {
  private testName: string;
  private screenshotIndex = 0;

  constructor(testName: string) {
    this.testName = testName.replace(/[^a-zA-Z0-9]/g, '_');
  }

  async capture(browser: WebdriverIO.Browser, name: string): Promise<string> {
    this.screenshotIndex++;
    const filename = `${this.testName}_${String(this.screenshotIndex).padStart(3, '0')}_${name}.png`;
    const filepath = path.join(SCREENSHOTS_DIR, filename);

    await browser.saveScreenshot(filepath);
    console.log(`📸 Screenshot: ${filename}`);

    return filepath;
  }

  async captureWithMetadata(browser: WebdriverIO.Browser, name: string, metadata: any): Promise<string> {
    const screenshotPath = await this.capture(browser, name);

    // Save metadata alongside screenshot
    const metadataPath = screenshotPath.replace('.png', '.json');
    fs.writeFileSync(metadataPath, JSON.stringify({
      screenshot: path.basename(screenshotPath),
      timestamp: new Date().toISOString(),
      ...metadata
    }, null, 2));

    return screenshotPath;
  }

  /**
   * Capture sequence of screenshots with delay
   */
  async captureSequence(
    browser: WebdriverIO.Browser,
    name: string,
    count: number,
    intervalMs: number
  ): Promise<string[]> {
    const paths: string[] = [];
    for (let i = 0; i < count; i++) {
      paths.push(await this.capture(browser, `${name}_${i}`));
      if (i < count - 1) {
        await browser.pause(intervalMs);
      }
    }
    return paths;
  }
}

/**
 * Problem detector - analyzes UI state and logs issues
 */
export class ProblemDetector {
  private logger: TestLogger;
  private issues: Issue[] = [];

  constructor(logger: TestLogger) {
    this.logger = logger;
  }

  async checkUIState(browser: WebdriverIO.Browser): Promise<Issue[]> {
    const newIssues: Issue[] = [];

    try {
      // Check for error states
      const statusOrb = await browser.$('#status-orb');
      if (await statusOrb.isExisting()) {
        const className = await statusOrb.getAttribute('class');
        if (className?.includes('error')) {
          const statusText = await browser.$('#status-text').getText();
          newIssues.push({
            type: 'error_state',
            message: 'Application is in error state',
            details: { statusText, className }
          });
        }
      }

      // Check for toast errors
      const toast = await browser.$('#toast');
      if (await toast.isExisting() && await toast.isDisplayed()) {
        const toastType = await toast.getAttribute('data-type');
        if (toastType === 'error') {
          newIssues.push({
            type: 'toast_error',
            message: 'Error toast displayed',
            details: { text: await toast.getText() }
          });
        }
      }

      // Check for console errors (via Tauri logs)
      const consoleErrors = await browser.execute(() => {
        return (window as any).__testConsoleErrors ?? [];
      });
      if (consoleErrors.length > 0) {
        newIssues.push({
          type: 'console_error',
          message: 'Console errors detected',
          details: { errors: consoleErrors }
        });
      }

      // Check if elements are properly rendered
      const criticalElements = ['#status-orb', '#settings-form', '#provider'];
      for (const selector of criticalElements) {
        const element = await browser.$(selector);
        if (!await element.isExisting()) {
          newIssues.push({
            type: 'missing_element',
            message: `Critical element missing: ${selector}`,
            details: { selector }
          });
        }
      }
    } catch (e: any) {
      newIssues.push({
        type: 'check_error',
        message: 'Error during UI check',
        details: { error: e.message }
      });
    }

    // Log and store new issues
    newIssues.forEach(issue => {
      this.logger.error(`Issue detected: ${issue.type}`, issue.details);
      this.issues.push(issue);
    });

    return newIssues;
  }

  getIssues(): Issue[] {
    return this.issues;
  }

  hasIssues(): boolean {
    return this.issues.length > 0;
  }
}

interface Issue {
  type: string;
  message: string;
  details: any;
}

/**
 * Helper to wait for specific conditions
 */
export async function waitForCondition(
  browser: WebdriverIO.Browser,
  condition: () => Promise<boolean>,
  options: { timeout?: number; interval?: number; message?: string } = {}
): Promise<void> {
  const { timeout = 10000, interval = 100, message = 'Condition not met' } = options;

  await browser.waitUntil(condition, {
    timeout,
    interval,
    timeoutMsg: message
  });
}

/**
 * Helper to retry flaky operations
 */
export async function retry<T>(
  fn: () => Promise<T>,
  options: { retries?: number; delay?: number } = {}
): Promise<T> {
  const { retries = 3, delay = 1000 } = options;

  let lastError: Error | undefined;
  for (let i = 0; i < retries; i++) {
    try {
      return await fn();
    } catch (e: any) {
      lastError = e;
      console.log(`⟳ Retry ${i + 1}/${retries} after error: ${e.message}`);
      await new Promise(r => setTimeout(r, delay));
    }
  }
  throw lastError;
}

/**
 * Generate test report
 */
export function generateReport(
  testName: string,
  logger: TestLogger,
  screenshots: string[],
  issues: Issue[]
): string {
  const report = {
    testName,
    timestamp: new Date().toISOString(),
    summary: {
      totalIssues: issues.length,
      screenshotCount: screenshots.length
    },
    issues,
    screenshots: screenshots.map(s => path.basename(s))
  };

  const reportPath = path.join(LOGS_DIR, `report_${testName}_${Date.now()}.json`);
  fs.writeFileSync(reportPath, JSON.stringify(report, null, 2));

  console.log(`📊 Report generated: ${reportPath}`);
  return reportPath;
}
