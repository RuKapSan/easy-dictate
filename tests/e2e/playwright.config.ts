/**
 * Playwright configuration for Tauri E2E tests
 *
 * Alternative to WebdriverIO - simpler setup, similar capabilities
 *
 * Usage:
 * 1. npm install @playwright/test
 * 2. npx playwright test
 */

import { defineConfig, devices } from '@playwright/test';
import * as path from 'path';

const APP_PATH = path.join(__dirname, '../../src-tauri/target/debug/app.exe');

export default defineConfig({
  testDir: './specs-playwright',
  fullyParallel: false, // Run tests sequentially for Tauri
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: 1,
  reporter: [
    ['html', { outputFolder: 'playwright-report' }],
    ['json', { outputFile: 'logs/playwright-results.json' }],
    ['list']
  ],

  timeout: 60000,

  use: {
    trace: 'on-first-retry',
    screenshot: 'on',
    video: 'retain-on-failure',
  },

  outputDir: 'playwright-artifacts',

  // Note: Playwright doesn't have native Tauri support
  // You'll need to launch the app manually and connect via CDP or use electron-like approach
});
