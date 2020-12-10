/**
 * 全カードの閲覧，カードの削除処理
 * http://localhost:8080/card
 */
use crate::models::Card;
use crate::schema::cards;
use actix_multipart::Multipart;
use actix_web::{error, web, Error, HttpResponse};
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use futures::{StreamExt, TryStreamExt};
use std::str;
use tera::Tera;

async fn view_card(
    pool: web::Data<r2d2::Pool<ConnectionManager<PgConnection>>>,
    tmpl: web::Data<tera::Tera>,
) -> Result<HttpResponse, Error> {
    let mut ctx = tera::Context::new();
    let conn = pool.get().expect("couldn't get db connection from pool");
    let cards = cards::table
        .load::<Card>(&conn)
        .expect("Error loading cards");
    ctx.insert("cards", &cards);
    let view = tmpl
        .render("card.html", &ctx)
        .map_err(|e| error::ErrorInternalServerError(e))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(view))
}

async fn delete_cards(
    pool: web::Data<r2d2::Pool<ConnectionManager<PgConnection>>>,
    tmpl: web::Data<tera::Tera>,
    mut payload: Multipart,
) -> Result<HttpResponse, Error> {
    let mut card_ids: Vec<i32> = Vec::new();
    while let Ok(Some(mut field)) = payload.try_next().await {
        while let Some(chunk) = field.next().await {
            let data = chunk.unwrap();
            let card_id = str::from_utf8(&data).unwrap().to_string();
            card_ids.push(card_id.parse().unwrap());
        }
    }
    // TODO 本当に削除するか確認
    // TODO belongings テーブルの処理
    let conn = pool.get().expect("couldn't get db connection from pool");
    for card_id in card_ids {
        diesel::delete(cards::table.filter(cards::id.eq(card_id)))
            .execute(&conn)
            .unwrap();
    }
    let mut ctx = tera::Context::new();
    let cards = cards::table
        .load::<Card>(&conn)
        .expect("Error loading cards");
    ctx.insert("cards", &cards);
    let view = tmpl
        .render("card.html", &ctx)
        .map_err(|e| error::ErrorInternalServerError(e))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(view))
}

pub fn register(config: &mut web::ServiceConfig) {
    let templates = Tera::new("templates/**/*").unwrap();
    config
        .data(templates)
        .route("/card", web::get().to(view_card))
        .route("/card/delete", web::post().to(delete_cards));
}
