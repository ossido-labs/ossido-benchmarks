import { Suspense, use, useState, type JSX } from 'react';
import { HeavyTable } from './HeavyTable.tsx';

// Streaming SSR workload. The shell (heading) is flushed to the client
// immediately, then a Suspense boundary streams in the expensive part (a large
// table) once it has rendered. This lets the benchmark measure time-to-first-
// byte (shell) separately from time-to-full-response, which is the whole point
// of streaming: the browser can start painting the shell while the server is
// still producing the rest.
//
// The heavy section is gated behind a microtask-resolved promise rather than a
// timer: React flushes the shell first, then renders and streams the table on
// the next tick. Using real CPU work (not an artificial timer delay) keeps the
// comparison against Next.js fair — both runtimes actually do the rendering
// work, and neither benefits from a collapsed/ignored timer.

const STREAM_ROWS = 3000;

function SlowSection({ resource }: { resource: Promise<void> }): JSX.Element {
  use(resource);
  return <HeavyTable rows={STREAM_ROWS} seed={3} />;
}

export function StreamPage(): JSX.Element {
  // Stable promise (created once) so React reuses it across Suspense retries.
  const [resource] = useState(() => Promise.resolve());
  return (
    <div className="flex w-full flex-col items-center gap-6">
      <header className="text-center">
        <h1 className="text-2xl font-bold text-white">Streaming SSR</h1>
        <p className="mt-1 text-sm text-white/60">
          Shell is sent first; the {STREAM_ROWS}-row table streams in once
          rendered.
        </p>
      </header>
      <Suspense
        fallback={<p className="text-sm text-white/50">Loading report…</p>}
      >
        <SlowSection resource={resource} />
      </Suspense>
    </div>
  );
}
