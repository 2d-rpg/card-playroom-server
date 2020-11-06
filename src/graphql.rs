use std::convert::From;
use std::sync::Arc;

use actix_web::{web, Error, HttpResponse};

use juniper::http::playground::playground_source;
use juniper::{http::GraphQLRequest, Executor, FieldResult};
// use juniper_eager_loading::{prelude::*, EagerLoading, HasMany};
use juniper_from_schema::graphql_schema_from_file;

use diesel::prelude::*;

use itertools::Itertools;

use crate::{DbCon, DbPool};

graphql_schema_from_file!("src/schema.graphql");

pub struct Context {
    db_con: DbCon,
}
impl juniper::Context for Context {}

pub struct Query;
pub struct Mutation;

impl QueryFields for Query {
    fn field_rooms(
        &self,
        executor: &Executor<'_, Context>,
        _trail: &QueryTrail<'_, Room, Walked>,
    ) -> FieldResult<Vec<Room>> {
        use crate::schema::rooms;

        rooms::table
            .load::<crate::models::Room>(&executor.context().db_con)
            .and_then(|rooms| Ok(rooms.into_iter().map_into().collect()))
            .map_err(Into::into)
    }
}

impl MutationFields for Mutation {
    fn field_create_room(
        &self,
        executor: &Executor<'_, Context>,
        _trail: &QueryTrail<'_, Room, Walked>,
        name: String,
        player: String,
    ) -> FieldResult<Room> {
        use crate::schema::rooms;

        let new_room = crate::models::NewRoom {
            name: name,
            players: vec![player],
        };

        diesel::insert_into(rooms::table)
            .values(&new_room)
            .get_result::<crate::models::Room>(&executor.context().db_con)
            .map(Into::into)
            .map_err(Into::into)
    }

    fn field_enter_room(
        &self,
        executor: &Executor<'_, Context>,
        _trail: &QueryTrail<'_, Room, Walked>,
        player: String,
        room_id: i32,
    ) -> FieldResult<Room> {
        use crate::schema::rooms;

        let target = rooms::table.find(room_id);
        let last_player: String = target
            .first::<crate::models::Room>(&executor.context().db_con)
            .unwrap()
            .players
            .get(0)
            .expect("There is no player!")
            .to_string();

        diesel::update(target)
            .set(rooms::dsl::players.eq(vec![last_player, player]))
            .get_result::<crate::models::Room>(&executor.context().db_con)
            .map(Into::into)
            .map_err(Into::into)

        // diesel::insert_into(rooms::table)
        //     .values(&new_room)
        //     .get_result::<crate::models::Room>(&executor.context().db_con)
        //     .map(Into::into)
        //     .map_err(Into::into)
    }
}

pub struct Room {
    id: i32,
    name: String,
    players: Vec<String>,
}

impl RoomFields for Room {
    fn field_id(&self, _: &Executor<'_, Context>) -> FieldResult<juniper::ID> {
        Ok(juniper::ID::new(self.id.to_string()))
    }

    fn field_name(&self, _: &Executor<'_, Context>) -> FieldResult<&String> {
        Ok(&self.name)
    }

    fn field_players(&self, _: &Executor<'_, Context>) -> FieldResult<&Vec<String>> {
        Ok(&self.players)
    }
}

impl From<crate::models::Room> for Room {
    fn from(room: crate::models::Room) -> Self {
        Self {
            id: room.id,
            name: room.name,
            players: room.players,
        }
    }
}

fn playground() -> HttpResponse {
    let html = playground_source("");
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

async fn graphql(
    schema: web::Data<Arc<Schema>>,
    data: web::Json<GraphQLRequest>,
    db_pool: web::Data<DbPool>,
) -> Result<HttpResponse, Error> {
    let ctx = Context {
        db_con: db_pool.get().unwrap(),
    };

    let room = web::block(move || {
        let res = data.execute(&schema, &ctx);
        Ok::<_, serde_json::error::Error>(serde_json::to_string(&res)?)
    })
    .await?;

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .body(room))
}

pub fn register(config: &mut web::ServiceConfig) {
    let schema = std::sync::Arc::new(Schema::new(Query, Mutation));

    config
        .data(schema)
        .route("/", web::post().to(graphql))
        .route("/", web::get().to(playground));
}
