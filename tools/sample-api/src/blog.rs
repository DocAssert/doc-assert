use std::collections::HashMap;

use crate::domain::{BlogPost, BlogMap, AllBlogs};
use rocket::serde::json::{Json, Value, json};
use rocket::serde::uuid::Uuid;


#[post("/", format = "json", data = "<blog>")]
async fn new(blog: Json<BlogPost>, list: AllBlogs<'_>) -> Json<BlogPost> {
    let mut blogs = list.lock().await;

    let mut blog = blog.into_inner();
    blog.id = Some(Uuid::new_v4());
    blog.date_upd = Some(chrono::Utc::now());

    blogs.insert(blog.id.unwrap(), blog.clone());

    Json(blog)
}

#[put("/<id>", format = "json", data = "<blog>")]
async fn update(id: Uuid, blog: Json<BlogPost>, list: AllBlogs<'_>) -> Option<Json<BlogPost>> {
    let mut blogs = list.lock().await;
    if let Some(saved_blog) = blogs.get_mut(&id) {
        saved_blog.title = blog.title.clone();
        saved_blog.body = blog.body.clone();
        saved_blog.date_upd = Some(chrono::Utc::now());

        Some(Json(saved_blog.clone()))
    } else {
        None
    }
}

#[get("/<id>", format = "json")]
async fn get(id: Uuid, list: AllBlogs<'_>) -> Option<Json<BlogPost>> {
    let blogs = list.lock().await;
    blogs.get(&id).map(|blog| Json(blog.clone()))
}

#[delete("/<id>", format = "json")]
async fn delete(id: Uuid, list: AllBlogs<'_>) -> Option<Json<BlogPost>> {
    let mut blogs = list.lock().await;
    blogs.remove(&id).map(|blog| Json(blog))
}

#[catch(404)]
fn not_found() -> Value {
    json!({
        "status": "error",
        "reason": "Resource was not found."
    })
}

pub fn stage() -> rocket::fairing::AdHoc {
    rocket::fairing::AdHoc::on_ignite("blog", |rocket| async {
        rocket.mount("/blog", routes![new, update, get, delete])
            .register("/blog", catchers![not_found])
            .manage(BlogMap::new(HashMap::new()))
    })
}