// @generated automatically by Diesel CLI.

diesel::table! {
    events (id) {
        id -> Int4,
        time -> Timestamptz,
        process -> Int4,
        duration -> Interval,
    }
}

diesel::table! {
    processes (id) {
        id -> Int4,
        executable -> Varchar,
        name -> Nullable<Varchar>,
        export -> Bool,
    }
}

diesel::joinable!(events -> processes (process));

diesel::allow_tables_to_appear_in_same_query!(events, processes,);
