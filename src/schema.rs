table! {
    belongings (id) {
        id -> Int4,
        deck_id -> Int4,
        card_id -> Int4,
        num -> Int4,
    }
}

table! {
    cards (id) {
        id -> Int4,
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

joinable!(belongings -> cards (card_id));
joinable!(belongings -> decks (deck_id));

allow_tables_to_appear_in_same_query!(
    belongings,
    cards,
    decks,
);
