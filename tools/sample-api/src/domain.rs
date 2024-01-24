use std::collections::HashMap;

use rocket::tokio::sync::Mutex;
use rocket::State;
use serde::{Serialize, Deserialize};
use rocket::serde::uuid::Uuid;
use chrono::{DateTime, Utc};
use chrono::serde::ts_seconds_option;


#[derive(Serialize, Deserialize, Clone)]
pub struct BlogPost {
    pub id: Option<Uuid>,
    pub title: String,
    pub body: String,
    
    #[serde(default)]
    #[serde(with = "ts_seconds_option")]
    pub date_upd: Option<DateTime<Utc>>,
    pub comments: Option<Vec<Comment>>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Comment {
    pub id: Option<Uuid>,
    pub user: String,
    pub text: String,

    #[serde(default)]
    #[serde(with = "ts_seconds_option")]
    pub date_upd: Option<DateTime<Utc>>,
    pub tags: Option<Vec<Tag>>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Tag {
    pub id: Option<Uuid>,
    pub value: String,
    
    #[serde(default)]
    #[serde(with = "ts_seconds_option")]
    pub date_add: Option<DateTime<Utc>>,
}

pub type BlogMap= Mutex<HashMap<Uuid, BlogPost>>;
pub type AllBlogs<'r> = &'r State<BlogMap>;