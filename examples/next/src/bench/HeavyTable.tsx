import type { JSX } from "react";
import { makeProducts, type Product } from "./data";

// Heavy, CPU-bound SSR workload: a large table where every cell is computed
// and formatted. This is the scenario that most stresses the SSR render path,
// so it is where a multi-threaded render pool pays off the most. Identical to
// examples/ossido/src/bench/HeavyTable.tsx.

function money(value: number): string {
  return `$${value.toFixed(2)}`;
}

function Row({ product }: { product: Product }): JSX.Element {
  const margin = product.price * (0.1 + (product.score % 40) / 100);
  const value = product.price * product.qty;
  return (
    <tr className="border-b border-white/5">
      <td className="px-3 py-1.5 font-mono text-xs text-white/50">{product.id}</td>
      <td className="px-3 py-1.5 text-white">{product.name}</td>
      <td className="px-3 py-1.5 text-white/70">{product.category}</td>
      <td className="px-3 py-1.5 text-right text-white">{money(product.price)}</td>
      <td className="px-3 py-1.5 text-right text-white/70">{product.qty}</td>
      <td className="px-3 py-1.5 text-right text-white/70">{money(margin)}</td>
      <td className="px-3 py-1.5 text-right text-white">{money(value)}</td>
      <td className="px-3 py-1.5 text-right">
        <span
          className={
            product.score > 500
              ? "rounded bg-emerald-500/20 px-2 py-0.5 text-xs text-emerald-300"
              : "rounded bg-amber-500/20 px-2 py-0.5 text-xs text-amber-300"
          }
        >
          {product.score}
        </span>
      </td>
    </tr>
  );
}

export function HeavyTable({ rows = 5000, seed = 7 }: { rows?: number; seed?: number }): JSX.Element {
  const products = makeProducts(rows, seed);
  return (
    <section className="w-full max-w-6xl">
      <h1 className="text-2xl font-bold text-white">Inventory report</h1>
      <p className="mt-1 text-sm text-white/60">{rows} rows rendered server-side</p>
      <table className="mt-6 w-full border-collapse text-sm">
        <thead>
          <tr className="border-b border-white/20 text-left text-white/60">
            <th className="px-3 py-2 font-medium">#</th>
            <th className="px-3 py-2 font-medium">Name</th>
            <th className="px-3 py-2 font-medium">Category</th>
            <th className="px-3 py-2 text-right font-medium">Price</th>
            <th className="px-3 py-2 text-right font-medium">Qty</th>
            <th className="px-3 py-2 text-right font-medium">Margin</th>
            <th className="px-3 py-2 text-right font-medium">Value</th>
            <th className="px-3 py-2 text-right font-medium">Score</th>
          </tr>
        </thead>
        <tbody>
          {products.map((product) => (
            <Row key={product.id} product={product} />
          ))}
        </tbody>
      </table>
    </section>
  );
}
