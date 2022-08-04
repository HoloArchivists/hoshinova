use super::TaskMap;
use crate::config::Config;
use actix_web::{
    get, post, put,
    web::{self, Data},
    HttpResponse, Responder,
};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(rust_embed::RustEmbed)]
#[folder = "web/dist"]
struct StaticFiles;

/// Configure routes for the webserver
pub fn configure(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.service(get_tasks);
    cfg.service(post_task);
    cfg.service(get_version);
    cfg.service(get_config);
    cfg.service(get_config_toml);
    cfg.service(put_config_toml);
    cfg.service(reload_config);
    cfg.service(serve_static);
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

#[post("/api/task")]
async fn post_task() -> actix_web::Result<impl Responder> {
    Ok(HttpResponse::Ok().json(""))
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
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
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
            .map_err(|e| actix_web::error::ErrorInternalServerError(e))?,
    ))
}

#[put("/api/config/toml")]
async fn put_config_toml(
    config: Data<Arc<RwLock<Config>>>,
    body: web::Bytes,
) -> actix_web::Result<impl Responder> {
    let body = std::str::from_utf8(&body).map_err(|e| actix_web::error::ErrorBadRequest(e))?;
    config
        .write()
        .await
        .set_source_toml(body)
        .await
        .map_err(|e| actix_web::error::ErrorBadRequest(e))?;
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
