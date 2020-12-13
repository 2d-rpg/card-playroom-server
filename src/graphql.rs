use std::sync::Arc;

use actix_web::{web, Error, HttpResponse};

use juniper::{graphql_value, http::GraphQLRequest, Executor, FieldError, FieldResult};
// use juniper_eager_loading::{prelude::*, EagerLoading, HasMany};
use juniper_from_schema::graphql_schema_from_file;

use diesel::prelude::*;

use itertools::Itertools;

use crate::{DbCon, DbPool};

use crate::models::{Belonging, Card, Deck, Room};
use crate::schema::{belongings, cards, decks, rooms};
graphql_schema_from_file!("src/schema.graphql");

pub struct Context {
    db_con: DbCon,
}
impl juniper::Context for Context {}

pub struct Query;
pub struct Mutation;

pub struct DeckWithCards {
    id: i32,
    name: String,
    card_ids: Vec<i32>,
}

impl QueryFields for Query {
    fn field_rooms(
        &self,
        executor: &Executor<'_, Context>,
        _trail: &QueryTrail<'_, Room, Walked>,
    ) -> FieldResult<Vec<Room>> {
        rooms::table
            .load::<crate::models::Room>(&executor.context().db_con)
            .and_then(|rooms| Ok(rooms.into_iter().map_into().collect()))
            .map_err(Into::into)
    }
    fn field_cards(
        &self,
        executor: &Executor<'_, Context>,
        _trail: &QueryTrail<'_, Card, Walked>,
    ) -> FieldResult<Vec<Card>> {
        cards::table
            .load::<Card>(&executor.context().db_con)
            .and_then(|cards| Ok(cards.into_iter().map_into().collect()))
            .map_err(Into::into)
    }
    fn field_decks_with_cards(
        &self,
        executor: &Executor<'_, Context>,
        _trail: &QueryTrail<'_, DeckWithCards, Walked>,
    ) -> FieldResult<Vec<DeckWithCards>> {
        let decks_result = decks::table
            .order_by(decks::id.asc())
            .load::<Deck>(&executor.context().db_con);
        if decks_result.is_err() {
            let error_message = FieldError::new(
                "Could not load deck data",
                graphql_value!({ "internal_error": "Database error" }),
            );
            Err(error_message)
        } else {
            let decks = decks_result.unwrap();
            let deck_with_cards: Vec<DeckWithCards> = decks
                .into_iter()
                .map(|deck| {
                    let card_ids_and_nums = Belonging::belonging_to(&deck)
                        .select((belongings::card_id, belongings::num))
                        .load::<(i32, i32)>(&executor.context().db_con)
                        .expect("Error loading belongings");
                    let card_ids: Vec<i32> = card_ids_and_nums
                        .into_iter()
                        .map(|card_id_and_nums| {
                            vec![card_id_and_nums.0; card_id_and_nums.1 as usize]
                        })
                        .flatten()
                        .collect();
                    return DeckWithCards {
                        id: deck.id,
                        name: deck.name,
                        card_ids: card_ids,
                    };
                })
                .collect();
            Ok(deck_with_cards)
        }
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
        let target = rooms::table.find(room_id);
        let mut players: Vec<String> = target
            .first::<crate::models::Room>(&executor.context().db_con)
            .unwrap()
            .players; // get players in table

        players.push(player); // add new player

        diesel::update(target)
            .set(rooms::dsl::players.eq(players)) // set updated players in table
            .get_result::<crate::models::Room>(&executor.context().db_con)
            .map(Into::into)
            .map_err(Into::into)
    }

    fn field_remove_room(
        &self,
        executor: &Executor<'_, Context>,
        _trail: &QueryTrail<'_, Room, Walked>,
        room_id: i32,
    ) -> FieldResult<Room> {
        let target = rooms::table.find(room_id);

        diesel::delete(target)
            .get_result::<crate::models::Room>(&executor.context().db_con)
            .map(Into::into)
            .map_err(Into::into)
    }
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

impl CardFields for Card {
    fn field_id(&self, _: &Executor<'_, Context>) -> FieldResult<juniper::ID> {
        Ok(juniper::ID::new(self.id.to_string()))
    }

    fn field_face(&self, _: &Executor<'_, Context>) -> FieldResult<&String> {
        Ok(&self.face)
    }

    fn field_back(&self, _: &Executor<'_, Context>) -> FieldResult<&String> {
        Ok(&self.back)
    }
}

impl DeckWithCardsFields for DeckWithCards {
    fn field_id(&self, _: &Executor<'_, Context>) -> FieldResult<juniper::ID> {
        Ok(juniper::ID::new(self.id.to_string()))
    }

    fn field_name(&self, _: &Executor<'_, Context>) -> FieldResult<&String> {
        Ok(&self.name)
    }

    fn field_card_ids(&self, _: &Executor<'_, Context>) -> FieldResult<&Vec<i32>> {
        Ok(&self.card_ids)
    }
}

async fn graphql(
    schema: web::Data<Arc<Schema>>,
    data: web::Json<GraphQLRequest>,
    db_pool: web::Data<DbPool>,
) -> Result<HttpResponse, Error> {
    let ctx = Context {
        db_con: db_pool.get().unwrap(),
    };

    let json_result = web::block(move || {
        let res = data.execute(&schema, &ctx);
        Ok::<_, serde_json::error::Error>(serde_json::to_string(&res)?)
    })
    .await?;

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .body(json_result))
}

pub fn register(config: &mut web::ServiceConfig) {
    let schema = std::sync::Arc::new(Schema::new(Query, Mutation));

    config
        .data(schema)
        .route("/graphql", web::post().to(graphql));
}
