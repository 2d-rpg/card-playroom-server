use super::schema::cards;
use super::schema::decks;
use super::schema::rooms;
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

#[derive(Identifiable, Queryable, Associations, Serialize)]
#[belongs_to(Deck)]
pub struct Card {
    pub id: i32,
    pub deck_id: Option<i32>,
    pub face: String,
    pub back: String,
}

#[derive(Insertable)]
#[table_name = "cards"]
pub struct NewCard {
    pub deck_id: Option<i32>,
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
