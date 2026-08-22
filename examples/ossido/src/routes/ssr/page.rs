use ossido::{Props, Request, handler};

// A server-side props handler makes this route `ƒ` (server-rendered per
// request) instead of `○` (statically prerendered at build). This matches the
// Next.js `export const dynamic = 'force-dynamic'` so both frameworks render on
// every request. The catalogue data itself is generated in the React component
// (identical to Next), so this handler stays trivial on purpose.
#[Props]
struct SsrProps {
    dynamic: bool,
}

#[handler]
async fn get_server_side_props(_req: Request) -> SsrProps {
    SsrProps { dynamic: true }
}
