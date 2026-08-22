import type { JSX } from 'react';
import { HeavyTable } from '../../bench/HeavyTable.tsx';

// /heavy — large CPU-bound render (5000-row table).
export default function HeavyPage(): JSX.Element {
  return <HeavyTable rows={5000} seed={7} />;
}
