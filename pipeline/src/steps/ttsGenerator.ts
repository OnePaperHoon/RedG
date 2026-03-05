import OpenAI from 'openai';
import * as fs from 'fs';
import * as path from 'path';
import { loadConfig } from '../config';

type TTSVoice = 'alloy' | 'echo' | 'fable' | 'onyx' | 'nova' | 'shimmer';
type TTSModel = 'tts-1' | 'tts-1-hd';

export async function generateTTS(text: string, outputPath: string): Promise<void> {
  const config = loadConfig();
  const client = new OpenAI({ apiKey: config.openai.apiKey });

  fs.mkdirSync(path.dirname(outputPath), { recursive: true });

  const response = await client.audio.speech.create({
    model:  config.openai.ttsModel as TTSModel,
    voice:  config.openai.ttsVoice as TTSVoice,
    input:  text,
    response_format: 'mp3',
  });

  const buffer = Buffer.from(await response.arrayBuffer());
  fs.writeFileSync(outputPath, buffer);
}
