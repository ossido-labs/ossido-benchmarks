import type { JSX } from 'react';
import { makeProducts, type Product } from './data.ts';

// Moderate SSR workload: a catalogue of product cards. Identical to the Next
// version in examples/next/src/bench/Catalog.tsx.

function money(value: number): string {
  return `$${value.toFixed(2)}`;
}

function Card({ product }: { product: Product }): JSX.Element {
  return (
    <article className="rounded-lg border border-white/10 bg-white/[0.02] p-4">
      <header className="flex items-center justify-between">
        <h3 className="text-base font-semibold text-white">{product.name}</h3>
        <span className="rounded-full bg-white/10 px-2 py-0.5 text-xs text-white/70">
          {product.category}
        </span>
      </header>
      <dl className="mt-3 grid grid-cols-2 gap-2 text-sm text-white/70">
        <dt>Price</dt>
        <dd className="text-right text-white">{money(product.price)}</dd>
        <dt>In stock</dt>
        <dd className="text-right text-white">{product.qty}</dd>
        <dt>Score</dt>
        <dd className="text-right text-white">{product.score}</dd>
        <dt>SKU</dt>
        <dd className="text-right font-mono text-xs text-white/60">{product.sku}</dd>
      </dl>
    </article>
  );
}

export function Catalog({ count = 60, seed = 1 }: { count?: number; seed?: number }): JSX.Element {
  const products = makeProducts(count, seed);
  const total = products.reduce((sum, p) => sum + p.price * p.qty, 0);
  return (
    <section className="w-full max-w-5xl">
      <h1 className="text-2xl font-bold text-white">Product catalogue</h1>
      <p className="mt-1 text-sm text-white/60">
        {count} products · inventory value {money(total)}
      </p>
      <div className="mt-6 grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {products.map((product) => (
          <Card key={product.id} product={product} />
        ))}
      </div>
    </section>
  );
}
