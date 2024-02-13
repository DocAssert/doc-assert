use serde::{Deserialize, Serialize};
use rocket::serde::json::{json, Json, Value};
use crate::domain::AllBlogs;

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