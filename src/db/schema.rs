// @generated automatically by Diesel CLI.

diesel::table! {
    app (id) {
        id -> Integer,
        app_name -> Text,
        app_path -> Nullable<Text>,
    }
}

diesel::table! {
    app_usage (id) {
        id -> Integer,
        session_id -> Text,
        app_name -> Text,
        screen_title_name -> Text,
        duration_in_seconds -> Integer,
        is_active -> Integer,
        last_active_time -> Nullable<Timestamp>,
        date -> Date,
        time_stamp -> Timestamp,
        created_at -> Nullable<Timestamp>,
        updated_at -> Nullable<Timestamp>,
    }
}

diesel::allow_tables_to_appear_in_same_query!(app, app_usage,);
