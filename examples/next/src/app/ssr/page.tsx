import type { JSX } from "react";
import { Catalog } from "../../bench/Catalog";

// /ssr — moderate server-rendered page (60 product cards).
// force-dynamic ensures the page is rendered on every request (no static
// prerender / full-route cache), matching Ossido's per-request SSR.
export const dynamic = "force-dynamic";

export default function SsrPage(): JSX.Element {
  return <Catalog count={60} seed={1} />;
}
