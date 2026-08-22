// Clustered production server for the Next.js benchmark app.
//
// Node runs JavaScript on a single thread, so a lone `next start` process can
// only ever use one CPU core for SSR. The idiomatic way to use more cores is
// the Node `cluster` module: the primary forks N workers that all accept
// connections on the SAME port, and the OS/primary distributes them. This is
// the Next.js counterpart to Ossido's multi-threaded V8 render pool.
//
//   WEB_CONCURRENCY=1        -> single process (single-threaded baseline)
//   WEB_CONCURRENCY=<cores>  -> one worker per core (multi-core)
//
// Uses Next's documented programmatic API (next({ dev:false }) +
// getRequestHandler), so it runs the exact production build in `.next`.

import cluster from "node:cluster";
import { createServer } from "node:http";
import next from "next";

const port = parseInt(process.env.PORT || "4100", 10);
const workers = Math.max(1, parseInt(process.env.WEB_CONCURRENCY || "1", 10));

if (workers > 1 && cluster.isPrimary) {
  for (let i = 0; i < workers; i++) cluster.fork();
  cluster.on("exit", (worker) => {
    // Keep the pool full if a worker dies mid-benchmark.
    if (!worker.exitedAfterDisconnect) cluster.fork();
  });
} else {
  const app = next({ dev: false, dir: process.cwd() });
  const handle = app.getRequestHandler();
  await app.prepare();
  createServer((req, res) => handle(req, res)).listen(port, () => {
    // The harness waits on an HTTP probe, but log for visibility.
    const role = workers > 1 ? `worker ${process.pid}` : "server";
    console.log(`next ${role} listening on http://127.0.0.1:${port}`);
  });
}
