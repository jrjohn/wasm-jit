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
        { "id": 1, "name": "資訊部" },
        { "id": 2, "name": "品保部" },
        { "id": 3, "name": "研發部" },
        { "id": 4, "name": "製造部" },
    ]))
}

async fn members(Path(dept): Path<u32>) -> Json<Value> {
    tokio::time::sleep(Duration::from_millis(120)).await;
    let rows = match dept {
        1 => json!([
            { "name": "林承翰", "title": "系統工程師" },
            { "name": "陳語彤", "title": "網路管理師" },
            { "name": "張育誠", "title": "MIS 主任" },
        ]),
        2 => json!([
            { "name": "黃于珊", "title": "品保工程師" },
            { "name": "劉冠廷", "title": "品保副理" },
        ]),
        3 => json!([
            { "name": "吳沛蓉", "title": "韌體工程師" },
            { "name": "蔡明軒", "title": "演算法工程師" },
            { "name": "許家瑜", "title": "研發經理" },
            { "name": "王柏森", "title": "硬體工程師" },
        ]),
        4 => json!([
            { "name": "李俊毅", "title": "製程工程師" },
            { "name": "周芷萱", "title": "產線組長" },
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
        .fallback_service(ServeDir::new(&dist).not_found_service(ServeFile::new(index)));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8645").await.unwrap();
    println!("api-server: http://127.0.0.1:8645  (dist = {dist})");
    axum::serve(listener, app).await.unwrap();
}
