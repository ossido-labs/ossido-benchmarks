import { readdirSync, readFileSync, writeFileSync, existsSync, statSync } from 'node:fs';
import { resolve } from 'node:path';
import { ROOT } from './config.ts';

// Maintains the results index table in README.md. Scans results/<version>/,
// reads each run's JSON for the framework versions, and rewrites the table
// between the BENCH_TABLE markers. Run standalone (`bun run bench:index`) or
// automatically at the end of `bun run bench` / `bun run bench:memory`.

const RESULTS_ROOT = resolve(ROOT, 'results');
const README = resolve(ROOT, 'README.md');
const START = '<!-- BENCH_TABLE:START -->';
const END = '<!-- BENCH_TABLE:END -->';

interface Row {
  ossido: string;
  next: string;
  dir: string;
  hasThroughput: boolean;
  hasMemory: boolean;
  generatedAt: string;
}

function readMeta(dir: string): { ossido?: string; next?: string; generatedAt?: string } {
  for (const file of ['results.json', 'memory.json']) {
    const p = resolve(dir, file);
    if (!existsSync(p)) continue;
    try {
      const j = JSON.parse(readFileSync(p, 'utf8'));
      return { ossido: j.versions?.ossido, next: j.versions?.next, generatedAt: j.generatedAt };
    } catch {
      // ignore malformed JSON, try the next file
    }
  }
  return {};
}

export function buildTable(): string {
  const dirs = existsSync(RESULTS_ROOT)
    ? readdirSync(RESULTS_ROOT).filter((d) => statSync(resolve(RESULTS_ROOT, d)).isDirectory())
    : [];

  const rows: Row[] = dirs.map((d) => {
    const dir = resolve(RESULTS_ROOT, d);
    const meta = readMeta(dir);
    return {
      ossido: meta.ossido ?? d,
      next: meta.next ?? '—',
      dir: d,
      hasThroughput: existsSync(resolve(dir, 'RESULTS.md')),
      hasMemory: existsSync(resolve(dir, 'MEMORY.md')),
      generatedAt: meta.generatedAt ?? '',
    };
  });

  // Newest first (by run timestamp, then version string).
  rows.sort((a, b) => b.generatedAt.localeCompare(a.generatedAt) || b.ossido.localeCompare(a.ossido));

  if (!rows.length) return '_No results yet — run `bun run bench` and `bun run bench:memory`._';

  const header =
    '| Ossido version | Next version | Throughput result | Memory result |\n' +
    '| --- | --- | --- | --- |';
  const body = rows
    .map((r) => {
      const base = `./results/${r.dir}`;
      const tp = r.hasThroughput
        ? `[RESULTS.md](${base}/RESULTS.md) · [json](${base}/results.json)`
        : '—';
      const mem = r.hasMemory
        ? `[MEMORY.md](${base}/MEMORY.md) · [json](${base}/memory.json)`
        : '—';
      return `| \`${r.ossido}\` | \`${r.next}\` | ${tp} | ${mem} |`;
    })
    .join('\n');
  return `${header}\n${body}`;
}

/** Rewrite the table between the BENCH_TABLE markers in README.md. */
export function updateReadmeTable(): void {
  if (!existsSync(README)) return;
  const md = readFileSync(README, 'utf8');
  if (!md.includes(START) || !md.includes(END)) {
    console.warn(`README.md is missing the ${START} / ${END} markers — skipping table update.`);
    return;
  }
  const block = `${START}\n${buildTable()}\n${END}`;
  const next = md.replace(new RegExp(`${START}[\\s\\S]*?${END}`), () => block);
  if (next !== md) writeFileSync(README, next);
}

if (import.meta.main) {
  updateReadmeTable();
  console.log('✔ README results table updated.');
}
