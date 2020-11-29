use actix_files::Files;
use actix_multipart::Multipart;
use actix_web::{error, web, Error, HttpResponse};
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use futures::{StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::str;
use tera::Tera;

fn get_back_file_names() -> Vec<String> {
    let paths = fs::read_dir("assets/back").unwrap();
    let mut file_names: Vec<String> = Vec::new();
    for path in paths {
        let file_name = path
            .unwrap()
            .path()
            .as_path()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        if file_name != ".gitkeep" {
            file_names.push(file_name);
        }
    }
    file_names.sort();
    file_names
}

async fn index(
    pool: web::Data<r2d2::Pool<ConnectionManager<PgConnection>>>,
    tmpl: web::Data<tera::Tera>,
) -> Result<HttpResponse, Error> {
    let mut ctx = tera::Context::new();
    use crate::schema::cards;
    let conn = pool.get().expect("couldn't get db connection from pool");
    let cards = cards::table
        .load::<crate::models::Card>(&conn)
        .expect("Error loading cards");
    ctx.insert("cards", &cards);
    let view = tmpl
        .render("index.html", &ctx)
        .map_err(|e| error::ErrorInternalServerError(e))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(view))
}

async fn upload_page(tmpl: web::Data<tera::Tera>) -> Result<HttpResponse, Error> {
    let mut ctx = tera::Context::new();
    ctx.insert("face_upload_comfirm", &"".to_owned());
    ctx.insert("back_upload_comfirm", &"".to_owned());
    ctx.insert("back_file_names", &get_back_file_names().to_owned());
    let view = tmpl
        .render("upload.html", &ctx)
        .map_err(|e| error::ErrorInternalServerError(e))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(view))
}

#[derive(Serialize, Deserialize)]
pub struct MyParams {
    back_image: String,
}

async fn upload_face(
    pool: web::Data<r2d2::Pool<ConnectionManager<PgConnection>>>,
    tmpl: web::Data<tera::Tera>,
    mut payload: Multipart,
) -> Result<HttpResponse, Error> {
    let mut face_img_file_names: Vec<String> = Vec::new();
    let mut back_img_file_name: String = String::from("");
    while let Ok(Some(mut field)) = payload.try_next().await {
        let content_type = field.content_disposition().unwrap();
        if content_type.get_filename() != None {
            // 画像ファイルの保存
            let filename = content_type.get_filename().unwrap();
            face_img_file_names.push(filename.to_string());
            // TODO ファイル名重複防止
            let filepath = format!("assets/face/{}", sanitize_filename::sanitize(&filename));
            // File::create is blocking operation, use threadpool
            let mut f = web::block(|| std::fs::File::create(filepath))
                .await
                .unwrap();
            // Field in turn is stream of *Bytes* object
            while let Some(chunk) = field.next().await {
                let data = chunk.unwrap();
                // filesystem operations are blocking, we have to use threadpool
                f = web::block(move || f.write_all(&data).map(|_| f)).await?;
            }
        } else {
            while let Some(chunk) = field.next().await {
                let data = chunk.unwrap();
                back_img_file_name = str::from_utf8(&data).unwrap().to_string();
            }
        }
    }
    use crate::schema::cards;
    for face_img_file_name in face_img_file_names {
        let new_card = crate::models::NewCard {
            face: face_img_file_name,
            back: String::from(&back_img_file_name),
        };
        let conn = pool.get().expect("couldn't get db connection from pool");
        diesel::insert_into(cards::table)
            .values(&new_card)
            .execute(&conn)
            .unwrap();
    }
    let mut ctx = tera::Context::new();
    ctx.insert(
        "face_upload_comfirm",
        &"アップロードしました！！".to_owned(),
    );
    ctx.insert("back_upload_comfirm", &"".to_owned());
    ctx.insert("back_file_names", &get_back_file_names().to_owned());
    let view = tmpl
        .render("upload.html", &ctx)
        .map_err(|e| error::ErrorInternalServerError(e))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(view))
}

async fn upload_back(
    tmpl: web::Data<tera::Tera>,
    mut payload: Multipart,
) -> Result<HttpResponse, Error> {
    while let Ok(Some(mut field)) = payload.try_next().await {
        let content_type = field.content_disposition().unwrap();
        let filename = content_type.get_filename().unwrap();
        // TODO ファイル名重複防止
        let filepath = format!("assets/back/{}", sanitize_filename::sanitize(&filename));
        // File::create is blocking operation, use threadpool
        let mut f = web::block(|| std::fs::File::create(filepath))
            .await
            .unwrap();

        // Field in turn is stream of *Bytes* object
        while let Some(chunk) = field.next().await {
            let data = chunk.unwrap();
            // filesystem operations are blocking, we have to use threadpool
            f = web::block(move || f.write_all(&data).map(|_| f)).await?;
        }
    }
    let mut ctx = tera::Context::new();
    ctx.insert("face_upload_comfirm", &"".to_owned());
    ctx.insert(
        "back_upload_comfirm",
        &"アップロードしました！！".to_owned(),
    );
    ctx.insert("back_file_names", &get_back_file_names().to_owned());
    let view = tmpl
        .render("upload.html", &ctx)
        .map_err(|e| error::ErrorInternalServerError(e))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(view))
}

pub fn register(config: &mut web::ServiceConfig) {
    let templates = Tera::new("templates/**/*").unwrap();
    config
        .data(templates)
        .route("/", web::get().to(index))
        .route("/upload", web::get().to(upload_page))
        .route("/upload/face", web::post().to(upload_face))
        .route("/upload/back", web::post().to(upload_back))
        .service(Files::new("/assets", "assets").show_files_listing());
}
