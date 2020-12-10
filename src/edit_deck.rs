/**
 * デッキ編集処理
 * http://localhost:8080/edit-deck
 */
use crate::models::{Belonging, Card, Deck, NewBelonging, NewCard, NewDeck};
use crate::schema::{belongings, cards, decks};
use actix_files::Files;
use actix_multipart::Multipart;
use actix_web::{error, web, Error, HttpRequest, HttpResponse};
use diesel::pg::expression::dsl::any;
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use futures::{StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
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
            .load::<Card>(&conn)
            .expect("Error loading cards");
        let card_nums_in_selected_deck = Belonging::belonging_to(&selected_deck)
            .select(belongings::num)
            .load::<i32>(&conn)
            .expect("Error loading belongings");
        let mut cards_info_in_selected_deck = Vec::new();
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

async fn edit_deck(
    pool: web::Data<r2d2::Pool<ConnectionManager<PgConnection>>>,
    tmpl: web::Data<tera::Tera>,
    mut payload: Multipart,
) -> Result<HttpResponse, Error> {
    let mut action: String = String::from("");
    let mut card_ids: Vec<i32> = Vec::new();
    let mut deck_id: i32 = -1;
    let mut selected_deck_id: i32 = -1;
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
        } else if name == "deck_id" {
            while let Some(chunk) = field.next().await {
                let data = chunk.unwrap();
                deck_id = str::from_utf8(&data).unwrap().parse().unwrap();
            }
        } else if name == "selected_deck_id" {
            while let Some(chunk) = field.next().await {
                let data = chunk.unwrap();
                selected_deck_id = str::from_utf8(&data).unwrap().parse().unwrap();
            }
        }
    }
    let conn = pool.get().expect("couldn't get db connection from pool");
    let mut ctx = tera::Context::new();
    if action == "copy" {
        for card_id in card_ids {
            let new_belonging = NewBelonging {
                deck_id: deck_id,
                card_id: card_id,
                num: 1,
            };
            diesel::insert_into(belongings::table)
                .values(&new_belonging)
                .execute(&conn)
                .unwrap();
        }
    } else if action == "delete" {
        for card_id in card_ids {
            diesel::delete(
                belongings::table.filter(
                    belongings::deck_id
                        .eq(selected_deck_id)
                        .and(belongings::card_id.eq(card_id)),
                ),
            )
            .execute(&conn)
            .unwrap();
        }
    }
    let decks = decks::table
        .load::<Deck>(&conn)
        .expect("Error loading decks");
    if selected_deck_id == -1 {
        let cards = cards::table
            .load::<Card>(&conn)
            .expect("Error loading cards");
        ctx.insert("cards", &cards);
        ctx.insert("decks", &decks);
        ctx.insert("add_deck_confirm", &"".to_owned());
        ctx.insert("deck_name", &"すべてのカード".to_owned());
        ctx.insert("deck_id", &selected_deck_id.to_owned());
    } else {
        let selected_deck = decks::table
            .find(selected_deck_id)
            .first::<Deck>(&conn)
            .expect("Error loading deck");
        let card_ids_in_selected_deck =
            Belonging::belonging_to(&selected_deck).select(belongings::card_id);
        let cards = cards::table
            .filter(cards::id.eq(any(card_ids_in_selected_deck)))
            .load::<Card>(&conn)
            .expect("Error loading cards");
        ctx.insert("cards", &cards);
        ctx.insert("decks", &decks);
        ctx.insert("add_deck_confirm", &"".to_owned());
        ctx.insert("deck_name", &selected_deck.name.to_owned());
        ctx.insert("deck_id", &selected_deck_id.to_owned());
    }
    let view = tmpl
        .render("deck.html", &ctx)
        .map_err(|e| error::ErrorInternalServerError(e))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(view))
}

pub fn register(config: &mut web::ServiceConfig) {
    let templates = Tera::new("templates/**/*").unwrap();
    config
        .data(templates)
        .route("/edit-deck", web::get().to(view_edit_deck_screen))
        .route("/edit-deck/{deck_id}", web::get().to(view_edit_deck_screen));
}
