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
    cards: std::vec::Vec<Card>,
    is_deck_selected: bool,
    decks: std::vec::Vec<Deck>,
    optional_selected_deck: Option<Deck>,
    edit_deck_confirm: &str,
    cards_in_deck: std::vec::Vec<CardInfoInDeck>,
) -> tera::Context {
    ctx.insert("cards", &cards);
    ctx.insert("is_deck_selected", &is_deck_selected);
    ctx.insert("decks", &decks);
    if optional_selected_deck.is_some() {
        let selected_deck = optional_selected_deck.unwrap();
        ctx.insert("selected_deck_id", &selected_deck.id);
        ctx.insert("selected_deck_name", &selected_deck.name);
    } else {
        ctx.insert("selected_deck_id", &"");
        ctx.insert("selected_deck_name", &"");
    }
    ctx.insert("edit_deck_confirm", &edit_deck_confirm);
    ctx.insert("cards_in_deck", &cards_in_deck);
    return ctx.clone();
}

async fn view_edit_deck_screen(
    req: HttpRequest,
    pool: web::Data<r2d2::Pool<ConnectionManager<PgConnection>>>,
    tmpl: web::Data<tera::Tera>,
) -> Result<HttpResponse, Error> {
    let optional_deck_id = req.match_info().get("deck_id");
    let conn = pool.get().expect("couldn't get db connection from pool");
    let mut ctx = tera::Context::new();
    let cards = cards::table
        .load::<Card>(&conn)
        .expect("Error loading cards");
    let decks = decks::table
        .load::<Deck>(&conn)
        .expect("Error loading decks");
    if optional_deck_id.is_none() {
        let inserted_ctx = insert_to_ctx(&mut ctx, cards, false, decks, None, "", Vec::new());
        let view = tmpl
            .render("edit-deck.html", &inserted_ctx)
            .map_err(|e| error::ErrorInternalServerError(e))?;
        return Ok(HttpResponse::Ok().content_type("text/html").body(view));
    } else {
        let deck_id: i32 = optional_deck_id.unwrap().parse().unwrap();
        let selected_deck = decks::table
            .find(deck_id)
            .first::<Deck>(&conn)
            .expect("Error loading deck");
        let card_ids_in_selected_deck =
            Belonging::belonging_to(&selected_deck).select(belongings::card_id);
        let mut cards_in_selected_deck = cards::table
            .filter(cards::id.eq(any(card_ids_in_selected_deck)))
            .order(cards::id.asc())
            .load::<Card>(&conn)
            .expect("Error loading cards");
        let mut card_nums_in_selected_deck = Belonging::belonging_to(&selected_deck)
            .order(belongings::card_id.asc())
            .select(belongings::num)
            .load::<i32>(&conn)
            .expect("Error loading belongings");
        let mut cards_info_in_selected_deck = Vec::new();
        card_nums_in_selected_deck.reverse();
        for i in 0..cards_in_selected_deck.len() {
            let card_in_selected_deck = cards_in_selected_deck.pop().unwrap();
            let card_info_in_selected_deck = CardInfoInDeck {
                card_id: card_in_selected_deck.id,
                face: card_in_selected_deck.face,
                back: card_in_selected_deck.back,
                num: card_nums_in_selected_deck[i],
            };
            cards_info_in_selected_deck.push(card_info_in_selected_deck);
        }
        cards_info_in_selected_deck.reverse();
        let inserted_ctx = insert_to_ctx(
            &mut ctx,
            cards,
            true,
            decks,
            Some(selected_deck),
            "",
            cards_info_in_selected_deck,
        );
        let view = tmpl
            .render("edit-deck.html", &inserted_ctx)
            .map_err(|e| error::ErrorInternalServerError(e))?;
        return Ok(HttpResponse::Ok().content_type("text/html").body(view));
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
    let cards = cards::table
        .load::<Card>(&conn)
        .expect("Error loading cards");
    let decks = decks::table
        .load::<Deck>(&conn)
        .expect("Error loading decks");
    let selected_deck = decks::table
        .find(selected_deck_id)
        .first::<Deck>(&conn)
        .expect("Error loading deck");
    let card_ids_in_selected_deck =
        Belonging::belonging_to(&selected_deck).select(belongings::card_id);
    let mut cards_in_selected_deck = cards::table
        .filter(cards::id.eq(any(card_ids_in_selected_deck)))
        .order(cards::id.asc())
        .load::<Card>(&conn)
        .expect("Error loading cards");
    let mut card_nums_in_selected_deck = Belonging::belonging_to(&selected_deck)
        .order(belongings::card_id.asc())
        .select(belongings::num)
        .load::<i32>(&conn)
        .expect("Error loading belongings");
    let mut cards_info_in_selected_deck = Vec::new();
    card_nums_in_selected_deck.reverse();
    for i in 0..cards_in_selected_deck.len() {
        let card_in_selected_deck = cards_in_selected_deck.pop().unwrap();
        let card_info_in_selected_deck = CardInfoInDeck {
            card_id: card_in_selected_deck.id,
            face: card_in_selected_deck.face,
            back: card_in_selected_deck.back,
            num: card_nums_in_selected_deck[i],
        };
        cards_info_in_selected_deck.push(card_info_in_selected_deck);
    }
    cards_info_in_selected_deck.reverse();
    let inserted_ctx = insert_to_ctx(
        &mut ctx,
        cards,
        true,
        decks,
        Some(selected_deck),
        "デッキ編集完了",
        cards_info_in_selected_deck,
    );
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
