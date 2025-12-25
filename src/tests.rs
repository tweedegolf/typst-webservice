use std::{io::Read, sync::Arc};

use axum::{
    Router,
    body::{self, Body},
    http::{Request, StatusCode},
};
use tower::util::ServiceExt;
use utoipa::OpenApi;
use utoipa_axum::{router::OpenApiRouter, routes};
use utoipa_swagger_ui::SwaggerUi;

use crate::{handlers, logging, pdf::PdfContext};

/// Construct an Axum router wired with the application's routes for testing.
fn build_router() -> Router {
    let context = Arc::new(PdfContext::from_directory("./assets").unwrap());
    let (router, api) = OpenApiRouter::with_openapi(crate::ApiDoc::openapi())
        .routes(routes!(handlers::render_pdf, handlers::render_pdf_batch))
        .with_state(context)
        .split_for_parts();

    router.merge(SwaggerUi::new("/").url("/apidoc/openapi.json", api))
}

#[tokio::test]
/// Verify that requesting a known template returns a PDF payload.
async fn render_pdf_success() {
    logging::init_for_tests();
    let router = build_router();

    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/render-pdf/example.typ/output.pdf")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"World","list":["Test"]}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let headers = response.headers();
    assert_eq!(
        headers.get(axum::http::header::CONTENT_TYPE).unwrap(),
        "application/pdf"
    );
    assert!(
        headers
            .get(axum::http::header::CONTENT_DISPOSITION)
            .unwrap()
            .to_str()
            .unwrap()
            .contains("output.pdf")
    );

    let body = body::to_bytes(response.into_body(), 10 * 1024 * 1024)
        .await
        .unwrap();
    assert!(!body.is_empty(), "expected PDF body to be non-empty");
}

#[tokio::test]
/// Ensure the batch endpoint renders a ZIP archive containing multiple PDFs.
async fn render_pdf_batch_success() {
    logging::init_for_tests();
    let router = build_router();

    let payload = serde_json::json!([
        {
            "template": "example.typ",
            "file_name": "first.pdf",
            "input": { "name": "Batch One", "list": ["Item"] }
        },
        {
            "template": "example.typ",
            "file_name": "second.pdf",
            "input": { "name": "Batch Two", "list": ["Item"] }
        }
    ]);

    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/render-pdf/batch")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let headers = response.headers();
    assert_eq!(
        headers.get(axum::http::header::CONTENT_TYPE).unwrap(),
        "application/zip"
    );
    assert!(
        headers
            .get(axum::http::header::CONTENT_DISPOSITION)
            .unwrap()
            .to_str()
            .unwrap()
            .contains(".zip")
    );

    let bytes = body::to_bytes(response.into_body(), 10 * 1024 * 1024)
        .await
        .unwrap();
    assert!(!bytes.is_empty(), "expected ZIP body to be non-empty");

    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    assert_eq!(archive.len(), 2);

    for name in ["first.pdf", "second.pdf"] {
        let mut file = archive.by_name(name).unwrap();
        let mut content = Vec::new();
        file.read_to_end(&mut content).unwrap();
        assert!(!content.is_empty(), "expected {name} to contain PDF data");
    }
}

#[tokio::test]
/// Confirm the batch endpoint propagates a 404 when any template is missing.
async fn render_pdf_batch_missing_template() {
    logging::init_for_tests();
    let router = build_router();

    let payload = serde_json::json!([{
        "template": "unknown.typ",
        "file_name": "missing.pdf",
        "input": { "name": "World" }
    }]);

    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/render-pdf/batch")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let bytes = body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json.get("error").unwrap(), "Requested template not found");
    assert!(json.get("reference").is_some());
}

#[tokio::test]
/// Confirm single render requests return 404 for missing templates.
async fn render_pdf_missing_template() {
    logging::init_for_tests();
    let router = build_router();

    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/render-pdf/unknown.typ/output.pdf")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"World"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let bytes = body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json.get("error").unwrap(), "Requested template not found");
    assert!(json.get("reference").is_some());
}

#[tokio::test]
/// Ensure malformed JSON payloads produce a 400 Bad Request response.
async fn render_pdf_invalid_json() {
    logging::init_for_tests();
    let router = build_router();

    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/render-pdf/example.typ/output.pdf")
                .header("content-type", "application/json")
                .body(Body::from("{"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let bytes = body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    assert!(!bytes.is_empty());
}

#[tokio::test]
/// Ensure incorrectly structured JSON payloads produce a 400 Bad Request response.
async fn render_pdf_invalid_json_structure() {
    logging::init_for_tests();
    let router = build_router();

    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/render-pdf/example.typ/output.pdf")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"world":"Name","list":["Item"]}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body_bytes = body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(body.get("error").unwrap(), "Document compilation failed");
}
