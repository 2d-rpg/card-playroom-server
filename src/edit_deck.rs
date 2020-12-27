/**
 * デッキ編集処理
 * http://localhost:8080/edit-deck
 */
use crate::models::{Belonging, Card, Deck, NewBelonging};
use crate::schema::{belongings, cards, decks};
use actix_multipart::Multipart;
use actix_web::{error, web, Error, HttpRequest, HttpResponse};
use diesel::pg::expression::dsl::any;
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use futures::{StreamExt, TryStreamExt};
use serde::Serialize;
use std::str;
use tera::Tera;

#[derive(Serialize)]
struct CardInfoInDeck {
    card_id: i32,
    face: String,
    back: String,
    num: i32,
}

fn insert_to_ctx(
    ctx: &mut tera::Context,
    conn: diesel::r2d2::PooledConnection<diesel::r2d2::ConnectionManager<diesel::PgConnection>>,
    is_deck_selected: bool,
    selected_deck_id: i32,
    edit_deck_confirm: &str,
) -> tera::Context {
    let cards = cards::table
        .order_by(cards::id.asc())
        .load::<Card>(&conn)
        .expect("Error loading cards");
    let decks = decks::table
        .order_by(decks::id.asc())
        .load::<Deck>(&conn)
        .expect("Error loading decks");
    ctx.insert("cards", &cards);
    ctx.insert("decks", &decks);
    ctx.insert("is_deck_selected", &is_deck_selected);
    ctx.insert("edit_deck_confirm", &edit_deck_confirm);
    if !is_deck_selected {
        let empty_vec: Vec<CardInfoInDeck> = Vec::new();
        ctx.insert("selected_deck_id", &"");
        ctx.insert("selected_deck_name", &"");
        ctx.insert("cards_in_deck", &empty_vec);
    } else {
        let selected_deck = decks::table
            .find(selected_deck_id)
            .first::<Deck>(&conn)
            .expect("Error loading deck");
        let card_ids_in_selected_deck =
            Belonging::belonging_to(&selected_deck).select(belongings::card_id);
        let cards_in_selected_deck = cards::table
            .filter(cards::id.eq(any(card_ids_in_selected_deck)))
            .order_by(cards::id.asc())
            .load::<Card>(&conn)
            .expect("Error loading cards");
        let cards_id_and_num_in_selected_deck = &mut Belonging::belonging_to(&selected_deck)
            .select((belongings::card_id, belongings::num))
            .load::<(i32, i32)>(&conn)
            .expect("Error loading belongings");
        let cards_info_in_selected_deck: Vec<CardInfoInDeck> = cards_in_selected_deck
            .into_iter()
            .map(|card_in_selected_deck| {
                let card_id = card_in_selected_deck.id;
                let face = card_in_selected_deck.face;
                let back = card_in_selected_deck.back;
                let card_num_in_selected_deck = cards_id_and_num_in_selected_deck
                    .into_iter()
                    .find(|card_id_and_num| card_id_and_num.0 == card_id)
                    .unwrap()
                    .1;
                return CardInfoInDeck {
                    card_id: card_id,
                    face: face,
                    back: back,
                    num: card_num_in_selected_deck,
                };
            })
            .collect();
        ctx.insert("selected_deck_id", &selected_deck.id);
        ctx.insert("selected_deck_name", &selected_deck.name);
        ctx.insert("cards_in_deck", &cards_info_in_selected_deck);
    }
    return ctx.clone();
}

async fn view_edit_deck_screen(
    req: HttpRequest,
    pool: web::Data<r2d2::Pool<ConnectionManager<PgConnection>>>,
    tmpl: web::Data<tera::Tera>,
) -> Result<HttpResponse, Error> {
    let optional_selected_deck_id = req.match_info().get("deck_id");
    let conn = pool.get().expect("couldn't get db connection from pool");
    let mut ctx = tera::Context::new();
    if optional_selected_deck_id.is_some() {
        let selected_deck_id: i32 = optional_selected_deck_id.unwrap().parse().unwrap();
        let inserted_ctx = insert_to_ctx(&mut ctx, conn, true, selected_deck_id, "");
        let view = tmpl
            .render("edit-deck.html", &inserted_ctx)
            .map_err(|e| error::ErrorInternalServerError(e))?;
        return Ok(HttpResponse::Ok().content_type("text/html").body(view));
    } else {
        let first_deck_result = decks::table.first::<Deck>(&conn);
        if first_deck_result.is_ok() {
            let first_deck_id = first_deck_result.unwrap().id;
            let inserted_ctx = insert_to_ctx(&mut ctx, conn, true, first_deck_id, "");
            let view = tmpl
                .render("edit-deck.html", &inserted_ctx)
                .map_err(|e| error::ErrorInternalServerError(e))?;
            return Ok(HttpResponse::Ok().content_type("text/html").body(view));
        } else {
            let inserted_ctx = insert_to_ctx(&mut ctx, conn, false, -1, "");
            let view = tmpl
                .render("edit-deck.html", &inserted_ctx)
                .map_err(|e| error::ErrorInternalServerError(e))?;
            return Ok(HttpResponse::Ok().content_type("text/html").body(view));
        }
    }
}

struct CardIdAndNum {
    card_id: i32,
    num: i32,
}

async fn complete_deck_editing(
    pool: web::Data<r2d2::Pool<ConnectionManager<PgConnection>>>,
    tmpl: web::Data<tera::Tera>,
    mut payload: Multipart,
) -> Result<HttpResponse, Error> {
    let mut cards_id_and_num: Vec<CardIdAndNum> = Vec::new();
    let mut selected_deck_id: i32 = -1;
    while let Ok(Some(mut field)) = payload.try_next().await {
        let content_type = field.content_disposition().unwrap();
        let name = content_type.get_name().unwrap();
        if name == "selected_deck_id" {
            while let Some(chunk) = field.next().await {
                let data = chunk.unwrap();
                selected_deck_id = str::from_utf8(&data).unwrap().parse().unwrap();
            }
        } else {
            while let Some(chunk) = field.next().await {
                let card_id = name.parse().unwrap();
                let data = chunk.unwrap();
                let num = str::from_utf8(&data).unwrap().to_string().parse().unwrap();
                let card_id_and_num = CardIdAndNum {
                    card_id: card_id,
                    num: num,
                };
                cards_id_and_num.push(card_id_and_num);
            }
        }
    }
    let conn = pool.get().expect("couldn't get db connection from pool");
    for card_id_and_num in cards_id_and_num {
        let belonging = belongings::table
            .filter(
                belongings::deck_id
                    .eq(selected_deck_id)
                    .and(belongings::card_id.eq(card_id_and_num.card_id)),
            )
            .first::<Belonging>(&conn);
        if belonging.is_err() {
            let new_belonging = NewBelonging {
                deck_id: selected_deck_id,
                card_id: card_id_and_num.card_id,
                num: card_id_and_num.num,
            };
            diesel::insert_into(belongings::table)
                .values(&new_belonging)
                .execute(&conn)
                .unwrap();
        } else {
            if card_id_and_num.num <= 0 {
                diesel::delete(
                    belongings::table.filter(
                        belongings::deck_id
                            .eq(selected_deck_id)
                            .and(belongings::card_id.eq(card_id_and_num.card_id)),
                    ),
                )
                .execute(&conn)
                .unwrap();
            } else {
                diesel::update(
                    belongings::table.filter(
                        belongings::deck_id
                            .eq(selected_deck_id)
                            .and(belongings::card_id.eq(card_id_and_num.card_id)),
                    ),
                )
                .set(belongings::num.eq(card_id_and_num.num))
                .execute(&conn)
                .unwrap();
            }
        }
    }
    let mut ctx = tera::Context::new();
    let inserted_ctx = insert_to_ctx(&mut ctx, conn, true, selected_deck_id, "デッキ編集完了");
    let view = tmpl
        .render("edit-deck.html", &inserted_ctx)
        .map_err(|e| error::ErrorInternalServerError(e))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(view))
}

pub fn register(config: &mut web::ServiceConfig) {
    let templates = Tera::new("templates/**/*").unwrap();
    config
        .data(templates)
        .route("/edit-deck", web::get().to(view_edit_deck_screen))
        .route("/edit-deck/{deck_id}", web::get().to(view_edit_deck_screen))
        .route("/edit-deck/complete", web::post().to(complete_deck_editing));
}
