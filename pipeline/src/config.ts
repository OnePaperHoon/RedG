import * as fs from 'fs';
import * as path from 'path';
import * as yaml from 'js-yaml';

export interface Config {
  anthropic: {
    apiKey: string;
    model:  string;
  };
  openai: {
    apiKey:    string;
    ttsModel:  string;
    ttsVoice:  string;
  };
  nanobanana: {
    apiKey: string;
    width:  number;
    height: number;
  };
  youtube: {
    clientSecrets:  string;
    defaultPrivacy: string;
  };
  comfyui: {
    host: string;
  };
  ayg: {
    imageBackend: 'nanobanana' | 'comfyui';
    workspace:    string;
    output:       string;
  };
}

let _config: Config | null = null;

export function loadConfig(configPath = './config.yaml'): Config {
  if (_config) return _config;
  const resolved = path.resolve(configPath);
  if (!fs.existsSync(resolved)) {
    throw new Error(`config.yaml not found at ${resolved}. Run 'ayg init' first.`);
  }
  _config = yaml.load(fs.readFileSync(resolved, 'utf-8')) as Config;
  return _config;
}
