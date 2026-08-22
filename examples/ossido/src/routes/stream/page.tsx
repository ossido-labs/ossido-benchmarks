import type { JSX } from 'react';
import { StreamPage } from '../../bench/Stream.tsx';

// /stream — streaming SSR: shell first, catalogue streams in after a delay.
export default function Stream(): JSX.Element {
  return <StreamPage />;
}
