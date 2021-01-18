/**
 * 画像のアップロード処理
 * http://localhost:8080/upload
 */
use crate::models::NewCard;
use crate::schema::cards;
use actix_files::Files;
use actix_multipart::Multipart;
use actix_web::{error, web, Error, HttpResponse};
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use futures::{StreamExt, TryStreamExt};
use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::str;
use tera::Tera;

fn insert_to_ctx(
    ctx: &mut tera::Context,
    face_upload_confirm: &str,
    back_upload_confirm: &str,
    back_file_names: Vec<String>,
) -> tera::Context {
    ctx.insert("face_upload_confirm", face_upload_confirm);
    ctx.insert("back_upload_confirm", back_upload_confirm);
    ctx.insert("back_file_names", &back_file_names);
    return ctx.clone();
}

// 背面用の画像一覧をファイル名でソートして返す
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

// ファイル名が重複した際にファイル名末尾に_1,_2,_3,...と数字を追加する
fn add_suffix(target_path: String) -> String {
    let path = Path::new(&target_path);
    let parent = path.parent().and_then(Path::to_str).unwrap();
    let stem = path.file_stem().and_then(OsStr::to_str).unwrap();
    let extension = path.extension().and_then(OsStr::to_str).unwrap();
    let mut file_path = target_path.clone();
    let mut count = 1;
    while Path::new(&file_path).exists() {
        file_path = format!("{}/{}_{}.{}", parent, stem, count, extension);
        count += 1;
    }
    return file_path.to_string();
}

async fn view_upload_screen(tmpl: web::Data<tera::Tera>) -> Result<HttpResponse, Error> {
    let mut ctx = tera::Context::new();
    let inserted_ctx = insert_to_ctx(&mut ctx, "", "", get_back_file_names());
    let view = tmpl
        .render("upload.html", &inserted_ctx)
        .map_err(|e| error::ErrorInternalServerError(e))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(view))
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
            let not_exist_path = add_suffix(format!(
                "assets/face/{}",
                sanitize_filename::sanitize(&filename)
            ));
            let mut f = web::block(|| std::fs::File::create(not_exist_path))
                .await
                .unwrap();
            while let Some(chunk) = field.next().await {
                let data = chunk.unwrap();
                f = web::block(move || f.write_all(&data).map(|_| f)).await?;
            }
        } else {
            // 選択した裏面画像を記録
            while let Some(chunk) = field.next().await {
                let data = chunk.unwrap();
                back_img_file_name = str::from_utf8(&data).unwrap().to_string();
            }
        }
    }
    for face_img_file_name in face_img_file_names {
        let new_card = NewCard {
            face: format!("/assets/face/{}", face_img_file_name),
            back: format!("/assets/back/{}", String::from(&back_img_file_name)),
        };
        let conn = pool.get().expect("couldn't get db connection from pool");
        diesel::insert_into(cards::table)
            .values(&new_card)
            .execute(&conn)
            .unwrap();
    }
    let mut ctx = tera::Context::new();
    let inserted_ctx = insert_to_ctx(&mut ctx, "アップロードしました", "", get_back_file_names());
    let view = tmpl
        .render("upload.html", &inserted_ctx)
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
        let not_exist_path = add_suffix(format!(
            "assets/back/{}",
            sanitize_filename::sanitize(&filename)
        ));
        let mut f = web::block(|| std::fs::File::create(not_exist_path))
            .await
            .unwrap();
        while let Some(chunk) = field.next().await {
            let data = chunk.unwrap();
            f = web::block(move || f.write_all(&data).map(|_| f)).await?;
        }
    }
    let mut ctx = tera::Context::new();
    let inserted_ctx = insert_to_ctx(&mut ctx, "", "アップロードしました", get_back_file_names());
    let view = tmpl
        .render("upload.html", &inserted_ctx)
        .map_err(|e| error::ErrorInternalServerError(e))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(view))
}

pub fn register(config: &mut web::ServiceConfig) {
    let templates = Tera::new("templates/**/*").unwrap();
    config
        .data(templates)
        .route("/upload", web::get().to(view_upload_screen))
        .route("/upload/face", web::post().to(upload_face))
        .route("/upload/back", web::post().to(upload_back))
        .service(Files::new("/assets", "assets").show_files_listing());
}
