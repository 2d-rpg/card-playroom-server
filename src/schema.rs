use juniper::FieldResult;
use juniper::RootNode;

use juniper::{GraphQLInputObject, GraphQLObject};

// table! {
//     rooms (id) {
//         id -> Varchar,
//         name -> Varchar,
//     }
// }

#[derive(GraphQLObject, Debug)]
#[graphql(description = "A game room")]
struct Room {
    name: String,
    id: String,
    player: Vec<String>,
}

/// 既存ルームが変更される場合に変更される対象データ．
/// 参加プレイヤーが増えるだけなので，playerのみ．部屋名変更などはない．
#[derive(GraphQLInputObject, Debug)]
#[graphql(description = "A game room")]
struct NewRoom {
    player: Vec<String>,
}

pub struct Query;

#[juniper::object]
impl Query {
    fn room(id: String) -> FieldResult<Room> {
        Ok(Room {
            name: "room-1".to_owned(),
            id: "1234".to_owned(),
            player: vec!["1234".to_owned()],
        })
    }
}

pub struct Mutation;

#[juniper::object]
impl Mutation {
    fn create_room(new_room: NewRoom) -> FieldResult<Room> {
        // diesel::insert_into()
        Ok(Room {
            name: "room-1".to_owned(),
            id: "1234".to_owned(),
            player: new_room.player,
        })
    }
}

pub type Schema = RootNode<'static, Query, Mutation>;

pub fn create_schema() -> Schema {
    Schema::new(Query {}, Mutation {})
}
