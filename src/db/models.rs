
use chrono::{NaiveDate, NaiveDateTime};
use diesel::prelude::*;
use serde::Deserialize;
use uuid::Uuid;


use crate::db::schema::{app, app_usage};

#[derive(Queryable, Identifiable, Selectable, Debug, PartialEq, Default, Eq, Hash)]
#[diesel(table_name = app)]
pub struct App {
    pub id: Uuid,
    pub app_name: String,
    pub app_path: Option<String>,
}

#[derive(Insertable, Deserialize)]
#[diesel(table_name = app)]
pub struct AddApp {
    pub id: Uuid,
    pub app_name: String,
    pub app_path: Option<String>,
}

#[derive(Queryable, Identifiable, Selectable, Associations, Debug, PartialEq)]
#[diesel(belongs_to(App, foreign_key = app_name))]
#[diesel(table_name = app_usage)]
pub struct AppUsage {
    pub id: Uuid,
    pub app_name: String,
    pub screen_title_name: Option<String>,
    pub duration_in_seconds: i32,
    pub date: NaiveDate,
    pub time_stamp: NaiveDateTime,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable, Deserialize)]
#[diesel(belongs_to(App, foreign_key = app_name))]
#[diesel(table_name = app_usage)]
pub struct NewAppUsage {
    pub id: Uuid,
    pub app_name: String,
    pub screen_title_name: Option<String>,
    pub duration_in_seconds: i32,
}