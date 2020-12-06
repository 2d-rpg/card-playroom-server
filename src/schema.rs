table! {
    cards (id) {
        id -> Int4,
        deck_id -> Int4,
        face -> Varchar,
        back -> Varchar,
    }
}

table! {
    decks (id) {
        id -> Int4,
        name -> Varchar,
    }
}

table! {
    rooms (id) {
        id -> Int4,
        name -> Varchar,
        players -> Array<Text>,
    }
}

allow_tables_to_appear_in_same_query!(
    cards,
    decks,
    rooms,
);
