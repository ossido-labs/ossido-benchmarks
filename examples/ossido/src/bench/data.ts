// Deterministic data generation shared by every benchmark route.
//
// The SAME source lives in examples/next/src/bench/data.ts. Keeping the two
// byte-for-byte identical is what makes the SSR comparison fair: both
// frameworks render the exact same React tree from the exact same data, so any
// difference in throughput/latency comes from the runtime, not the workload.

export interface Product {
  id: number;
  name: string;
  category: string;
  price: number;
  qty: number;
  score: number;
  sku: string;
}

const CATEGORIES = [
  'Alpha',
  'Beta',
  'Gamma',
  'Delta',
  'Epsilon',
  'Zeta',
  'Eta',
  'Theta',
] as const;

// A tiny deterministic PRNG (mulberry32) so both apps generate identical data
// from the same seed without pulling in a dependency.
function mulberry32(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a |= 0;
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export function makeProducts(count: number, seed = 12345): Product[] {
  const rand = mulberry32(seed);
  const products: Product[] = new Array(count);
  for (let i = 0; i < count; i++) {
    const category = CATEGORIES[i % CATEGORIES.length];
    products[i] = {
      id: i,
      name: `${category} Widget #${i}`,
      category,
      price: Math.round(rand() * 100000) / 100,
      qty: 1 + Math.floor(rand() * 500),
      score: Math.floor(rand() * 1000),
      sku: `SKU-${(i * 2654435761) % 1000000}`,
    };
  }
  return products;
}
