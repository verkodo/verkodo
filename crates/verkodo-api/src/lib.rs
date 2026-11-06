// SPDX-License-Identifier: AGPL-3.0-or-later
//! The open API of verkodo: HTTP and JSON under `/api/v1`, described in OpenAPI (`api/openapi.json`) – Lean for the
//! web editor, which `verkodo-web` serves around it. Like every interface, it works through `verkodo-sdk` only. It listens on the device itself; OPEN: authentication and TLS before it listens anywhere else.

use std::{
    net::SocketAddr,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};

use verkodo_sdk::knowledge::{self, ReportData, Severity};

/// The description of the API in OpenAPI.
#[derive(Debug, OpenApi)]
#[openapi(
    info(
        title = "verkodo",
        description = "The open API of verkodo: truth code – mathematics and law written formally, checked by machine."
    ),
    paths(lean_info, prove),
    components(schemas(Problem, LeanInfo, ProveRequest, SourceText, ProveResult, DiagnosticView))
)]
pub struct ApiDoc;

/// The description of the API in OpenAPI, as pretty JSON – what `api/openapi.json` holds.
pub fn openapi() -> String {
    ApiDoc::openapi().to_pretty_json().expect("the description in JSON")
}

/// An error: what was refused or went wrong.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Problem {
    /// What happened, in English.
    pub error: String,
}

/// An answer that went wrong: a status and what to say.
#[derive(Debug)]
pub struct ApiError(StatusCode, String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(Problem { error: self.1 })).into_response()
    }
}

/// The state of the server: Lean, if it may run.
#[derive(Debug)]
pub struct Server {
    /// The Lean project that `prove` uses; none: `prove` is switched off.
    lean: Option<PathBuf>,
    /// Whether Lean is running – one check at a time.
    proving: AtomicBool,
}

type Shared = Arc<Server>;

/// Whether this server lets Lean check (`verkodo serve --lean <dir>`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LeanInfo {
    pub available: bool,
}

/// A source file to check, as an editor holds it.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SourceText {
    pub path: String,
    pub text: String,
}

/// Files to let Lean check – all files that belong together, in the order the editor keeps them.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProveRequest {
    pub files: Vec<SourceText>,
}

/// A message about a place in a file; `start` and `end` are byte offsets in its text.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DiagnosticView {
    pub file: usize,
    pub start: usize,
    pub end: usize,
    /// `error` or `warning`.
    pub severity: String,
    pub message: String,
}

/// What Lean said.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProveResult {
    pub checked: usize,
    pub open: usize,
    pub rejected: usize,
    /// Diagnostics of verkodo and of Lean.
    pub diagnostics: Vec<DiagnosticView>,
    /// Lean's verdict on every part and the names it resolved – for verkodo in the browser (`ReportData`); its
    /// numbers belong to these files, analysed by this version of verkodo.
    #[schema(value_type = Object)]
    pub report: serde_json::Value,
}

/// At most this many bytes of sources per check.
const PROVE_LIMIT: usize = 1 << 20;

/// Whether `prove` is switched on.
#[utoipa::path(get, path = "/api/v1/lean", responses((status = 200, body = LeanInfo)))]
async fn lean_info(State(server): State<Shared>) -> Json<LeanInfo> {
    Json(LeanInfo { available: server.lean.is_some() })
}

/// Lets Lean check the files. One check at a time; it can take a minute or more.
#[utoipa::path(post, path = "/api/v1/prove", request_body = ProveRequest,
    responses((status = 200, body = ProveResult), (status = 413, body = Problem),
        (status = 501, body = Problem, description = "Lean is switched off on this server"),
        (status = 503, body = Problem, description = "Lean is busy with another check")))]
async fn prove(State(server): State<Shared>, Json(request): Json<ProveRequest>) -> Result<Json<ProveResult>, ApiError> {
    let Some(lean_dir) = server.lean.clone() else {
        return Err(ApiError(StatusCode::NOT_IMPLEMENTED, "Lean is switched off (verkodo serve --lean <dir>)".into()));
    };
    if request.files.iter().map(|f| f.text.len() + f.path.len()).sum::<usize>() > PROVE_LIMIT {
        return Err(ApiError(StatusCode::PAYLOAD_TOO_LARGE, format!("at most {PROVE_LIMIT} bytes per check")));
    }
    if server.proving.swap(true, Ordering::AcqRel) {
        return Err(ApiError(StatusCode::SERVICE_UNAVAILABLE, "Lean is busy with another check".into()));
    }
    let texts = request.files.into_iter().map(|f| (f.path, f.text)).collect();
    let state = server.clone();
    let result = tokio::task::spawn_blocking(move || {
        let outcome = knowledge::prove_texts(texts, &lean_dir);
        state.proving.store(false, Ordering::Release);
        outcome
    })
    .await
    .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let (sources, outcome) = result.map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let analysis = knowledge::analyze(&sources);
    let diagnostics = sources
        .diags
        .iter()
        .chain(&analysis.diags)
        .chain(&outcome.diags)
        .map(|d| DiagnosticView {
            file: d.file,
            start: d.span.start,
            end: d.span.end,
            severity: if d.severity == Severity::Error { "error" } else { "warning" }.into(),
            message: d.message.clone(),
        })
        .collect();
    let (checked, open, rejected) = outcome.report.counts();
    let report = serde_json::to_value(ReportData::from(&outcome.report))
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(ProveResult { checked, open, rejected, diagnostics, report }))
}

async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

/// The router: the API under `/api/v1` (the web editor around it comes from `verkodo-web`). With `lean`, `prove`
/// lets Lean check in that project – Lean runs the code it is given, so only where whoever can reach the server may
/// run code here.
pub fn router(lean: Option<PathBuf>) -> Router {
    let api = Router::new()
        .route("/openapi.json", get(openapi_json))
        .route("/lean", get(lean_info))
        .route("/prove", post(prove))
        .fallback(|| async { ApiError(StatusCode::NOT_FOUND, "no such route of the API".into()) });
    let server = Server { lean, proving: AtomicBool::new(false) };
    Router::new().nest("/api/v1", api).with_state(Arc::new(server))
}

/// Serves `router` – the API's, perhaps with the web editor around it – on the device itself, at `port`, until
/// interrupted; `ready` is told the address.
pub async fn serve(router: Router, port: u16, ready: impl FnOnce(SocketAddr)) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], port))).await?;
    ready(listener.local_addr()?);
    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
}

#[cfg(test)]
mod tests;
