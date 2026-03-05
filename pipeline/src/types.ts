// ── IPC Commands (Rust → Node.js) ──────────────────────────────────────────
export type Command =
  | { type: 'start_job';            jobId: string; topic: string; backend: Backend }
  | { type: 'script_approved';       jobId: string; scenes: EditedScene[] }
  | { type: 'image_config_approved'; jobId: string; globalStyle: string;
                                      globalNegative: string; scenes: ImageScene[] }
  | { type: 'regenerate_script';     jobId: string }
  | { type: 'cancel_job';            jobId: string }
  | { type: 'resume_job';            jobId: string }
  | { type: 'batch_start';           topics: string[] };

// ── IPC Events (Node.js → Rust) ────────────────────────────────────────────
export type Event =
  | { type: 'job_created';        jobId: string; topic: string }
  | { type: 'script_ready';       jobId: string; script: Script }
  | { type: 'image_config_ready'; jobId: string; scenes: ImageConfigScene[] }
  | { type: 'step_start';         jobId: string; step: Step }
  | { type: 'step_update';        jobId: string; step: Step; progress: number; detail?: string }
  | { type: 'step_done';          jobId: string; step: Step }
  | { type: 'step_error';         jobId: string; step: Step; error: string }
  | { type: 'log';                jobId: string; level: LogLevel; message: string }
  | { type: 'job_done';           jobId: string; url: string };

// ── Domain Types ────────────────────────────────────────────────────────────
export type Step     = 'script' | 'images' | 'tts' | 'compose' | 'upload';
export type Backend  = 'nanobanana' | 'comfyui';
export type LogLevel = 'info' | 'warn' | 'error';

export interface Script {
  title:       string;
  description: string;
  tags:        string[];
  scenes:      ScriptScene[];
}

export interface ScriptScene {
  id:          number;
  subtitle:    string;
  narration:   string;
  imagePrompt: string;
  duration:    number;
}

export interface EditedScene {
  id:        number;
  subtitle:  string;
  narration: string;
  duration:  number;
}

export interface ImageScene {
  id:       number;
  prompt:   string;
  style:    string;
  negative: string;
}

export interface ImageConfigScene {
  id:       number;
  prompt:   string;
  style:    string;
  negative: string;
}
