import type { JSX } from "react";
import { HeavyTable } from "../../bench/HeavyTable";

// /heavy — large CPU-bound render (5000-row table).
export const dynamic = "force-dynamic";

export default function HeavyPage(): JSX.Element {
  return <HeavyTable rows={5000} seed={7} />;
}
