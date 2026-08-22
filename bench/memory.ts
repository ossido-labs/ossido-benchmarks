import os from 'node:os';
import { execSync } from 'node:child_process';
import { existsSync, mkdirSync } from 'node:fs';
import { writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import {
  FRAMEWORKS,
  OSSIDO_BINARY,
  NEXT_DIR,
  resultsDir,
  overriddenEnvVars,
  cpuCount,
  readVersions,
  CONNECTIONS,
  DURATION,
  WARMUP,
  type Framework,
} from './config.ts';
import { startServer } from './servers.ts';
import { loadTest, type LoadResult } from './load.ts';
import { updateReadmeTable } from './index.ts';

// The route used for the memory sweep. /ssr gives steady, high throughput and a
// realistic per-request render+serialize workload (unlike /api which never
// touches V8, or /heavy whose huge buffers spike memory non-representatively).
const MEM_PATH = '/ssr';

interface MemRecord {
  framework: Framework['key'];
  parallelism: number;
  idleRssMb: number;
  meanRssMb: number;
  peakRssMb: number;
  load: LoadResult;
}

/** Sum RSS (MB) of every process in the given process group. */
function groupRssMb(pgid: number): number {
  let kb = 0;
  try {
    const out = execSync('ps -axo pid=,pgid=,rss=', { encoding: 'utf8' });
    for (const line of out.split('\n')) {
      const parts = line.trim().split(/\s+/);
      if (parts.length < 3) continue;
      if (Number(parts[1]) === pgid) kb += Number(parts[2]);
    }
  } catch {
    // ps failed; return whatever we have
  }
  return kb / 1024;
}

const sleep = (ms: number): Promise<void> => new Promise((r) => setTimeout(r, ms));

/** Parallelism levels to sweep: powers of two up to the core count, inclusive. */
function levels(cores: number): number[] {
  const set = new Set<number>([1]);
  for (let n = 2; n <= cores; n *= 2) set.add(n);
  set.add(cores);
  return [...set].sort((a, b) => a - b);
}

function ensureBuilt(): void {
  const binary = OSSIDO_BINARY;
  const nextDir = resolve(NEXT_DIR, '.next');
  if (!existsSync(binary) || !existsSync(nextDir)) {
    console.error('Apps are not built. Run `bun run bench:build` first.');
    process.exit(1);
  }
}

async function measure(framework: Framework, parallelism: number): Promise<MemRecord> {
  const server = await startServer(framework, parallelism);
  try {
    // Let workers finish warming/prepping, then capture idle memory.
    await sleep(1500);
    const idleRssMb = groupRssMb(server.pgid);

    // Sample RSS while the load test runs.
    const samples: number[] = [];
    const timer = setInterval(() => samples.push(groupRssMb(server.pgid)), 250);
    let load: LoadResult;
    try {
      load = await loadTest(`http://127.0.0.1:${framework.port}${MEM_PATH}`);
    } finally {
      clearInterval(timer);
    }

    const peakRssMb = samples.length ? Math.max(...samples) : idleRssMb;
    const meanRssMb = samples.length
      ? samples.reduce((a, b) => a + b, 0) / samples.length
      : idleRssMb;

    return { framework: framework.key, parallelism, idleRssMb, meanRssMb, peakRssMb, load };
  } finally {
    await server.stop();
    await sleep(500); // let ports/pgid fully release before the next server
  }
}

function fmt(n: number, digits = 0): string {
  return n.toLocaleString('en-US', { minimumFractionDigits: digits, maximumFractionDigits: digits });
}

// One bar chart over both frameworks' parallelism sweep. Ossido's levels come
// first, then Next.js's. (Single `bar` series — the mermaid grammar the editor
// validates against is happy with `bar` but rejects paired `line` series.)
function mermaidSweepBar(
  title: string,
  yLabel: string,
  xs: number[],
  ossido: number[],
  next: number[],
): string {
  const round = (v: number): number => Math.round(v * 100) / 100;
  // NB: use ASCII 'x' (not '×') — the mermaid grammar rejects U+00D7 in labels.
  const labels = [
    ...xs.map((x) => `"Oss x${x}"`),
    ...xs.map((x) => `"Next x${x}"`),
  ].join(', ');
  const values = [...ossido, ...next].map(round).join(', ');
  return [
    '```mermaid',
    'xychart-beta',
    `    title "${title}"`,
    `    x-axis [${labels}]`,
    `    y-axis "${yLabel}"`,
    `    bar [${values}]`,
    '```',
    '',
  ].join('\n');
}

async function writeMemoryReport(records: MemRecord[], cores: number, outPath: string): Promise<void> {
  const xs = levels(cores);
  const get = (fw: string, n: number): MemRecord | undefined =>
    records.find((r) => r.framework === fw && r.parallelism === n);
  const effOf = (r: MemRecord): number => r.load.rps / r.meanRssMb;

  const L: string[] = [];
  const p = (s = ''): void => void L.push(s);

  p('# Ossido vs Next.js — memory efficiency');
  p();
  p('> Hypothesis: **Ossido serves far more requests per unit of memory.**');
  p('>');
  p('> Ossido scales SSR across cores with V8 render threads inside a *single*');
  p('> Rust process (one shared heap); Next.js scales by forking *N* full Node.js');
  p('> processes (one heap each). This sweep runs the same `/ssr` load at each');
  p('> parallelism level while sampling the resident memory (RSS) of the whole');
  p('> server process group, and reports **req/s per MB**.');
  p();
  p('## Environment');
  p();
  p('| | |');
  p('| --- | --- |');
  p(`| Date | ${new Date().toISOString()} |`);
  p(`| Host | ${os.type()} ${os.release()} · ${os.arch()} |`);
  p(`| CPU | ${os.cpus()[0]?.model ?? 'unknown'} |`);
  p(`| Logical cores | ${cores} |`);
  p(`| Memory | ${(os.totalmem() / 1024 ** 3).toFixed(1)} GB |`);
  p(`| Load | ${CONNECTIONS} connections, ${DURATION}s (+${WARMUP}s warm-up), route \`${MEM_PATH}\` |`);
  p();

  // ---- Main table ----
  p('## Results');
  p();
  p('| Framework | Parallelism | Idle RSS (MB) | Mean RSS (MB) | Peak RSS (MB) | req/s | **req/s per MB** |');
  p('| --- | ---: | ---: | ---: | ---: | ---: | ---: |');
  for (const fw of ['ossido', 'next'] as const) {
    for (const n of xs) {
      const r = get(fw, n);
      if (!r) continue;
      const name = fw === 'ossido' ? 'Ossido' : 'Next.js';
      p(
        `| ${name} | ${n} | ${fmt(r.idleRssMb)} | ${fmt(r.meanRssMb)} | ${fmt(r.peakRssMb)} | ` +
          `${fmt(r.load.rps)} | **${fmt(effOf(r), 1)}** |`,
      );
    }
  }
  p();

  // ---- Headline ----
  const oMax = get('ossido', cores);
  const nMax = get('next', cores);
  if (oMax && nMax) {
    const oEff = effOf(oMax);
    const nEff = effOf(nMax);
    p('## Headline');
    p();
    p(`At full parallelism (${cores} threads / workers), serving \`/ssr\`:`);
    p();
    p(
      `- **Ossido:** ${fmt(oMax.load.rps)} req/s using ${fmt(oMax.meanRssMb)} MB ` +
        `→ **${fmt(oEff, 1)} req/s per MB**`,
    );
    p(
      `- **Next.js:** ${fmt(nMax.load.rps)} req/s using ${fmt(nMax.meanRssMb)} MB ` +
        `→ **${fmt(nEff, 1)} req/s per MB**`,
    );
    p();
    p(
      `➡️ Ossido is **${(oEff / nEff).toFixed(1)}× more memory-efficient** here, and uses ` +
        `**${(nMax.meanRssMb / oMax.meanRssMb).toFixed(1)}× less RAM** ` +
        `(${fmt(oMax.meanRssMb)} MB vs ${fmt(nMax.meanRssMb)} MB) while serving ` +
        `${(oMax.load.rps / nMax.load.rps).toFixed(2)}× the throughput.`,
    );
    p();
    p(
      `**Baseline footprint:** the gap is starkest at idle. Scaling Ossido from 1 to ` +
        `${cores} threads adds only **${fmt(oMax.idleRssMb - (get('ossido', 1)?.idleRssMb ?? 0))} MB** ` +
        `(shared process, ${fmt(oMax.idleRssMb)} MB total); scaling Next.js to ${cores} ` +
        `workers costs **${fmt(nMax.idleRssMb)} MB** — ` +
        `**${(nMax.idleRssMb / oMax.idleRssMb).toFixed(0)}× more** just to sit idle, ` +
        `because every worker is a full Node.js + Next.js heap.`,
    );
    p();

    // Iso-memory framing: what Next parallelism fits inside Ossido's footprint?
    const budget = oMax.meanRssMb;
    const fits = xs.filter((n) => (get('next', n)?.meanRssMb ?? Infinity) <= budget);
    const bestNextFit = fits.length ? get('next', Math.max(...fits)) : undefined;
    p(
      `**Iso-memory:** within Ossido's ${fmt(budget)} MB footprint, Next.js can only ` +
        (bestNextFit
          ? `run ${bestNextFit.parallelism} worker(s) (${fmt(bestNextFit.meanRssMb)} MB, ${fmt(bestNextFit.load.rps)} req/s) — ` +
            `Ossido serves ${(oMax.load.rps / bestNextFit.load.rps).toFixed(1)}× the requests in the same RAM.`
          : 'not even fit a single worker at these levels.'),
    );
    p();
  }

  // ---- Charts ----
  p('## Charts');
  p();
  p('*Bars are the parallelism sweep: `Oss xn` = Ossido with n threads, `Next xn` =');
  p('Next.js with n workers.*');
  p();

  p('### Throughput vs parallelism (req/s — higher is better)');
  p();
  p(
    mermaidSweepBar(
      'Throughput — req/s',
      'req/s',
      xs,
      xs.map((n) => get('ossido', n)?.load.rps ?? 0),
      xs.map((n) => get('next', n)?.load.rps ?? 0),
    ),
  );

  p('### Memory vs parallelism (mean RSS, MB — lower is better)');
  p();
  p(
    mermaidSweepBar(
      'Memory — mean RSS (MB)',
      'MB',
      xs,
      xs.map((n) => get('ossido', n)?.meanRssMb ?? 0),
      xs.map((n) => get('next', n)?.meanRssMb ?? 0),
    ),
  );

  p('### Efficiency vs parallelism (req/s per MB — higher is better)');
  p();
  p('This is the direct test of the hypothesis.');
  p();
  p(
    mermaidSweepBar(
      'Efficiency — req/s per MB',
      'req/s per MB',
      xs,
      xs.map((n) => {
        const r = get('ossido', n);
        return r ? effOf(r) : 0;
      }),
      xs.map((n) => {
        const r = get('next', n);
        return r ? effOf(r) : 0;
      }),
    ),
  );

  p('---');
  p();
  p('### How this is measured');
  p();
  p('- **Memory:** RSS of the entire server process group (Ossido: one process');
  p('  with N render threads; Next.js: the cluster primary + N workers), summed');
  p('  from `ps` and sampled every 250ms during the load. *Idle* is captured just');
  p('  before load; *mean*/*peak* are over the load window.');
  p('- **Throughput:** [autocannon](https://github.com/mcollina/autocannon) against');
  p(`  \`${MEM_PATH}\` (${CONNECTIONS} connections, ${DURATION}s, after warm-up).`);
  p('- **Parallelism** = `OSSIDO_SSR_THREADS` for Ossido, Node `cluster` worker');
  p('  count (`WEB_CONCURRENCY`) for Next.js.');
  p('- Both are production builds rendering the identical component tree.');
  p();

  await writeFile(outPath, L.join('\n'));
}

/**
 * Machine-readable memory-efficiency results for the docs site. Stable,
 * self-describing shape (bump `schema` on any breaking change).
 */
async function writeMemoryJson(records: MemRecord[], cores: number, outPath: string): Promise<void> {
  const json = {
    schema: 'ossido-benchmark/memory@1',
    generatedAt: new Date().toISOString(),
    environment: {
      host: `${os.type()} ${os.release()} · ${os.arch()}`,
      cpu: os.cpus()[0]?.model ?? 'unknown',
      cores,
      totalMemGb: Number((os.totalmem() / 1024 ** 3).toFixed(1)),
    },
    versions: readVersions(),
    load: {
      connections: CONNECTIONS,
      durationSec: DURATION,
      warmupSec: WARMUP,
      route: MEM_PATH,
    },
    levels: levels(cores),
    results: records.map((r) => ({
      framework: r.framework,
      parallelism: r.parallelism,
      idleRssMb: Number(r.idleRssMb.toFixed(1)),
      meanRssMb: Number(r.meanRssMb.toFixed(1)),
      peakRssMb: Number(r.peakRssMb.toFixed(1)),
      rps: r.load.rps,
      reqPerMb: Number((r.load.rps / r.meanRssMb).toFixed(2)),
    })),
  };
  await writeFile(outPath, JSON.stringify(json, null, 2) + '\n');
}

async function main(): Promise<void> {
  ensureBuilt();
  const cores = cpuCount();
  const xs = levels(cores);

  console.log('\n=== Memory efficiency: Ossido vs Next.js ===');
  console.log(`host: ${cores} cores · sweeping parallelism ${xs.join(', ')} · route ${MEM_PATH}\n`);

  const records: MemRecord[] = [];
  for (const framework of FRAMEWORKS) {
    for (const n of xs) {
      process.stdout.write(`▶ ${framework.label.padEnd(8)} ×${String(n).padEnd(2)} `);
      const rec = await measure(framework, n);
      records.push(rec);
      console.log(
        `${fmt(rec.load.rps).padStart(7)} req/s · ` +
          `${fmt(rec.meanRssMb).padStart(5)} MB · ` +
          `${fmt(rec.load.rps / rec.meanRssMb, 1).padStart(6)} req/s per MB`,
      );
    }
  }

  const overrides = overriddenEnvVars();
  if (overrides.length) {
    console.log(
      `\n⚠ Non-default load (${overrides.join(', ')} set) — skipping output. ` +
        'Results above are for inspection only; run with defaults to update ' +
        'results/<version>/ and the README table.',
    );
    return;
  }

  const dir = resultsDir();
  mkdirSync(dir, { recursive: true });
  const mdPath = resolve(dir, 'MEMORY.md');
  const jsonPath = resolve(dir, 'memory.json');
  await writeMemoryReport(records, cores, mdPath);
  await writeMemoryJson(records, cores, jsonPath);
  updateReadmeTable();
  console.log(`\n✔ Reports written to ${mdPath} and ${jsonPath}`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
