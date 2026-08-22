use ossido::axum::Json;
use ossido::{Request, Type, api};

// /api/bench — JSON API throughput benchmark. Serialises a deterministic list
// of products entirely on the Rust/axum side (no V8 involved), so this measures
// the backend request path rather than SSR. The Next equivalent lives in
// examples/next/src/app/api/bench/route.ts.

#[Type]
pub struct Product {
    id: u32,
    name: String,
    category: String,
    price: f64,
    qty: u32,
    score: u32,
    sku: String,
}

const CATEGORIES: [&str; 8] = [
    "Alpha", "Beta", "Gamma", "Delta", "Epsilon", "Zeta", "Eta", "Theta",
];

fn make_products(count: u32) -> Vec<Product> {
    let mut products = Vec::with_capacity(count as usize);
    let mut a: u32 = 12345;
    let mut rand = || {
        a = a.wrapping_add(0x6d2b79f5);
        let mut t = (a ^ (a >> 15)).wrapping_mul(1 | a);
        t = t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t)) ^ t;
        ((t ^ (t >> 14)) as f64) / 4294967296.0
    };
    for i in 0..count {
        let category = CATEGORIES[(i as usize) % CATEGORIES.len()];
        products.push(Product {
            id: i,
            name: format!("{category} Widget #{i}"),
            category: category.to_string(),
            price: (rand() * 100000.0).round() / 100.0,
            qty: 1 + (rand() * 500.0) as u32,
            score: (rand() * 1000.0) as u32,
            sku: format!("SKU-{}", (i as u64).wrapping_mul(2654435761) % 1000000),
        });
    }
    products
}

#[api(GET)]
pub async fn get_bench(_req: Request) -> Json<Vec<Product>> {
    Json(make_products(100))
}
