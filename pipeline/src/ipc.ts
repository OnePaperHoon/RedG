import { createInterface } from 'readline';
import type { Command, Event } from './types';

/** Node.js → Rust: NDJSON 이벤트 전송 */
export function send(event: Event): void {
  process.stdout.write(JSON.stringify(event) + '\n');
}

/** Rust → Node.js: NDJSON 커맨드 수신 (AsyncGenerator) */
export async function* recv(): AsyncGenerator<Command> {
  const rl = createInterface({ input: process.stdin, terminal: false });
  for await (const line of rl) {
    const trimmed = line.trim();
    if (trimmed) {
      try {
        yield JSON.parse(trimmed) as Command;
      } catch {
        process.stderr.write(`[ipc] invalid JSON: ${trimmed}\n`);
      }
    }
  }
}
