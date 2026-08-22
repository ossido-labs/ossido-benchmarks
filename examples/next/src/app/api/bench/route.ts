import { makeProducts } from "../../../bench/data";

// /api/bench — JSON API throughput benchmark. Serialises a deterministic list
// of products in the Node.js runtime. The Ossido equivalent (Rust/axum) lives
// in examples/ossido/src/routes/api/bench.rs.
export const dynamic = "force-dynamic";

export function GET(): Response {
  return Response.json(makeProducts(100));
}
