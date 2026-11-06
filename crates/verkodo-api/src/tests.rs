// SPDX-License-Identifier: AGPL-3.0-or-later
use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use tower::ServiceExt;

use super::*;

async fn call(router: &Router, method: &str, uri: &str, body: Option<&str>) -> (StatusCode, String) {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(body.map(|b| Body::from(b.to_owned())).unwrap_or_default())
        .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

#[tokio::test]
async fn unknown_routes_are_not_found() {
    let router = router(None);
    assert_eq!(call(&router, "GET", "/api/v1/nothing", None).await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn prove_is_off_without_lean() {
    let router = router(None);
    assert_eq!(call(&router, "GET", "/api/v1/lean", None).await.1, r#"{"available":false}"#);
    let body = r#"{"files":[{"path":"m.vk","text":"module m"}]}"#;
    assert_eq!(call(&router, "POST", "/api/v1/prove", Some(body)).await.0, StatusCode::NOT_IMPLEMENTED);
    // Too much for one check is refused before Lean.
    let router = router_with_lean();
    let big = format!(r#"{{"files":[{{"path":"m.vk","text":"{}"}}]}}"#, "x".repeat(PROVE_LIMIT + 1));
    assert_eq!(call(&router, "POST", "/api/v1/prove", Some(&big)).await.0, StatusCode::PAYLOAD_TOO_LARGE);
}

fn router_with_lean() -> Router {
    router(Some(PathBuf::from("lean-that-is-not-there")))
}

#[test]
fn the_description_names_every_route() {
    let description = openapi();
    for path in ["/api/v1/lean", "/api/v1/prove"] {
        assert!(description.contains(path), "{path}");
    }
}
