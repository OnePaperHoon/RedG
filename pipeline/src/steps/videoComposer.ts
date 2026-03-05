import ffmpeg from 'fluent-ffmpeg';
import * as fs from 'fs';
import * as path from 'path';

export interface SceneInput {
  imagePath: string;
  audioPath: string;
  subtitle:  string;
  duration:  number;
}

export async function composeVideo(scenes: SceneInput[], outputPath: string): Promise<void> {
  fs.mkdirSync(path.dirname(outputPath), { recursive: true });

  const tmpDir = path.dirname(outputPath);
  const clipPaths: string[] = [];

  for (let i = 0; i < scenes.length; i++) {
    const scene = scenes[i];
    const clipPath = path.join(tmpDir, `_clip_${i + 1}.mp4`);
    clipPaths.push(clipPath);
    await renderClip(scene, clipPath);
  }

  await concatenateClips(clipPaths, outputPath);

  // 임시 클립 정리
  for (const p of clipPaths) {
    try { fs.unlinkSync(p); } catch { /* ignore */ }
  }
}

function escapeDrawtext(text: string): string {
  // FFmpeg drawtext 특수문자 이스케이프
  return text
    .replace(/\\/g, '\\\\')
    .replace(/'/g, "\\'")
    .replace(/:/g, '\\:')
    .replace(/\[/g, '\\[')
    .replace(/\]/g, '\\]');
}

function renderClip(scene: SceneInput, outputPath: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const subtitle = escapeDrawtext(scene.subtitle);
    const drawtextFilter =
      `drawtext=text='${subtitle}':` +
      `fontcolor=white:fontsize=48:` +
      `x=(w-text_w)/2:y=h-160:` +
      `box=1:boxcolor=black@0.6:boxborderw=8:` +
      `line_spacing=8`;

    ffmpeg()
      .input(scene.imagePath)
      .inputOptions(['-loop 1'])
      .input(scene.audioPath)
      .videoCodec('libx264')
      .audioCodec('aac')
      .outputOptions([
        '-pix_fmt yuv420p',
        '-shortest',
        `-vf ${drawtextFilter}`,
        '-preset fast',
        '-crf 23',
      ])
      .output(outputPath)
      .on('end', () => resolve())
      .on('error', (err) => reject(new Error(`FFmpeg clip error: ${err.message}`)))
      .run();
  });
}

function concatenateClips(clipPaths: string[], outputPath: string): Promise<void> {
  const listPath = path.join(path.dirname(outputPath), '_concat_list.txt');
  const content = clipPaths.map(p => `file '${p.replace(/\\/g, '/')}'`).join('\n');
  fs.writeFileSync(listPath, content);

  return new Promise((resolve, reject) => {
    ffmpeg()
      .input(listPath)
      .inputOptions(['-f concat', '-safe 0'])
      .videoCodec('copy')
      .audioCodec('copy')
      .output(outputPath)
      .on('end', () => {
        try { fs.unlinkSync(listPath); } catch { /* ignore */ }
        resolve();
      })
      .on('error', (err) => {
        try { fs.unlinkSync(listPath); } catch { /* ignore */ }
        reject(new Error(`FFmpeg concat error: ${err.message}`));
      })
      .run();
  });
}
