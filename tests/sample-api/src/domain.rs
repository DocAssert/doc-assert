// Copyright 2024 The DocAssert Authors
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::collections::HashMap;

use chrono::serde::ts_seconds_option;
use chrono::{DateTime, Utc};
use rocket::serde::uuid::Uuid;
use rocket::tokio::sync::Mutex;
use rocket::State;
use serde::{Deserialize, Serialize};

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

pub type BlogMap = Mutex<HashMap<Uuid, BlogPost>>;
pub type AllBlogs<'r> = &'r State<BlogMap>;
