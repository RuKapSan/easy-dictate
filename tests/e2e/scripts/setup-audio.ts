/**
 * Setup script to generate test audio files
 *
 * Run: npm run setup:audio
 */

import { setupTestAudioFiles, TestAudioGenerator, AudioPlayer } from '../utils/audio-mock';
import * as path from 'path';

async function main() {
  console.log('🎵 Setting up test audio files...\n');

  // List available TTS voices
  console.log('Available TTS voices:');
  const voices = TestAudioGenerator.listVoices();
  voices.forEach(v => console.log(`  - ${v}`));
  console.log();

  // List audio devices
  console.log('Available audio output devices:');
  const devices = AudioPlayer.listDevices();
  devices.forEach(d => console.log(`  - ${d}`));
  console.log();

  // Check for virtual audio cable
  const virtualCable = AudioPlayer.findVirtualCable();
  if (virtualCable) {
    console.log(`✅ Found virtual audio cable: ${virtualCable}`);
  } else {
    console.log('⚠️  No virtual audio cable found.');
    console.log('   For best results, install VB-CABLE or similar virtual audio device.');
    console.log('   Download from: https://vb-audio.com/Cable/');
  }
  console.log();

  // Generate test audio files
  console.log('Generating test audio files...');
  const files = await setupTestAudioFiles();

  console.log('\n✅ Test audio files ready:');
  Object.entries(files).forEach(([name, filePath]) => {
    console.log(`  ${name}: ${path.basename(filePath)}`);
  });

  console.log('\n🎉 Audio setup complete!');
}

main().catch(console.error);
