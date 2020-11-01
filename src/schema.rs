table! {
    rooms (id) {
        id -> Int4,
        name -> Varchar,
        players -> Array<Varchar>,
    }
}

table! {
    users (id) {
        id -> Int4,
        name -> Varchar,
    }
}

allow_tables_to_appear_in_same_query!(rooms, users,);
