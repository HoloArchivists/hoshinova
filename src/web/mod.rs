use actix_web::{get, web::ServiceConfig, HttpResponse, Responder};

pub fn configure(app: &mut ServiceConfig) {
    app.service(hello);
}

#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Hello world!")
}
