/**
 * デッキの作成，削除処理
 * http://localhost:8080/deck
 */
use crate::models::{Deck, NewDeck};
use crate::schema::{belongings, decks};
use actix_web::{error, web, Error, HttpResponse};
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use serde::{Deserialize, Serialize};
use std::str;
use tera::Tera;

fn insert_to_ctx(
    ctx: &mut tera::Context,
    conn: diesel::r2d2::PooledConnection<diesel::r2d2::ConnectionManager<diesel::PgConnection>>,
    add_deck_confirm: &str,
    delete_deck_confirm: &str,
) -> tera::Context {
    let decks = decks::table
        .order_by(decks::id.asc())
        .load::<Deck>(&conn)
        .expect("Error loading decks");
    ctx.insert("decks", &decks);
    ctx.insert("add_deck_confirm", add_deck_confirm);
    ctx.insert("delete_deck_confirm", delete_deck_confirm);
    return ctx.clone();
}

async fn view_deck(
    pool: web::Data<r2d2::Pool<ConnectionManager<PgConnection>>>,
    tmpl: web::Data<tera::Tera>,
) -> Result<HttpResponse, Error> {
    let conn = pool.get().expect("couldn't get db connection from pool");
    let mut ctx = tera::Context::new();
    let inserted_ctx = insert_to_ctx(&mut ctx, conn, "", "");
    let view = tmpl
        .render("deck.html", &inserted_ctx)
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
    let deck_name = String::from(&params.deck_name);
    let mut ctx = tera::Context::new();
    let conn = pool.get().expect("couldn't get db connection from pool");
    let existing_deck = decks::table
        .filter(decks::name.eq(&deck_name))
        .first::<Deck>(&conn);
    if existing_deck.is_err() {
        let new_deck = NewDeck { name: deck_name };
        diesel::insert_into(decks::table)
            .values(&new_deck)
            .execute(&conn)
            .unwrap();
        let inserted_ctx = insert_to_ctx(&mut ctx, conn, "デッキを追加しました", "");
        let view = tmpl
            .render("deck.html", &inserted_ctx)
            .map_err(|e| error::ErrorInternalServerError(e))?;
        return Ok(HttpResponse::Ok().content_type("text/html").body(view));
    } else {
        let inserted_ctx = insert_to_ctx(&mut ctx, conn, "その名前のデッキは既に存在します", "");
        let view = tmpl
            .render("deck.html", &inserted_ctx)
            .map_err(|e| error::ErrorInternalServerError(e))?;
        return Ok(HttpResponse::Ok().content_type("text/html").body(view));
    }
}

#[derive(Serialize, Deserialize)]
pub struct DeleteDeckFormParams {
    deck_id: i32,
}
async fn delete_deck(
    pool: web::Data<r2d2::Pool<ConnectionManager<PgConnection>>>,
    tmpl: web::Data<tera::Tera>,
    params: web::Form<DeleteDeckFormParams>,
) -> Result<HttpResponse, Error> {
    let conn = pool.get().expect("couldn't get db connection from pool");
    diesel::delete(belongings::table.filter(belongings::deck_id.eq(params.deck_id)))
        .execute(&conn)
        .unwrap();
    diesel::delete(decks::table.filter(decks::id.eq(params.deck_id)))
        .execute(&conn)
        .unwrap();
    let mut ctx = tera::Context::new();
    let inserted_ctx = insert_to_ctx(&mut ctx, conn, "", "デッキを削除しました");
    let view = tmpl
        .render("deck.html", &inserted_ctx)
        .map_err(|e| error::ErrorInternalServerError(e))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(view))
}

pub fn register(config: &mut web::ServiceConfig) {
    let templates = Tera::new("templates/**/*").unwrap();
    config
        .data(templates)
        .route("/deck", web::get().to(view_deck))
        .route("/deck/add", web::post().to(add_deck))
        .route("/deck/delete", web::post().to(delete_deck));
}
