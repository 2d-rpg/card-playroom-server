use super::schema::cards;
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

#[derive(Queryable, Serialize)]
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
