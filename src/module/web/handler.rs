use super::TaskMap;
use crate::{
    config::Config,
    module::{Message, Task},
    msgbus::BusTx,
    youtube,
};
use actix_web::{
    error::{ErrorBadRequest, ErrorInternalServerError},
    get, post, put,
    web::{self, Data},
    HttpResponse, Responder,
};
use anyhow::anyhow;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::RwLock;
use ts_rs::TS;

#[derive(rust_embed::RustEmbed)]
#[folder = "web/dist"]
struct StaticFiles;

/// Configure routes for the webserver
pub fn configure(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.service(get_tasks)
        .service(post_task)
        .service(get_version)
        .service(get_config)
        .service(get_config_toml)
        .service(put_config_toml)
        .service(reload_config)
        .service(serve_static);
}

#[get("/api/tasks")]
async fn get_tasks(data: TaskMap) -> actix_web::Result<impl Responder> {
    Ok(HttpResponse::Ok().json(
        data.read()
            .await
            .iter()
            .map(|(_, v)| v.to_owned())
            .collect::<Vec<_>>(),
    ))
}

#[derive(Deserialize, TS)]
#[ts(export, export_to = "web/src/bindings/")]
struct CreateTaskRequest {
    video_url: String,
    output_directory: String,
}

// TODO: clean up
#[derive(Deserialize)]
struct InitialPlayerResponse {
    #[serde(rename = "videoDetails")]
    video_details: InitialPlayerResponseVideoDetails,
}
#[derive(Deserialize)]
struct InitialPlayerResponseVideoDetails {
    #[serde(rename = "videoId")]
    video_id: String,
    title: String,
    #[serde(rename = "channelId")]
    channel_id: String,
    author: String,
    thumbnail: InitialPlayerResponseVideoDetailsThumbnail,
}
#[derive(Deserialize)]
struct InitialPlayerResponseVideoDetailsThumbnail {
    thumbnails: Vec<InitialPlayerResponseVideoDetailsThumbnailThumbnail>,
}
#[derive(Deserialize)]
struct InitialPlayerResponseVideoDetailsThumbnailThumbnail {
    url: String,
    width: u32,
    height: u32,
}

#[post("/api/task")]
async fn post_task(
    tx: Data<BusTx<Message>>,
    taskreq: web::Json<CreateTaskRequest>,
) -> actix_web::Result<impl Responder> {
    use actix_web::http::Uri;

    let taskreq = taskreq.into_inner();

    // Parse URL
    let uri = &taskreq
        .video_url
        .parse::<Uri>()
        .map_err(|e| ErrorBadRequest(anyhow!("Invalid URL: {}", e)))?;

    // Make sure it's a supported URL
    let host = uri.host().ok_or(ErrorBadRequest(anyhow!("Invalid URL")))?;
    if host != "youtube.com" && host != "www.youtube.com" && host != "youtu.be" {
        return Err(ErrorBadRequest(anyhow!("Unsupported URL")));
    }

    // Fetch the video URL
    let html = reqwest::get(&taskreq.video_url)
        .await
        .map_err(|e| ErrorInternalServerError(anyhow!("Failed to fetch video URL: {}", e)))?
        .error_for_status()
        .map_err(|e| ErrorInternalServerError(anyhow!("Failed to fetch video URL: {}", e)))?
        .text()
        .await
        .map_err(|e| ErrorInternalServerError(anyhow!("Failed to fetch video URL: {}", e)))?;

    // Parse page contents
    // TODO: refactor out into a separate module
    lazy_static::lazy_static! {
        static ref IPR_RE: regex::Regex =
            regex::Regex::new(r#"ytInitialPlayerResponse = (.*?});"#).unwrap();
    }

    let ipr = IPR_RE
        .captures(&html)
        .ok_or(ErrorInternalServerError(anyhow!(
            "Failed to find the initial player response"
        )))?
        .get(1)
        .ok_or(ErrorInternalServerError(anyhow!(
            "Failed to find the initial player response"
        )))?
        .as_str();

    // Parse the initial player response
    let ipr: InitialPlayerResponse = serde_json::from_str(ipr).map_err(|e| {
        ErrorInternalServerError(anyhow!(
            "Failed to parse the initial player response: {}",
            e
        ))
    })?;

    // Get the best thumbnail
    let mut thumbs = ipr.video_details.thumbnail.thumbnails;
    thumbs.sort_by_key(|t| t.width);

    // Fetch the channel image
    // TODO: reuse the Client
    let channel_picture =
        youtube::channel::fetch_picture_url(reqwest::Client::new(), &ipr.video_details.channel_id)
            .await
            .map_err(|e| {
                ErrorInternalServerError(anyhow!("Failed to fetch channel picture: {}", e))
            })?;

    // Create the task
    let task = Task {
        title: ipr.video_details.title,
        video_id: ipr.video_details.video_id,
        video_picture: thumbs.last().map(|t| t.url.clone()).unwrap_or("".into()),
        channel_name: ipr.video_details.author,
        channel_id: ipr.video_details.channel_id,
        channel_picture: Some(channel_picture),
        output_directory: taskreq.output_directory,
    };

    tx.send(Message::ToRecord(task))
        .await
        .map_err(|e| ErrorInternalServerError(e))?;
    Ok(HttpResponse::Accepted().finish())
}

#[get("/api/version")]
async fn get_version() -> actix_web::Result<impl Responder> {
    Ok(HttpResponse::Ok().body(crate::APP_NAME.to_owned()))
}

#[get("/api/config")]
async fn get_config(config: Data<Arc<RwLock<Config>>>) -> actix_web::Result<impl Responder> {
    Ok(HttpResponse::Ok().json(config.read().await.to_owned()))
}

#[post("/api/config/reload")]
async fn reload_config(config: Data<Arc<RwLock<Config>>>) -> actix_web::Result<impl Responder> {
    config
        .write()
        .await
        .reload()
        .await
        .map_err(|e| ErrorInternalServerError(e))?;
    Ok(HttpResponse::Ok().json("ok"))
}

#[get("/api/config/toml")]
async fn get_config_toml(config: Data<Arc<RwLock<Config>>>) -> actix_web::Result<impl Responder> {
    Ok(HttpResponse::Ok().body(
        config
            .read()
            .await
            .get_source_toml()
            .await
            .map_err(|e| ErrorInternalServerError(e))?,
    ))
}

#[put("/api/config/toml")]
async fn put_config_toml(
    config: Data<Arc<RwLock<Config>>>,
    body: web::Bytes,
) -> actix_web::Result<impl Responder> {
    let body = std::str::from_utf8(&body).map_err(|e| ErrorBadRequest(e))?;
    config
        .write()
        .await
        .set_source_toml(body)
        .await
        .map_err(|e| ErrorBadRequest(e))?;
    Ok(HttpResponse::Ok().json("ok"))
}

#[get("/{_:.*}")]
async fn serve_static(path: web::Path<String>) -> impl Responder {
    let mut path = path.into_inner();
    if path.is_empty() {
        path = "index.html".to_string();
    }

    // If debug mode, serve the files from the static folder
    #[cfg(debug_assertions)]
    return tokio::fs::read(format!("web/dist/{}", path))
        .await
        .map(|bytes| {
            HttpResponse::Ok()
                .content_type(mime_guess::from_path(path).first_or_octet_stream().as_ref())
                .body(bytes)
        })
        .unwrap_or_else(|_| HttpResponse::NotFound().body("404"));

    // Otherwise serve the files from the embedded folder
    #[cfg(not(debug_assertions))]
    return match StaticFiles::get(&path) {
        Some(content) => HttpResponse::Ok()
            .content_type(mime_guess::from_path(path).first_or_octet_stream().as_ref())
            .body(content.data.into_owned()),
        None => HttpResponse::NotFound().body("404 Not Found"),
    };
}
