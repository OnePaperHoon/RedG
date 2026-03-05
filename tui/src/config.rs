use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub anthropic:  AnthropicConfig,
    pub openai:     OpenAIConfig,
    pub nanobanana: NanobananaConfig,
    pub youtube:    YouTubeConfig,
    pub comfyui:    ComfyUIConfig,
    pub ayg:        AygConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnthropicConfig {
    #[serde(rename = "apiKey", default)]
    pub api_key: String,
    #[serde(default = "default_anthropic_model")]
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OpenAIConfig {
    #[serde(rename = "apiKey", default)]
    pub api_key: String,
    #[serde(rename = "ttsModel", default = "default_tts_model")]
    pub tts_model: String,
    #[serde(rename = "ttsVoice", default = "default_tts_voice")]
    pub tts_voice: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NanobananaConfig {
    #[serde(rename = "apiKey", default)]
    pub api_key: String,
    #[serde(default = "default_width")]
    pub width: u32,
    #[serde(default = "default_height")]
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct YouTubeConfig {
    #[serde(rename = "clientSecrets", default = "default_client_secrets")]
    pub client_secrets: String,
    #[serde(rename = "defaultPrivacy", default = "default_privacy")]
    pub default_privacy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComfyUIConfig {
    #[serde(default = "default_comfyui_host")]
    pub host: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AygConfig {
    #[serde(rename = "imageBackend", default = "default_image_backend")]
    pub image_backend: String,
    #[serde(default = "default_workspace")]
    pub workspace: String,
    #[serde(default = "default_output")]
    pub output: String,
}

impl Default for AygConfig {
    fn default() -> Self {
        Self {
            image_backend: default_image_backend(),
            workspace:     default_workspace(),
            output:        default_output(),
        }
    }
}

fn default_anthropic_model() -> String { "claude-sonnet-4-20250514".into() }
fn default_tts_model()       -> String { "tts-1".into() }
fn default_tts_voice()       -> String { "nova".into() }
fn default_width()           -> u32    { 1080 }
fn default_height()          -> u32    { 1920 }
fn default_client_secrets()  -> String { "./client_secrets.json".into() }
fn default_privacy()         -> String { "private".into() }
fn default_comfyui_host()    -> String { "http://127.0.0.1:8188".into() }
fn default_image_backend()   -> String { "nanobanana".into() }
fn default_workspace()       -> String { "./workspace".into() }
fn default_output()          -> String { "./output".into() }

pub const CONFIG_PATH: &str = "./config.yaml";

pub fn config_exists() -> bool {
    Path::new(CONFIG_PATH).exists()
}

pub fn load_config() -> anyhow::Result<Config> {
    let raw = std::fs::read_to_string(CONFIG_PATH)?;
    let cfg: Config = serde_yaml::from_str(&raw)?;
    Ok(cfg)
}

pub fn save_config(cfg: &Config) -> anyhow::Result<()> {
    let yaml = serde_yaml::to_string(cfg)?;
    std::fs::write(CONFIG_PATH, yaml)?;
    Ok(())
}

/// ayg init 온보딩 결과로 config.yaml 생성
pub fn write_initial_config(
    anthropic_key: &str,
    openai_key:    &str,
    nb_key:        &str,
    yt_secrets:    &str,
    comfyui_host:  &str,
) -> anyhow::Result<()> {
    let cfg = Config {
        anthropic:  AnthropicConfig  { api_key: anthropic_key.into(), model: default_anthropic_model() },
        openai:     OpenAIConfig     { api_key: openai_key.into(), tts_model: default_tts_model(), tts_voice: default_tts_voice() },
        nanobanana: NanobananaConfig { api_key: nb_key.into(), width: default_width(), height: default_height() },
        youtube:    YouTubeConfig    { client_secrets: if yt_secrets.is_empty() { default_client_secrets() } else { yt_secrets.into() }, default_privacy: default_privacy() },
        comfyui:    ComfyUIConfig    { host: if comfyui_host.is_empty() { default_comfyui_host() } else { comfyui_host.into() } },
        ayg:        AygConfig::default(),
    };
    save_config(&cfg)
}
