use crate::schema::{cards, decks};
use actix_files::Files;
use actix_multipart::Multipart;
use actix_web::{error, web, Error, HttpRequest, HttpResponse};
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

async fn upload(tmpl: web::Data<tera::Tera>) -> Result<HttpResponse, Error> {
    let mut ctx = tera::Context::new();
    ctx.insert("face_upload_confirm", &"".to_owned());
    ctx.insert("back_upload_confirm", &"".to_owned());
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
    for face_img_file_name in face_img_file_names {
        let new_card = crate::models::NewCard {
            deck_id: None,
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
    ctx.insert("face_upload_confirm", &"アップロードしました".to_owned());
    ctx.insert("back_upload_confirm", &"".to_owned());
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
    ctx.insert("face_upload_confirm", &"".to_owned());
    ctx.insert("back_upload_confirm", &"アップロードしました".to_owned());
    ctx.insert("back_file_names", &get_back_file_names().to_owned());
    let view = tmpl
        .render("upload.html", &ctx)
        .map_err(|e| error::ErrorInternalServerError(e))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(view))
}

async fn all_cards(
    pool: web::Data<r2d2::Pool<ConnectionManager<PgConnection>>>,
    tmpl: web::Data<tera::Tera>,
) -> Result<HttpResponse, Error> {
    let conn = pool.get().expect("couldn't get db connection from pool");
    let mut ctx = tera::Context::new();
    let cards = cards::table
        .load::<crate::models::Card>(&conn)
        .expect("Error loading cards");
    let decks = decks::table
        .load::<crate::models::Deck>(&conn)
        .expect("Error loading decks");
    ctx.insert("cards", &cards);
    ctx.insert("decks", &decks);
    ctx.insert("add_deck_confirm", &"".to_owned());
    ctx.insert("deck_name", &"すべてのカード".to_owned());
    let view = tmpl
        .render("deck.html", &ctx)
        .map_err(|e| error::ErrorInternalServerError(e))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(view))
}

#[derive(Serialize, Deserialize)]
pub struct AddDeckFormParams {
    deck_name: String,
}
async fn add_deck(
    pool: web::Data<r2d2::Pool<ConnectionManager<PgConnection>>>,
    tmpl: web::Data<tera::Tera>,
    params: web::Form<AddDeckFormParams>,
) -> Result<HttpResponse, Error> {
    let new_deck = crate::models::NewDeck {
        name: String::from(&params.deck_name),
    };
    let conn = pool.get().expect("couldn't get db connection from pool");
    diesel::insert_into(decks::table)
        .values(&new_deck)
        .execute(&conn)
        .unwrap();
    let mut ctx = tera::Context::new();
    let cards = cards::table
        .load::<crate::models::Card>(&conn)
        .expect("Error loading cards");
    let decks = decks::table
        .load::<crate::models::Deck>(&conn)
        .expect("Error loading decks");
    ctx.insert("cards", &cards);
    ctx.insert("decks", &decks);
    ctx.insert("add_deck_confirm", &"デッキを追加しました".to_owned());
    ctx.insert("deck_name", &"すべてのカード".to_owned());
    let view = tmpl
        .render("deck.html", &ctx)
        .map_err(|e| error::ErrorInternalServerError(e))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(view))
}

async fn deck(
    req: HttpRequest,
    pool: web::Data<r2d2::Pool<ConnectionManager<PgConnection>>>,
    tmpl: web::Data<tera::Tera>,
) -> Result<HttpResponse, Error> {
    let deck_id: i32 = req.match_info().get("deck_id").unwrap().parse().unwrap();
    let conn = pool.get().expect("couldn't get db connection from pool");
    let mut ctx = tera::Context::new();
    let selected_deck = decks::table
        .find(deck_id)
        .first::<crate::models::Deck>(&conn)
        .expect("Error loading deck");
    let cards = crate::models::Card::belonging_to(&selected_deck)
        .load::<crate::models::Card>(&conn)
        .expect("Error loading cards");
    let decks = decks::table
        .load::<crate::models::Deck>(&conn)
        .expect("Error loading decks");
    ctx.insert("cards", &cards);
    ctx.insert("decks", &decks);
    ctx.insert("add_deck_confirm", &"".to_owned());
    ctx.insert("deck_name", &selected_deck.name.to_owned());
    let view = tmpl
        .render("deck.html", &ctx)
        .map_err(|e| error::ErrorInternalServerError(e))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(view))
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EditDeckFormParams {
    action: String, // "copy" or "delete"
    card_id: Vec<String>,
}
async fn edit_deck(
    pool: web::Data<r2d2::Pool<ConnectionManager<PgConnection>>>,
    tmpl: web::Data<tera::Tera>,
    mut payload: Multipart,
) -> Result<HttpResponse, Error> {
    let mut action: String = String::from("");
    let mut card_ids: Vec<i32> = Vec::new();
    while let Ok(Some(mut field)) = payload.try_next().await {
        let content_type = field.content_disposition().unwrap();
        let name = content_type.get_name().unwrap();
        if name == "action" {
            while let Some(chunk) = field.next().await {
                let data = chunk.unwrap();
                action = str::from_utf8(&data).unwrap().to_string();
            }
        } else if name == "card_id[]" {
            while let Some(chunk) = field.next().await {
                let data = chunk.unwrap();
                let card_id = str::from_utf8(&data).unwrap().to_string();
                card_ids.push(card_id.parse().unwrap());
            }
        }
    }
    dbg!(action);
    dbg!(card_ids);
    let conn = pool.get().expect("couldn't get db connection from pool");
    let mut ctx = tera::Context::new();
    let cards = cards::table
        .load::<crate::models::Card>(&conn)
        .expect("Error loading cards");
    let decks = decks::table
        .load::<crate::models::Deck>(&conn)
        .expect("Error loading decks");
    ctx.insert("cards", &cards);
    ctx.insert("decks", &decks);
    ctx.insert("add_deck_confirm", &"".to_owned());
    ctx.insert("deck_name", &"すべてのカード".to_owned());
    let view = tmpl
        .render("deck.html", &ctx)
        .map_err(|e| error::ErrorInternalServerError(e))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(view))
}

pub fn register(config: &mut web::ServiceConfig) {
    let templates = Tera::new("templates/**/*").unwrap();
    config
        .data(templates)
        .route("/", web::get().to(index))
        .route("/upload", web::get().to(upload))
        .route("/upload/face", web::post().to(upload_face))
        .route("/upload/back", web::post().to(upload_back))
        .route("/deck", web::get().to(all_cards))
        .route("/deck/{deck_id}", web::get().to(deck))
        .route("/deck/add", web::post().to(add_deck))
        .route("/deck/edit", web::post().to(edit_deck))
        .service(Files::new("/assets", "assets").show_files_listing());
}
