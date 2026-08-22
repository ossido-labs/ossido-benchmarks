import { Suspense, type JSX } from "react";
import { HeavyTable } from "../../bench/HeavyTable";

// /stream — streaming SSR: the shell is flushed first, then a Suspense boundary
// streams in the expensive part (a large table) once rendered. Next renders
// this with a native async Server Component; the observable behaviour (shell
// first, then the heavy section) mirrors the Ossido /stream route.
//
// The heavy work is gated behind a microtask (`await Promise.resolve()`) rather
// than a timer, so this measures real render/stream time on both runtimes.
export const dynamic = "force-dynamic";

const STREAM_ROWS = 3000;

async function SlowSection(): Promise<JSX.Element> {
  await Promise.resolve();
  return <HeavyTable rows={STREAM_ROWS} seed={3} />;
}

export default function StreamPage(): JSX.Element {
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
        <SlowSection />
      </Suspense>
    </div>
  );
}
