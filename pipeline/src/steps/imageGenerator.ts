import * as fs from 'fs';
import * as path from 'path';
import { GoogleGenAI } from '@google/genai';
import { loadConfig } from '../config';
import type { Backend } from '../types';

export async function generateImage(
  prompt: string,
  negative: string,
  outputPath: string,
  backend: Backend,
): Promise<void> {
  fs.mkdirSync(path.dirname(outputPath), { recursive: true });

  if (backend === 'nanobanana') {
    await generateNanobanana(prompt, negative, outputPath);
  } else {
    await generateComfyUI(prompt, negative, outputPath);
  }
}

async function generateNanobanana(
  prompt: string,
  negative: string,
  outputPath: string,
): Promise<void> {
  const config = loadConfig();
  const ai = new GoogleGenAI({ apiKey: config.nanobanana.apiKey });

  const fullPrompt = negative ? `${prompt}\n\nAvoid: ${negative}` : prompt;

  const response = await ai.models.generateContent({
    model: 'gemini-3.1-flash-image-preview',
    config: {
      responseModalities: ['IMAGE', 'TEXT'],
      imageConfig: { aspectRatio: '9:16' },
    },
    contents: [{ role: 'user', parts: [{ text: fullPrompt }] }],
  });

  const parts = response.candidates?.[0]?.content?.parts ?? [];
  const imagePart = parts.find((p: any) => p.inlineData?.data);
  if (!imagePart?.inlineData?.data) {
    throw new Error(`Gemini image API: no image received. Response: ${JSON.stringify(response)}`);
  }

  fs.writeFileSync(outputPath, Buffer.from(imagePart.inlineData.data, 'base64'));
}

async function generateComfyUI(
  prompt: string,
  negative: string,
  outputPath: string,
): Promise<void> {
  const config = loadConfig();
  const host = config.comfyui.host;

  // ComfyUI workflow — Phase 2에서 커스텀 workflow로 교체 가능
  const workflow = {
    '3': {
      class_type: 'KSampler',
      inputs: {
        seed: Math.floor(Math.random() * 1e9),
        steps: 20,
        cfg: 7,
        sampler_name: 'euler',
        scheduler: 'normal',
        denoise: 1,
        model: ['4', 0],
        positive: ['6', 0],
        negative: ['7', 0],
        latent_image: ['5', 0],
      },
    },
    '4': { class_type: 'CheckpointLoaderSimple', inputs: { ckpt_name: 'v1-5-pruned-emaonly.ckpt' } },
    '5': { class_type: 'EmptyLatentImage', inputs: { width: 1080, height: 1920, batch_size: 1 } },
    '6': { class_type: 'CLIPTextEncode', inputs: { text: prompt, clip: ['4', 1] } },
    '7': { class_type: 'CLIPTextEncode', inputs: { text: negative, clip: ['4', 1] } },
    '8': { class_type: 'VAEDecode', inputs: { samples: ['3', 0], vae: ['4', 2] } },
    '9': { class_type: 'SaveImage', inputs: { filename_prefix: 'ayg', images: ['8', 0] } },
  };

  const queueRes = await fetch(`${host}/prompt`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ prompt: workflow }),
  });
  if (!queueRes.ok) throw new Error(`ComfyUI queue error: ${queueRes.status}`);
  const { prompt_id } = await queueRes.json() as { prompt_id: string };

  // Poll for completion
  for (let i = 0; i < 120; i++) {
    await sleep(2000);
    const histRes = await fetch(`${host}/history/${prompt_id}`);
    if (!histRes.ok) continue;
    const hist = await histRes.json() as Record<string, any>;
    const item = hist[prompt_id];
    if (!item) continue;

    const outputs = item.outputs as Record<string, any>;
    for (const nodeId of Object.keys(outputs)) {
      const imgs = outputs[nodeId]?.images as { filename: string; subfolder: string; type: string }[];
      if (imgs?.[0]) {
        const { filename, subfolder, type } = imgs[0];
        const imgRes = await fetch(`${host}/view?filename=${filename}&subfolder=${subfolder}&type=${type}`);
        if (!imgRes.ok) throw new Error(`ComfyUI download error: ${imgRes.status}`);
        fs.writeFileSync(outputPath, Buffer.from(await imgRes.arrayBuffer()));
        return;
      }
    }
  }
  throw new Error('ComfyUI: timed out waiting for image');
}

function sleep(ms: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, ms));
}
