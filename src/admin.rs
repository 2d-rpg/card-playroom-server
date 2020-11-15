use actix_web::{web, HttpResponse};
use juniper::http::playground::playground_source;

fn playground() -> HttpResponse {
    let html = playground_source("");
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

pub fn register(config: &mut web::ServiceConfig) {
    config.route("/", web::get().to(playground));
}
