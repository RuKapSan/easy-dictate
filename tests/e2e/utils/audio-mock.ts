/**
 * Audio mock utilities for Easy Dictate tests
 *
 * Provides methods to inject test audio into the application:
 * 1. Via Tauri backend command (RECOMMENDED - bypasses hardware completely)
 * 2. Via virtual audio cable (VB-CABLE, etc.)
 * 3. Via pre-recorded WAV files
 *
 * NOTE: For reliable CI testing, use the backend inject_test_audio command
 * instead of audio playback methods.
 */

import { execSync, spawn, ChildProcess } from 'child_process';
import * as fs from 'fs';
import * as path from 'path';

const AUDIO_MOCKS_DIR = path.join(__dirname, '../audio-mocks');
const isCI = process.env.CI === 'true' || process.env.GITHUB_ACTIONS === 'true';

// Ensure audio mocks directory exists
if (!fs.existsSync(AUDIO_MOCKS_DIR)) {
  fs.mkdirSync(AUDIO_MOCKS_DIR, { recursive: true });
}

/**
 * Check if audio playback is available
 * In CI, audio playback via speakers won't work
 */
export function checkAudioPlaybackAvailable(): { available: boolean; reason?: string } {
  if (isCI) {
    return {
      available: false,
      reason: 'Running in CI environment - no audio hardware available. Use browser.injectTestAudio() instead.'
    };
  }

  const virtualCable = AudioPlayer.findVirtualCable();
  if (!virtualCable) {
    return {
      available: false,
      reason: 'No virtual audio cable found. Install VB-CABLE or use browser.injectTestAudio() for reliable testing.'
    };
  }

  return { available: true };
}

/**
 * Audio player that plays to a specific output device
 */
export class AudioPlayer {
  private ffplayProcess: ChildProcess | null = null;

  /**
   * List available audio output devices
   */
  static listDevices(): string[] {
    try {
      // Use PowerShell to list audio devices
      const result = execSync(`powershell -Command "Get-AudioDevice -List | Select-Object Name, Type | ConvertTo-Json"`, {
        encoding: 'utf8',
        windowsHide: true
      });

      const devices = JSON.parse(result);
      return devices
        .filter((d: any) => d.Type === 'Playback')
        .map((d: any) => d.Name);
    } catch {
      // Fallback: try ffmpeg to list devices
      try {
        const result = execSync('ffmpeg -list_devices true -f dshow -i dummy 2>&1', {
          encoding: 'utf8',
          windowsHide: true
        });

        const lines = result.split('\n');
        const audioDevices: string[] = [];
        let isAudio = false;

        for (const line of lines) {
          if (line.includes('DirectShow audio devices')) {
            isAudio = true;
          } else if (line.includes('DirectShow video devices')) {
            isAudio = false;
          } else if (isAudio && line.includes('"')) {
            const match = line.match(/"([^"]+)"/);
            if (match) audioDevices.push(match[1]);
          }
        }

        return audioDevices;
      } catch {
        return [];
      }
    }
  }

  /**
   * Find virtual audio cable device (VB-CABLE, etc.)
   */
  static findVirtualCable(): string | null {
    const devices = this.listDevices();
    const virtualCablePatterns = [
      /vb-cable/i,
      /virtual.*cable/i,
      /cable input/i,
      /voicemeeter/i,
      /virtual audio/i
    ];

    for (const device of devices) {
      for (const pattern of virtualCablePatterns) {
        if (pattern.test(device)) {
          return device;
        }
      }
    }

    return null;
  }

  /**
   * Play audio file using ffplay
   */
  async playFile(filePath: string, deviceName?: string): Promise<void> {
    return new Promise((resolve, reject) => {
      const args = [
        '-nodisp',
        '-autoexit',
        '-hide_banner',
        '-loglevel', 'error'
      ];

      if (deviceName) {
        // Use DirectShow output for specific device
        args.push('-f', 'dshow', '-audio_device_number', deviceName);
      }

      args.push(filePath);

      console.log(`🔊 Playing audio: ${path.basename(filePath)}`);

      this.ffplayProcess = spawn('ffplay', args, {
        stdio: 'pipe',
        windowsHide: true
      });

      this.ffplayProcess.on('close', (code) => {
        this.ffplayProcess = null;
        if (code === 0) {
          resolve();
        } else {
          reject(new Error(`ffplay exited with code ${code}`));
        }
      });

      this.ffplayProcess.on('error', (err) => {
        this.ffplayProcess = null;
        reject(err);
      });
    });
  }

  /**
   * Stop current playback
   */
  stop(): void {
    if (this.ffplayProcess) {
      this.ffplayProcess.kill();
      this.ffplayProcess = null;
    }
  }

  /**
   * Play via Windows Media Player COM (PowerShell)
   */
  async playViaPowerShell(filePath: string): Promise<void> {
    const script = `
$player = New-Object System.Media.SoundPlayer
$player.SoundLocation = "${filePath.replace(/\\/g, '\\\\')}"
$player.PlaySync()
    `;

    return new Promise((resolve, reject) => {
      const ps = spawn('powershell.exe', ['-Command', script], {
        stdio: 'pipe',
        windowsHide: true
      });

      ps.on('close', (code) => {
        if (code === 0) resolve();
        else reject(new Error(`PowerShell exited with code ${code}`));
      });

      ps.on('error', reject);
    });
  }
}

/**
 * Test audio generator
 */
export class TestAudioGenerator {
  /**
   * Generate a test WAV file with spoken text using Windows TTS
   */
  static async generateTTS(text: string, outputPath: string, voice?: string): Promise<string> {
    const script = `
Add-Type -AssemblyName System.Speech
$synth = New-Object System.Speech.Synthesis.SpeechSynthesizer

${voice ? `$synth.SelectVoice("${voice}")` : ''}

$synth.SetOutputToWaveFile("${outputPath.replace(/\\/g, '\\\\')}")
$synth.Speak("${text.replace(/"/g, '\\"')}")
$synth.Dispose()
Write-Host "Generated: ${outputPath}"
    `;

    return new Promise((resolve, reject) => {
      const ps = spawn('powershell.exe', ['-Command', script], {
        stdio: 'pipe',
        windowsHide: true
      });

      ps.on('close', (code) => {
        if (code === 0 && fs.existsSync(outputPath)) {
          console.log(`🎤 Generated TTS audio: ${path.basename(outputPath)}`);
          resolve(outputPath);
        } else {
          reject(new Error('Failed to generate TTS audio'));
        }
      });

      ps.on('error', reject);
    });
  }

  /**
   * List available TTS voices
   */
  static listVoices(): string[] {
    try {
      const result = execSync(`powershell -Command "Add-Type -AssemblyName System.Speech; $synth = New-Object System.Speech.Synthesis.SpeechSynthesizer; $synth.GetInstalledVoices() | ForEach-Object { $_.VoiceInfo.Name }"`, {
        encoding: 'utf8',
        windowsHide: true
      });

      return result.split('\n').map(v => v.trim()).filter(Boolean);
    } catch {
      return [];
    }
  }

  /**
   * Generate silence WAV file
   */
  static generateSilence(durationMs: number, outputPath: string): string {
    const sampleRate = 16000;
    const numSamples = Math.floor(sampleRate * (durationMs / 1000));
    const buffer = Buffer.alloc(44 + numSamples * 2);

    // WAV header
    buffer.write('RIFF', 0);
    buffer.writeUInt32LE(36 + numSamples * 2, 4);
    buffer.write('WAVE', 8);
    buffer.write('fmt ', 12);
    buffer.writeUInt32LE(16, 16); // fmt chunk size
    buffer.writeUInt16LE(1, 20);  // PCM
    buffer.writeUInt16LE(1, 22);  // mono
    buffer.writeUInt32LE(sampleRate, 24);
    buffer.writeUInt32LE(sampleRate * 2, 28);
    buffer.writeUInt16LE(2, 32);
    buffer.writeUInt16LE(16, 34);
    buffer.write('data', 36);
    buffer.writeUInt32LE(numSamples * 2, 40);

    // Silence data (zeros)
    fs.writeFileSync(outputPath, buffer);
    return outputPath;
  }
}

/**
 * Audio injector for tests
 *
 * WARNING: This class uses system audio playback which requires:
 * - A virtual audio cable (VB-CABLE) for reliable testing
 * - Not running in CI environment
 *
 * For CI and reliable testing, use browser.injectTestAudio() instead,
 * which bypasses the audio hardware completely.
 */
export class AudioInjector {
  private player: AudioPlayer;
  private virtualDevice: string | null;
  private isAvailable: boolean;

  constructor() {
    const check = checkAudioPlaybackAvailable();

    if (!check.available) {
      console.warn(`⚠️ AudioInjector: ${check.reason}`);
      console.warn(`⚠️ Use browser.injectTestAudio(filePath) for reliable testing.`);

      // In CI, throw error to fail fast
      if (isCI) {
        throw new Error(
          'AudioInjector cannot be used in CI environment. ' +
          'Use browser.injectTestAudio() to inject audio directly via Tauri backend.'
        );
      }
    }

    this.player = new AudioPlayer();
    this.virtualDevice = AudioPlayer.findVirtualCable();
    this.isAvailable = check.available;

    if (this.virtualDevice) {
      console.log(`🔌 Found virtual audio device: ${this.virtualDevice}`);
    } else {
      console.log(`⚠️ No virtual audio cable found. Audio injection may not work correctly.`);
    }
  }

  /**
   * Check if audio injection is available
   */
  canInject(): boolean {
    return this.isAvailable && this.virtualDevice !== null;
  }

  /**
   * Inject audio file during test
   */
  async injectAudio(filePath: string): Promise<void> {
    if (this.virtualDevice) {
      await this.player.playFile(filePath, this.virtualDevice);
    } else {
      await this.player.playViaPowerShell(filePath);
    }
  }

  /**
   * Inject TTS audio with custom text
   */
  async injectTTS(text: string): Promise<void> {
    const tempFile = path.join(AUDIO_MOCKS_DIR, `tts_${Date.now()}.wav`);
    await TestAudioGenerator.generateTTS(text, tempFile);
    await this.injectAudio(tempFile);

    // Cleanup temp file
    setTimeout(() => {
      try {
        fs.unlinkSync(tempFile);
      } catch {}
    }, 5000);
  }

  /**
   * Stop any current audio injection
   */
  stop(): void {
    this.player.stop();
  }
}

/**
 * Pre-built test audio scenarios
 */
export const TEST_AUDIO_SCENARIOS = {
  // Simple phrases for testing
  HELLO_WORLD: 'Hello world, this is a test',
  SHORT_PHRASE: 'Testing one two three',
  LONG_TEXT: 'This is a longer piece of text designed to test the transcription system with continuous speech that goes on for several seconds.',

  // Edge cases
  NUMBERS: 'The number is twelve thousand three hundred and forty five',
  PUNCTUATION: 'Hello! How are you? I am fine, thank you.',
  MIXED_LANGUAGE: 'Привет Hello Bonjour',

  // Technical terms
  CODE_SPEAK: 'Function get user by ID returns a promise with user data',
  EMAIL: 'My email is test at example dot com'
};

/**
 * Setup mock audio files for tests
 */
export async function setupTestAudioFiles(): Promise<Record<string, string>> {
  const files: Record<string, string> = {};

  for (const [name, text] of Object.entries(TEST_AUDIO_SCENARIOS)) {
    const filePath = path.join(AUDIO_MOCKS_DIR, `${name.toLowerCase()}.wav`);

    if (!fs.existsSync(filePath)) {
      console.log(`📝 Generating test audio: ${name}`);
      await TestAudioGenerator.generateTTS(text, filePath);
    }

    files[name] = filePath;
  }

  // Generate silence files of various lengths
  for (const duration of [1000, 3000, 5000]) {
    const filePath = path.join(AUDIO_MOCKS_DIR, `silence_${duration}ms.wav`);
    if (!fs.existsSync(filePath)) {
      TestAudioGenerator.generateSilence(duration, filePath);
    }
    files[`SILENCE_${duration}MS`] = filePath;
  }

  return files;
}
