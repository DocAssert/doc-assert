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

use crate::domain::AllBlogs;
use rocket::serde::json::{json, Json, Value};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct CensorshipInput {
    pattern: String,
    replacement: String,
}

#[post("/censor", format = "json", data = "<censor>")]
async fn censor(censor: Json<CensorshipInput>, state: AllBlogs<'_>) -> Value {
    let mut blogs = state.lock().await;

    blogs.iter_mut().for_each(|(_, blog)| {
        blog.body = blog.body.replace(&censor.pattern, &censor.replacement);
        blog.title = blog.title.replace(&censor.pattern, &censor.replacement);

        if let Some(comments) = &mut blog.comments {
            comments.iter_mut().for_each(|comment| {
                comment.text = comment.text.replace(&censor.pattern, &censor.replacement);
                comment.user = comment.user.replace(&censor.pattern, &censor.replacement);

                if let Some(tags) = &mut comment.tags {
                    tags.iter_mut().for_each(|tag| {
                        tag.value = tag.value.replace(&censor.pattern, &censor.replacement);
                    });
                }
            });
        }
    });

    json!({
        "status": "ok",
    })
}

pub fn stage() -> rocket::fairing::AdHoc {
    rocket::fairing::AdHoc::on_ignite("admin", |rocket| async {
        rocket.mount("/admin", routes![censor])
    })
}
