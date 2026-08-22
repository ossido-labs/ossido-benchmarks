import type { JSX } from 'react';
import { Catalog } from '../../bench/Catalog.tsx';

// /ssr — moderate server-rendered page (60 product cards).
export default function SsrPage(): JSX.Element {
  return <Catalog count={60} seed={1} />;
}
