//! Rust API(Axum)for the form PoC.
//! GET /api/departments        → 部門清單
//! GET /api/members/{dept}     → 該部門人員列表
//! 其餘路徑 → 靜態服務 leptos-poc/dist(同源,免 CORS)。
//! 各 API 加 120ms 人工延遲,讓前端 loading 狀態可見。

use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::{extract::Path, routing::get, Json, Router};
use serde_json::{json, Value};
use std::time::Duration;
use tower_http::services::{ServeDir, ServeFile};

/// 表單 schema:每次請求「現讀」磁碟檔案 —— 改 JSON、前端重載即生效,零重編。
/// 這就是「form 不在 Rust source 裡」的證明點。
async fn form_schema() -> impl IntoResponse {
    let path = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "api-server/form-schema.json".to_string());
    match tokio::fs::read_to_string(&path).await {
        Ok(s) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            s,
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [(header::CONTENT_TYPE, "text/plain")],
            format!("schema file '{path}' unreadable: {e}"),
        ),
    }
}

async fn departments() -> Json<Value> {
    tokio::time::sleep(Duration::from_millis(120)).await;
    Json(json!([
        { "id": 1, "name": "IT" },
        { "id": 2, "name": "QA" },
        { "id": 3, "name": "R&D" },
        { "id": 4, "name": "Manufacturing" },
    ]))
}

async fn members(Path(dept): Path<u32>) -> Json<Value> {
    tokio::time::sleep(Duration::from_millis(120)).await;
    let rows = match dept {
        1 => json!([
            { "name": "Lin Cheng-han", "title": "Systems Engineer" },
            { "name": "Chen Yu-tong", "title": "Network Admin" },
            { "name": "Chang Yu-cheng", "title": "MIS Lead" },
        ]),
        2 => json!([
            { "name": "Huang Yu-shan", "title": "QA Engineer" },
            { "name": "Liu Kuan-ting", "title": "QA Deputy Manager" },
        ]),
        3 => json!([
            { "name": "Wu Pei-jung", "title": "Firmware Engineer" },
            { "name": "Tsai Ming-hsuan", "title": "Algorithm Engineer" },
            { "name": "Hsu Chia-yu", "title": "R&D Manager" },
            { "name": "Wang Po-sen", "title": "Hardware Engineer" },
        ]),
        4 => json!([
            { "name": "Li Chun-yi", "title": "Process Engineer" },
            { "name": "Chou Chih-hsuan", "title": "Line Lead" },
        ]),
        _ => json!([]),
    };
    Json(rows)
}

/// 版面 schema:同 form-schema,每次請求現讀磁碟 —— 整個 app 版面都是資料。
async fn layout_schema() -> impl IntoResponse {
    match tokio::fs::read_to_string("api-server/layout-schema.json").await {
        Ok(s) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            s,
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [(header::CONTENT_TYPE, "text/plain")],
            format!("layout-schema.json unreadable: {e}"),
        ),
    }
}

/// 繪圖範例種子(examples/*.dsl),白名單限定。
async fn example(Path(name): Path<String>) -> impl IntoResponse {
    if !matches!(name.as_str(), "buddha" | "guanyin" | "minecraft" | "mc3p") {
        return (
            StatusCode::NOT_FOUND,
            [(header::CONTENT_TYPE, "text/plain")],
            "unknown example".to_string(),
        );
    }
    match tokio::fs::read_to_string(format!("examples/{name}.dsl")).await {
        Ok(s) => (StatusCode::OK, [(header::CONTENT_TYPE, "text/plain")], s),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [(header::CONTENT_TYPE, "text/plain")],
            format!("examples/{name}.dsl unreadable: {e}"),
        ),
    }
}

/// 真 AssemblyScript 編出的種子(asc 產物),application/wasm。
async fn as_wasm(Path(name): Path<String>) -> impl IntoResponse {
    if name != "buddha" {
        return (StatusCode::NOT_FOUND, [(header::CONTENT_TYPE, "text/plain")], Vec::new());
    }
    match tokio::fs::read(format!("assemblyscript/build/{name}.wasm")).await {
        Ok(b) => (StatusCode::OK, [(header::CONTENT_TYPE, "application/wasm")], b),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [(header::CONTENT_TYPE, "text/plain")],
            format!("{name}.wasm not built (cd assemblyscript && npm run build): {e}").into_bytes(),
        ),
    }
}

/// AssemblyScript 源碼(給前端顯示語法用)。
async fn as_src(Path(name): Path<String>) -> impl IntoResponse {
    if name != "buddha" {
        return (StatusCode::NOT_FOUND, [(header::CONTENT_TYPE, "text/plain")], String::new());
    }
    match tokio::fs::read_to_string(format!("assemblyscript/assembly/{name}.ts")).await {
        Ok(s) => (StatusCode::OK, [(header::CONTENT_TYPE, "text/plain; charset=utf-8")], s),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, [(header::CONTENT_TYPE, "text/plain")], e.to_string()),
    }
}

#[tokio::main]
async fn main() {
    let dist = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "leptos-poc/dist".to_string());
    let index = format!("{dist}/index.html");
    let app = Router::new()
        .route("/api/departments", get(departments))
        .route("/api/members/{dept}", get(members))
        .route("/api/form-schema", get(form_schema))
        .route("/api/layout-schema", get(layout_schema))
        .route("/api/examples/{name}", get(example))
        .route("/api/as/{name}", get(as_wasm))
        .route("/api/as-src/{name}", get(as_src))
        .fallback_service(ServeDir::new(&dist).not_found_service(ServeFile::new(index)));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8645").await.unwrap();
    println!("api-server: http://127.0.0.1:8645  (dist = {dist})");
    axum::serve(listener, app).await.unwrap();
}
