use super::schema::rooms;

#[derive(Queryable)]
pub struct Room {
    pub id: i32,
    pub name: String,
    pub playersID: Vec<String>,
}

#[derive(Insertable)]
#[table_name = "rooms"]
pub struct NewRoom {
    pub name: String,
    pub playerID: String,
}
