use axum::{
    extract::Path,
    http::StatusCode,
    response::Html,
    routing::{delete, get, post},
    Json, Router,
};
use chrono::{Local, NaiveDate};
use serde::Deserialize;
use tokio::net::TcpListener;

use crate::data::{build_report, load, load_goal, save, save_goal, Entry, Report};

const INDEX_HTML: &str = include_str!("index.html");

#[derive(Deserialize)]
struct LogBody {
    weight: f32,
    date: Option<String>,
}

#[derive(Deserialize)]
struct GoalBody {
    weight: f32,
}

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn api_report() -> Json<Report> {
    Json(build_report(load(), load_goal()))
}

async fn api_log(
    Json(body): Json<LogBody>,
) -> Result<Json<Entry>, (StatusCode, String)> {
    let date = match body.date {
        Some(s) => NaiveDate::parse_from_str(&s, "%Y-%m-%d")
            .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid date".to_string()))?,
        None => Local::now().date_naive(),
    };

    let mut entries = load();
    if let Some(e) = entries.iter_mut().find(|e| e.date == date) {
        e.weight = body.weight;
        let entry = e.clone();
        save(&entries);
        Ok(Json(entry))
    } else {
        let entry = Entry { date, weight: body.weight };
        entries.push(entry.clone());
        entries.sort_by_key(|e| e.date);
        save(&entries);
        Ok(Json(entry))
    }
}

async fn api_delete(
    Path(date_str): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid date".to_string()))?;

    let mut entries = load();
    let before = entries.len();
    entries.retain(|e| e.date != date);

    if entries.len() == before {
        return Err((StatusCode::NOT_FOUND, format!("No entry for {}", date)));
    }
    save(&entries);
    Ok(StatusCode::NO_CONTENT)
}

async fn api_get_goal() -> Json<Option<f32>> {
    Json(load_goal())
}

async fn api_set_goal(Json(body): Json<GoalBody>) -> StatusCode {
    save_goal(body.weight);
    StatusCode::NO_CONTENT
}

pub async fn serve(port: u16) {
    let app = Router::new()
        .route("/", get(index))
        .route("/api/report", get(api_report))
        .route("/api/log", post(api_log))
        .route("/api/entries/:date", delete(api_delete))
        .route("/api/goal", get(api_get_goal).post(api_set_goal));

    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr).await.unwrap();
    println!("Serving at http://{}", addr);
    axum::serve(listener, app).await.unwrap();
}
