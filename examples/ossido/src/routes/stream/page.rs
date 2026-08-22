use ossido::{Props, Request, handler};

// See src/routes/ssr/page.rs — a trivial props handler forces per-request SSR
// (`ƒ`). Streaming only makes sense per request, so this route must not be
// statically prerendered.
#[Props]
struct StreamProps {
    dynamic: bool,
}

#[handler]
async fn get_server_side_props(_req: Request) -> StreamProps {
    StreamProps { dynamic: true }
}
