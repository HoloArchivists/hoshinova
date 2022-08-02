use super::TaskMap;
use actix_web::{get, web, HttpResponse, Responder};

#[derive(rust_embed::RustEmbed)]
#[folder = "web/dist"]
struct StaticFiles;

/// Configure routes for the webserver
pub fn configure(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.service(api_tasks);
    cfg.service(api_version);
    cfg.service(serve_static);
}

#[get("/api/tasks")]
async fn api_tasks(data: TaskMap) -> actix_web::Result<impl Responder> {
    Ok(HttpResponse::Ok().json(
        data.read()
            .await
            .iter()
            .map(|(_, v)| v.to_owned())
            .collect::<Vec<_>>(),
    ))
}

#[get("/api/version")]
async fn api_version() -> actix_web::Result<impl Responder> {
    Ok(HttpResponse::Ok().body(crate::APP_NAME.to_owned()))
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
