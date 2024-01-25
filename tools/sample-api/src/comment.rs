use rocket::serde::json::Json;
use crate::domain::{BlogPost, AllBlogs, Comment};
use rocket::serde::uuid::Uuid;


#[post("/<blog_id>/comment", format = "json", data = "<comment>")]
async fn create(blog_id: Uuid, comment: Json<Comment>, state: AllBlogs<'_>) -> Option<Json<BlogPost>> {
    let mut blogs = state.lock().await;
    if let Some(saved_blog) = blogs.get_mut(&blog_id) {
        let mut comment = comment.into_inner();
        comment.id = Some(Uuid::new_v4());
        comment.date_upd = Some(chrono::Utc::now());
        if let Some(tags) = &mut comment.tags {
            for tag in tags {
                tag.id = Some(Uuid::new_v4());
                tag.date_add = Some(chrono::Utc::now());
            }
        }

        saved_blog.comments = match &saved_blog.comments {
            Some(comments) => {
                let mut comments = comments.clone();
                comments.push(comment.clone());
                Some(comments)
            },
            None => Some(vec![comment.clone()])
        };

        Some(Json(saved_blog.clone()))
    } else {
        None
    }
}

#[put("/<blog_id>/comment/<comment_id>", format = "json", data = "<comment>")]
async fn update(blog_id: Uuid, comment_id: Uuid, comment: Json<Comment>, state: AllBlogs<'_>) -> Option<Json<BlogPost>> {
    let mut blogs = state.lock().await;
    if let Some(saved_blog) = blogs.get_mut(&blog_id) {
        if let Some(comments) = &mut saved_blog.comments {
            if let Some(saved_comment) = comments.iter_mut().find(|c| c.id == Some(comment_id)) {
                saved_comment.user = comment.user.clone();
                saved_comment.text = comment.text.clone();
                saved_comment.date_upd = Some(chrono::Utc::now());

                return Some(Json(saved_blog.clone()));
            }
        }
    }

    None
}

#[get("/<blog_id>/comment/<comment_id>", format = "json")]
async fn get(blog_id: Uuid, comment_id: Uuid, state: AllBlogs<'_>) -> Option<Json<Comment>> {
    let blogs = state.lock().await;
    if let Some(saved_blog) = blogs.get(&blog_id) {
        if let Some(comments) = &saved_blog.comments {
            if let Some(saved_comment) = comments.iter().find(|c| c.id == Some(comment_id)) {
                return Some(Json(saved_comment.clone()));
            }
        }
    }

    None
}

#[delete("/<blog_id>/comment/<comment_id>", format = "json")]
async fn delete(blog_id: Uuid, comment_id: Uuid, state: AllBlogs<'_>) -> Option<Json<BlogPost>> {
    let mut blogs = state.lock().await;
    if let Some(saved_blog) = blogs.get_mut(&blog_id) {
        if let Some(comments) = &mut saved_blog.comments {
            if let Some(index) = comments.iter().position(|c| c.id == Some(comment_id)) {
                comments.remove(index);
                return Some(Json(saved_blog.clone()));
            }
        }
    }

    None
}

pub fn stage() -> rocket::fairing::AdHoc {
    rocket::fairing::AdHoc::on_ignite("comments", |rocket| async {
        rocket.mount("/blog", routes![create, update, get, delete])
    })
}