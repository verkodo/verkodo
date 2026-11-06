// SPDX-License-Identifier: AGPL-3.0-or-later
//! The web editor of verkodo, built in: the page, its script and style and verkodo as WebAssembly, served from the
//! binary with a Content-Security-Policy that allows nothing from elsewhere. Around a router – the API's in `verkodo
//! serve` –, it answers every path that router does not know.

use axum::{
    Router,
    http::header,
    response::{Html, IntoResponse, Response},
};

/// `router` with the web editor around it: every path it does not know is a file of the editor or the editor itself.
pub fn around<S: Clone + Send + Sync + 'static>(router: Router<S>) -> Router<S> {
    router.fallback(page)
}

/// The web editor, built by `pnpm build` in `web/` into this crate (`web/` here) and embedded; a build without it
/// says so.
#[derive(rust_embed::Embed)]
#[folder = "web"]
#[allow_missing = true]
struct WebApp;

/// What the web app may load: itself only – no script, style, font or connection elsewhere, no frames around it.
/// Two openings, both for the editor: `'wasm-unsafe-eval'` lets it compile verkodo's WebAssembly module (not `eval`
/// for JavaScript), and a nonce, new with every page, lets the editor add its style elements and the preview its
/// inline style and script – never inline code that does not carry it.
fn policy(nonce: &str) -> String {
    format!(
        "default-src 'self'; script-src 'self' 'wasm-unsafe-eval' 'nonce-{nonce}'; style-src 'self' 'nonce-{nonce}'; \
img-src 'self' data:; font-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; \
form-action 'self'"
    )
}

/// What the page holds where the server puts the nonce (`web/index.html`).
const NONCE_SLOT: &str = r#"content="verkodo-nonce""#;

/// 16 random bytes as hex: a nonce for one page.
fn nonce() -> String {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).expect("randomness from the operating system");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The page shown where the web app is not built into the binary.
const NO_WEB_APP: &str = "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><title>verkodo</title>\
<body style=\"font-family:system-ui;max-width:40rem;margin:4rem auto;padding:0 1rem\"><h1>verkodo</h1>\
<p>This build has no web editor – run <code>pnpm wasm</code> and <code>pnpm build</code> in <code>web/</code> and \
build again. The API, if it runs, is at <code>/api/v1</code>.</p>";

/// A file of the web app; any other path is the app itself, which routes by the address.
async fn page(uri: axum::http::Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let (file, path) = match WebApp::get(path) {
        Some(file) if !path.is_empty() => (file, path),
        _ => match WebApp::get("index.html") {
            Some(file) => (file, "index.html"),
            None => return Html(NO_WEB_APP).into_response(),
        },
    };
    // Built files carry their hash in their name and never change; the page itself is asked for every time.
    let cache = if path.starts_with("assets/") { "public, max-age=31536000, immutable" } else { "no-cache" };
    let nonce = nonce();
    let data = if path == "index.html" {
        let page = String::from_utf8_lossy(&file.data).replace(NONCE_SLOT, &format!(r#"content="{nonce}""#));
        page.into_bytes().into()
    } else {
        file.data
    };
    (
        [
            (header::CONTENT_TYPE, file.metadata.mimetype().to_owned()),
            (header::CACHE_CONTROL, cache.to_owned()),
            (header::CONTENT_SECURITY_POLICY, policy(&nonce)),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_owned()),
            (header::REFERRER_POLICY, "no-referrer".to_owned()),
        ],
        data,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[tokio::test]
    async fn every_page_gets_a_new_nonce_in_page_and_policy() {
        let app = around(Router::new());
        let get = |path: &str| {
            let app = app.clone();
            let request = axum::http::Request::builder().uri(path).body(Body::empty()).unwrap();
            async move { app.oneshot(request).await.unwrap() }
        };
        let first = get("/").await;
        let policy = first.headers()[header::CONTENT_SECURITY_POLICY].to_str().unwrap().to_string();
        assert!(policy.contains("'wasm-unsafe-eval'") && policy.contains("frame-ancestors 'none'"), "{policy}");
        let nonce = policy.split("'nonce-").nth(1).unwrap().split('\'').next().unwrap().to_string();
        let body = String::from_utf8(first.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
        // With the editor built in, the page carries the policy's nonce; without it, the page says how to build it.
        assert!(body.contains(&format!(r#"content="{nonce}""#)) || body.contains("no web editor"), "{body}");
        let second = get("/somewhere").await;
        let other = second.headers()[header::CONTENT_SECURITY_POLICY].to_str().unwrap().to_string();
        assert_ne!(policy, other, "a new nonce for every page");
    }
}
