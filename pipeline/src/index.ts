import { recv, send } from './ipc';
import { startJob, handleCommand } from './runner';

async function main(): Promise<void> {
  process.stderr.write('[pipeline] started\n');

  for await (const cmd of recv()) {
    switch (cmd.type) {
      case 'start_job':
        startJob(cmd.jobId, cmd.topic, cmd.backend).catch(err => {
          process.stderr.write(`[pipeline] job ${cmd.jobId} fatal: ${err}\n`);
          send({ type: 'step_error', jobId: cmd.jobId, step: 'script', error: String(err) });
        });
        break;

      case 'batch_start':
        for (const topic of cmd.topics) {
          const jobId = `batch_${Date.now()}_${Math.random().toString(36).slice(2, 7)}`;
          startJob(jobId, topic, 'nanobanana').catch(err => {
            process.stderr.write(`[pipeline] batch job ${jobId} fatal: ${err}\n`);
          });
        }
        break;

      default:
        handleCommand(cmd);
    }
  }

  process.stderr.write('[pipeline] stdin closed, exiting\n');
}

main().catch(err => {
  process.stderr.write(`[pipeline] fatal: ${err}\n`);
  process.exit(1);
});
