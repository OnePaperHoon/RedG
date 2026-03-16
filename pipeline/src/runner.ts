import * as fs from 'fs';
import * as path from 'path';
import { EventEmitter } from 'events';
import { send } from './ipc';
import { loadConfig } from './config';
import type { Command, Step, Backend, Script, EditedScene, ImageScene, ImageConfigScene } from './types';
import { generateScript } from './steps/scriptGenerator';
import { generateImage } from './steps/imageGenerator';
import { generateTTS } from './steps/ttsGenerator';
import { composeVideo } from './steps/videoComposer';

// ── 이벤트 버스 (커맨드 라우팅) ─────────────────────────────────────────────
const jobEvents = new EventEmitter();
jobEvents.setMaxListeners(200);

export function handleCommand(cmd: Command): void {
  if ('jobId' in cmd) {
    jobEvents.emit(`${cmd.type}:${(cmd as any).jobId}`, cmd);
  }
}

function waitFor<T>(type: string, jobId: string): Promise<T> {
  return new Promise<T>(resolve => {
    jobEvents.once(`${type}:${jobId}`, resolve as (...args: any[]) => void);
  });
}

// ── 체크포인트 ───────────────────────────────────────────────────────────────
interface StepState { done: boolean; ts: string | null; completedScenes?: number[]; }

interface Checkpoint {
  jobId:               string;
  topic:               string;
  created:             string;
  scriptApproved:      boolean;
  imageConfigApproved: boolean;
  steps:               Record<Step, StepState>;
  script:              Script | null;
  editedScript:        { scenes: EditedScene[] } | null;
  imageConfig:         { globalStyle: string; globalNegative: string; scenes: ImageScene[] } | null;
}

function workspaceDir(jobId: string): string {
  return path.resolve(loadConfig().ayg.workspace, jobId);
}

function logPath(jobId: string): string {
  return path.join(workspaceDir(jobId), 'log.jsonl');
}

function appendLog(jobId: string, level: string, message: string): void {
  const entry = JSON.stringify({ ts: new Date().toISOString(), level, message }) + '\n';
  try { fs.appendFileSync(logPath(jobId), entry); } catch { /* ignore */ }
}

function logAndSend(jobId: string, level: 'info' | 'warn' | 'error', message: string): void {
  appendLog(jobId, level, message);
  send({ type: 'log', jobId, level, message });
}

function cpPath(jobId: string): string {
  return path.join(workspaceDir(jobId), 'checkpoint.json');
}

function loadCp(jobId: string): Checkpoint | null {
  const p = cpPath(jobId);
  return fs.existsSync(p) ? (JSON.parse(fs.readFileSync(p, 'utf-8')) as Checkpoint) : null;
}

function saveCp(cp: Checkpoint): void {
  fs.mkdirSync(workspaceDir(cp.jobId), { recursive: true });
  fs.writeFileSync(cpPath(cp.jobId), JSON.stringify(cp, null, 2));
}

function initCp(jobId: string, topic: string): Checkpoint {
  return {
    jobId,
    topic,
    created:             new Date().toISOString(),
    scriptApproved:      false,
    imageConfigApproved: false,
    steps: {
      script:  { done: false, ts: null },
      images:  { done: false, ts: null },
      tts:     { done: false, ts: null },
      compose: { done: false, ts: null },
      upload:  { done: false, ts: null },
    },
    script:       null,
    editedScript: null,
    imageConfig:  null,
  };
}

// ── 메인 파이프라인 ──────────────────────────────────────────────────────────
const STEPS: Step[] = ['script', 'images', 'tts', 'compose', 'upload'];

export async function startJob(jobId: string, topic: string, backend: Backend): Promise<void> {
  send({ type: 'job_created', jobId, topic });

  const cp = loadCp(jobId) ?? initCp(jobId, topic);
  const wDir = workspaceDir(jobId);
  fs.mkdirSync(path.join(wDir, 'images'), { recursive: true });
  fs.mkdirSync(path.join(wDir, 'audio'),  { recursive: true });

  for (const step of STEPS) {
    if (cp.steps[step].done) continue;

    // ★ 사용자 개입 #1 — ScriptReview
    if (step === 'images' && !cp.scriptApproved) {
      await waitForScriptApproval(jobId, cp, wDir);
      saveCp(cp);

      // ImageConfig 준비
      const imageConfigScenes: ImageConfigScene[] = (cp.script?.scenes ?? []).map(s => ({
        id: s.id, prompt: s.imagePrompt, style: '', negative: '',
      }));
      send({ type: 'image_config_ready', jobId, scenes: imageConfigScenes });
    }

    // ★ 사용자 개입 #2 — ImageConfig
    if (step === 'images' && !cp.imageConfigApproved) {
      logAndSend(jobId, 'info', '이미지 설정을 기다리는 중...');
      const cmd = await waitFor<{ type: 'image_config_approved'; jobId: string;
        globalStyle: string; globalNegative: string; scenes: ImageScene[] }>(
        'image_config_approved', jobId,
      );
      cp.imageConfigApproved = true;
      cp.imageConfig = { globalStyle: cmd.globalStyle, globalNegative: cmd.globalNegative, scenes: cmd.scenes };
      saveCp(cp);
    }

    send({ type: 'step_start', jobId, step });
    try {
      await runStep(step, jobId, topic, cp, wDir, backend);
      cp.steps[step] = { done: true, ts: new Date().toISOString() };
      saveCp(cp);
      send({ type: 'step_done', jobId, step });
    } catch (e) {
      const errMsg = String(e);
      appendLog(jobId, 'error', `[${step}] ${errMsg}`);
      send({ type: 'step_error', jobId, step, error: errMsg });
      return;
    }
  }

  const finalPath = path.resolve(loadConfig().ayg.output, `${jobId}.mp4`);
  send({ type: 'job_done', jobId, url: finalPath });
}

// ── 사용자 승인 대기 (재생성 루프 포함) ─────────────────────────────────────
async function waitForScriptApproval(jobId: string, cp: Checkpoint, wDir: string): Promise<void> {
  logAndSend(jobId, 'info', '스크립트 검토를 기다리는 중...');

  while (true) {
    type ApproveCmd = { type: 'script_approved';  jobId: string; scenes: EditedScene[] };
    type RegenCmd   = { type: 'regenerate_script'; jobId: string };

    const result = await new Promise<ApproveCmd | RegenCmd>(resolve => {
      const onApprove = (cmd: ApproveCmd) => { cleanup(); resolve(cmd); };
      const onRegen   = (cmd: RegenCmd)   => { cleanup(); resolve(cmd); };
      const cleanup = () => {
        jobEvents.off(`script_approved:${jobId}`,      onApprove as any);
        jobEvents.off(`regenerate_script:${jobId}`,    onRegen   as any);
      };
      jobEvents.once(`script_approved:${jobId}`,   onApprove as any);
      jobEvents.once(`regenerate_script:${jobId}`, onRegen   as any);
    });

    if (result.type === 'script_approved') {
      cp.scriptApproved = true;
      cp.editedScript   = { scenes: result.scenes };
      return;
    }

    // 재생성
    send({ type: 'step_start', jobId, step: 'script' });
    logAndSend(jobId, 'info', '스크립트 재생성 중...');
    try {
      const newScript = await generateScript(cp.topic);
      cp.script = newScript;
      fs.writeFileSync(path.join(wDir, 'script.json'), JSON.stringify(newScript, null, 2));
      send({ type: 'step_done', jobId, step: 'script' });
      send({ type: 'script_ready', jobId, script: newScript });
      logAndSend(jobId, 'info', '스크립트 재생성 완료. 다시 검토해주세요.');
    } catch (e) {
      send({ type: 'step_error', jobId, step: 'script', error: String(e) });
      throw e;
    }
  }
}

// ── 단계별 실행 ──────────────────────────────────────────────────────────────
async function runStep(
  step:    Step,
  jobId:   string,
  topic:   string,
  cp:      Checkpoint,
  wDir:    string,
  backend: Backend,
): Promise<void> {
  switch (step) {
    case 'script':  return runScript(jobId, topic, cp, wDir);
    case 'images':  return runImages(jobId, cp, wDir, backend);
    case 'tts':     return runTTS(jobId, cp, wDir);
    case 'compose': return runCompose(jobId, cp, wDir);
    case 'upload':
      logAndSend(jobId, 'info', 'Upload: Phase 2에서 구현 예정');
  }
}

async function runScript(jobId: string, topic: string, cp: Checkpoint, wDir: string): Promise<void> {
  const script = await generateScript(topic);
  cp.script = script;
  fs.writeFileSync(path.join(wDir, 'script.json'), JSON.stringify(script, null, 2));

  const imageConfigScenes: ImageConfigScene[] = script.scenes.map(s => ({
    id: s.id, prompt: s.imagePrompt, style: '', negative: '',
  }));
  send({ type: 'script_ready', jobId, script });
  // image_config_ready는 waitForScriptApproval 후에 발송됨
  void imageConfigScenes; // suppress unused warning — used after approval
}

async function runImages(
  jobId:   string,
  cp:      Checkpoint,
  wDir:    string,
  backend: Backend,
): Promise<void> {
  if (!cp.imageConfig) throw new Error('imageConfig not set');
  const scenes = cp.imageConfig.scenes;
  const total  = scenes.length;

  if (!cp.steps.images.completedScenes) cp.steps.images.completedScenes = [];
  const done = new Set(cp.steps.images.completedScenes);

  for (let i = 0; i < scenes.length; i++) {
    const s = scenes[i];
    if (done.has(s.id)) {
      logAndSend(jobId, 'info', `이미지 씬 ${s.id} 이미 완료, 스킵`);
      send({ type: 'step_update', jobId, step: 'images', progress: Math.round((i + 1) / total * 100), detail: `${i + 1}/${total}` });
      continue;
    }

    const outPath = path.join(wDir, 'images', `scene_${String(s.id).padStart(2, '0')}.png`);
    const parts = [s.prompt, s.style, cp.imageConfig.globalStyle].filter(Boolean);
    const finalPrompt   = parts.join(', ');
    const finalNegative = [s.negative, cp.imageConfig.globalNegative].filter(Boolean).join(', ');

    send({ type: 'step_update', jobId, step: 'images', progress: Math.round(i / total * 100), detail: `${i + 1}/${total}` });
    logAndSend(jobId, 'info', `이미지 생성 ${i + 1}/${total} (씬 ${s.id})`);

    await generateImage(finalPrompt, finalNegative, outPath, backend);

    cp.steps.images.completedScenes!.push(s.id);
    saveCp(cp);
    send({ type: 'step_update', jobId, step: 'images', progress: Math.round((i + 1) / total * 100), detail: `${i + 1}/${total}` });
  }
}

async function runTTS(jobId: string, cp: Checkpoint, wDir: string): Promise<void> {
  if (!cp.script) throw new Error('script not set');
  const scenes: Array<{ id: number; narration: string }> =
    cp.editedScript?.scenes ?? cp.script.scenes;
  const total = scenes.length;

  if (!cp.steps.tts.completedScenes) cp.steps.tts.completedScenes = [];
  const done = new Set(cp.steps.tts.completedScenes);

  for (let i = 0; i < scenes.length; i++) {
    const s = scenes[i];
    if (done.has(s.id)) {
      logAndSend(jobId, 'info', `TTS 씬 ${s.id} 이미 완료, 스킵`);
      send({ type: 'step_update', jobId, step: 'tts', progress: Math.round((i + 1) / total * 100), detail: `${i + 1}/${total}` });
      continue;
    }

    const outPath = path.join(wDir, 'audio', `scene_${String(s.id).padStart(2, '0')}.mp3`);

    send({ type: 'step_update', jobId, step: 'tts', progress: Math.round(i / total * 100), detail: `${i + 1}/${total}` });
    logAndSend(jobId, 'info', `TTS 생성 ${i + 1}/${total} (씬 ${s.id})`);

    await generateTTS(s.narration, outPath);

    cp.steps.tts.completedScenes!.push(s.id);
    saveCp(cp);
    send({ type: 'step_update', jobId, step: 'tts', progress: Math.round((i + 1) / total * 100), detail: `${i + 1}/${total}` });
  }
}

async function runCompose(jobId: string, cp: Checkpoint, wDir: string): Promise<void> {
  if (!cp.script) throw new Error('script not set');

  const scenes: Array<{ id: number; subtitle: string; duration: number }> =
    cp.editedScript?.scenes ?? cp.script.scenes;

  const inputs = scenes.map(s => ({
    imagePath: path.join(wDir, 'images', `scene_${String(s.id).padStart(2, '0')}.png`),
    audioPath: path.join(wDir, 'audio',  `scene_${String(s.id).padStart(2, '0')}.mp3`),
    subtitle:  s.subtitle,
    duration:  s.duration,
  }));

  logAndSend(jobId, 'info', '영상 합성 중...');

  const clipPath = path.join(wDir, 'output.mp4');
  await composeVideo(inputs, clipPath);

  const config    = loadConfig();
  const finalPath = path.resolve(config.ayg.output, `${jobId}.mp4`);
  fs.mkdirSync(path.dirname(finalPath), { recursive: true });
  fs.copyFileSync(clipPath, finalPath);

  logAndSend(jobId, 'info', `영상 완성: ${finalPath}`);
}
