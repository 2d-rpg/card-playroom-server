use super::schema::rooms;

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

#[derive(Queryable)]
pub struct Card {
    pub id: i32,
    pub face: String,
    pub back: String,
}

#[derive(Queryable)]
pub struct Deck {
    pub id: i32,
    pub face: String,
    pub back: String,
}
