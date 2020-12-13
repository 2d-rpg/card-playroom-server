use super::schema::{belongings, cards, decks, rooms};
use serde::Serialize;

#[derive(Queryable)]
pub struct Room {
    pub id: i32,
    pub name: String,
    pub players: Vec<String>,
}

#[derive(Insertable)]
#[table_name = "rooms"]
pub struct NewRoom {
    pub name: String,
    pub players: Vec<String>,
}

#[derive(Identifiable, Queryable, Serialize)]
pub struct Card {
    pub id: i32,
    pub face: String,
    pub back: String,
}

#[derive(Insertable)]
#[table_name = "cards"]
pub struct NewCard {
    pub face: String,
    pub back: String,
}

#[derive(Identifiable, Queryable, Serialize)]
pub struct Deck {
    pub id: i32,
    pub name: String,
}

#[derive(Insertable)]
#[table_name = "decks"]
pub struct NewDeck {
    pub name: String,
}

#[derive(Identifiable, Queryable, Serialize, Associations)]
#[belongs_to(Deck)]
#[belongs_to(Card)]
pub struct Belonging {
    pub id: i32,
    pub deck_id: i32,
    pub card_id: i32,
    pub num: i32,
}

#[derive(Insertable)]
#[table_name = "belongings"]
pub struct NewBelonging {
    pub deck_id: i32,
    pub card_id: i32,
    pub num: i32,
}
