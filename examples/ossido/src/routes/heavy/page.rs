use ossido::{Props, Request, handler};

// See src/routes/ssr/page.rs — a trivial props handler forces per-request SSR
// (`ƒ`) so the heavy table is rendered on every request, matching Next.
#[Props]
struct HeavyProps {
    dynamic: bool,
}

#[handler]
async fn get_server_side_props(_req: Request) -> HeavyProps {
    HeavyProps { dynamic: true }
}
