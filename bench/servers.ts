import { spawn, type ChildProcess } from 'node:child_process';
import {
  OSSIDO_DIR,
  OSSIDO_BINARY,
  NEXT_DIR,
  OSSIDO_PORT,
  NEXT_PORT,
  cpuCount,
  type Framework,
  type ThreadMode,
} from './config.ts';

export interface RunningServer {
  framework: Framework;
  threads: number;
  /** Process-group id of the server tree (= the spawned child's pid, since it
   *  is started detached/setsid). Used to sum RSS across all worker processes. */
  pgid: number;
  stop: () => Promise<void>;
}

const sleep = (ms: number): Promise<void> => new Promise((r) => setTimeout(r, ms));

/** Poll an HTTP endpoint until it responds, or throw after `timeoutMs`. */
async function waitForServer(url: string, timeoutMs = 30000): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const res = await fetch(url);
      if (res.ok) return;
    } catch {
      // not up yet
    }
    await sleep(200);
  }
  throw new Error(`Server at ${url} did not become ready within ${timeoutMs}ms`);
}

/** Kill a detached child and its whole process group. */
function killGroup(child: ChildProcess): Promise<void> {
  return new Promise((resolve) => {
    if (child.exitCode !== null || child.signalCode !== null || child.pid === undefined) {
      resolve();
      return;
    }
    child.once('exit', () => resolve());
    try {
      process.kill(-child.pid, 'SIGKILL');
    } catch {
      try {
        child.kill('SIGKILL');
      } catch {
        // already gone
      }
    }
    // Safety net if no exit event fires.
    setTimeout(resolve, 2000);
  });
}

export function threadsFor(mode: ThreadMode): number {
  return mode === 'single' ? 1 : cpuCount();
}

export async function startOssido(framework: Framework, threads: number): Promise<RunningServer> {
  const child = spawn(OSSIDO_BINARY, {
    cwd: OSSIDO_DIR,
    detached: true,
    stdio: 'ignore',
    env: {
      ...process.env,
      OSSIDO_SSR_THREADS: String(threads),
    },
  });
  child.unref();
  await waitForServer(`http://127.0.0.1:${OSSIDO_PORT}/ssr`);
  return { framework, threads, pgid: child.pid!, stop: () => killGroup(child) };
}

export async function startNext(framework: Framework, workers: number): Promise<RunningServer> {
  const child = spawn('node', ['server.mjs'], {
    cwd: NEXT_DIR,
    detached: true,
    stdio: 'ignore',
    env: {
      ...process.env,
      NODE_ENV: 'production',
      PORT: String(NEXT_PORT),
      WEB_CONCURRENCY: String(workers),
    },
  });
  child.unref();
  await waitForServer(`http://127.0.0.1:${NEXT_PORT}/ssr`);
  return { framework, threads: workers, pgid: child.pid!, stop: () => killGroup(child) };
}

/** Start a framework's server with an explicit thread/worker count. */
export async function startServer(framework: Framework, threads: number): Promise<RunningServer> {
  return framework.key === 'ossido'
    ? startOssido(framework, threads)
    : startNext(framework, threads);
}
