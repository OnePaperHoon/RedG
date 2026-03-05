use std::collections::{HashMap, VecDeque};
use serde::{Deserialize, Serialize};

// ── AppMode ──────────────────────────────────────────────────────────────────
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppMode {
    Dashboard,
    NewJob,
    ScriptReview,
    ImageConfig,
    ImageConfigGlobalPopup,
    Confirm(ConfirmAction),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmAction {
    DeleteJob(String),
    DeleteScene(usize),
}

// ── Job 상태 ─────────────────────────────────────────────────────────────────
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobStatus {
    Queued,
    GeneratingScript,
    AwaitingScriptReview,
    AwaitingImageConfig,
    GeneratingImages,
    GeneratingTTS,
    Composing,
    Uploading,
    Done,
    Failed(String),
}

impl JobStatus {
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Queued                 => "○",
            Self::AwaitingScriptReview
            | Self::AwaitingImageConfig  => "⏸",
            Self::Done                   => "✓",
            Self::Failed(_)              => "✕",
            _                            => "●",
        }
    }
    pub fn label(&self) -> &str {
        match self {
            Self::Queued                => "Queued",
            Self::GeneratingScript      => "Script",
            Self::AwaitingScriptReview  => "Review",
            Self::AwaitingImageConfig   => "ImgCfg",
            Self::GeneratingImages      => "Images",
            Self::GeneratingTTS         => "TTS",
            Self::Composing             => "Compose",
            Self::Uploading             => "Upload",
            Self::Done                  => "Done",
            Self::Failed(_)             => "Failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Step { Script, Images, Tts, Compose, Upload }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepStatus { Pending, Running, Done, Failed }

#[derive(Debug, Clone)]
pub struct Job {
    pub id:       String,
    pub topic:    String,
    pub status:   JobStatus,
    pub steps:    HashMap<Step, StepStatus>,
    pub progress: u8,
    pub logs:     VecDeque<LogEntry>,
}

impl Job {
    pub fn new(id: String, topic: String) -> Self {
        let mut steps = HashMap::new();
        steps.insert(Step::Script,  StepStatus::Pending);
        steps.insert(Step::Images,  StepStatus::Pending);
        steps.insert(Step::Tts,     StepStatus::Pending);
        steps.insert(Step::Compose, StepStatus::Pending);
        steps.insert(Step::Upload,  StepStatus::Pending);
        Self { id, topic, status: JobStatus::Queued, steps, progress: 0, logs: VecDeque::new() }
    }
}

// ── Script / Image 씬 타입 ───────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptScene {
    pub id:           u32,
    pub subtitle:     String,
    pub narration:    String,
    #[serde(rename = "imagePrompt")]
    pub image_prompt: String,
    pub duration:     u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageScene {
    pub id:       u32,
    pub prompt:   String,
    pub style:    String,
    pub negative: String,
}

// ── 로그 ─────────────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level:   String,
    pub message: String,
    pub job_id:  String,
}

// ── 편집 포커스 필드 ──────────────────────────────────────────────────────────
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SceneField { Subtitle, Narration }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageField { Prompt, Style, Negative }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GlobalStylePreset { Cinematic, Anime, Flat, Custom }

// ── 중앙 AppState ─────────────────────────────────────────────────────────────
pub struct AppState {
    pub mode:          AppMode,
    pub jobs:          Vec<Job>,
    pub selected_job:  usize,
    pub global_logs:   VecDeque<LogEntry>,
    pub should_quit:   bool,
    pub backend:       String,  // "nanobanana" | "comfyui"

    // NewJob 팝업
    pub new_job_input: String,

    // ScriptReview
    pub current_job_id:  Option<String>,
    pub script_scenes:   Vec<ScriptScene>,
    pub selected_scene:  usize,
    pub scene_edit_mode: bool,
    pub scene_field:     SceneField,
    pub scene_edit_buf:  String,

    // ImageConfig
    pub image_scenes:    Vec<ImageScene>,
    pub image_field:     ImageField,
    pub image_edit_mode: bool,
    pub image_edit_buf:  String,
    pub global_style:    String,
    pub global_negative: String,
    pub global_preset:   GlobalStylePreset,
    pub global_style_buf: String,
    pub global_neg_buf:  String,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            mode:          AppMode::Dashboard,
            jobs:          Vec::new(),
            selected_job:  0,
            global_logs:   VecDeque::new(),
            should_quit:   false,
            backend:       "nanobanana".into(),

            new_job_input: String::new(),

            current_job_id:  None,
            script_scenes:   Vec::new(),
            selected_scene:  0,
            scene_edit_mode: false,
            scene_field:     SceneField::Subtitle,
            scene_edit_buf:  String::new(),

            image_scenes:    Vec::new(),
            image_field:     ImageField::Prompt,
            image_edit_mode: false,
            image_edit_buf:  String::new(),
            global_style:    String::new(),
            global_negative: String::new(),
            global_preset:   GlobalStylePreset::Cinematic,
            global_style_buf: String::new(),
            global_neg_buf:  String::new(),
        }
    }

    pub fn selected_job(&self) -> Option<&Job> {
        self.jobs.get(self.selected_job)
    }

    pub fn selected_job_mut(&mut self) -> Option<&mut Job> {
        self.jobs.get_mut(self.selected_job)
    }

    pub fn job_mut(&mut self, job_id: &str) -> Option<&mut Job> {
        self.jobs.iter_mut().find(|j| j.id == job_id)
    }

    pub fn add_log(&mut self, entry: LogEntry) {
        if let Some(job) = self.jobs.iter_mut().find(|j| j.id == entry.job_id) {
            if job.logs.len() >= 200 { job.logs.pop_front(); }
            job.logs.push_back(entry.clone());
        }
        if self.global_logs.len() >= 500 { self.global_logs.pop_front(); }
        self.global_logs.push_back(entry);
    }
}
