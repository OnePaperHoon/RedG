import Anthropic from '@anthropic-ai/sdk';
import { loadConfig } from '../config';
import type { Script } from '../types';

const SYSTEM_PROMPT = `당신은 YouTube 숏폼(9:16) 영상 스크립트 작성 전문가입니다.
주어진 주제로 5개 씬의 스크립트를 작성하세요.
반드시 유효한 JSON만 출력하고, 다른 텍스트는 포함하지 마세요.`;

const USER_TEMPLATE = (topic: string) => `주제: "${topic}"

다음 JSON 형식으로 스크립트를 작성하세요:
{
  "title": "영상 제목 (40자 이내)",
  "description": "영상 설명 (150자 이내)",
  "tags": ["태그1", "태그2", "태그3", "태그4", "태그5"],
  "scenes": [
    {
      "id": 1,
      "subtitle": "화면에 표시될 자막 (30자 이내, 핵심 문장)",
      "narration": "TTS로 읽을 내레이션 (2-3문장, 자연스럽게)",
      "imagePrompt": "장면을 표현하는 영어 이미지 프롬프트 (detailed, visual)",
      "duration": 5
    }
  ]
}
씬은 정확히 5개 작성하세요.`;

export async function generateScript(topic: string): Promise<Script> {
  const config = loadConfig();
  const client = new Anthropic({ apiKey: config.anthropic.apiKey });

  const response = await client.messages.create({
    model: config.anthropic.model,
    max_tokens: 2048,
    system: SYSTEM_PROMPT,
    messages: [{ role: 'user', content: USER_TEMPLATE(topic) }],
  });

  const text = response.content[0].type === 'text' ? response.content[0].text : '';

  // JSON 블록 추출 (```json ... ``` 형태 포함)
  const jsonMatch = text.match(/```json\s*([\s\S]*?)```/) || text.match(/(\{[\s\S]*\})/);
  if (!jsonMatch) {
    throw new Error(`Failed to extract JSON from Claude response:\n${text}`);
  }

  const raw = jsonMatch[1] ?? jsonMatch[0];
  const script = JSON.parse(raw) as Script;

  // 기본 유효성 검사
  if (!script.scenes || script.scenes.length === 0) {
    throw new Error('Script has no scenes');
  }

  return script;
}
