#[macro_use] extern crate rocket;

mod domain;
mod blog;
mod comment;
mod admin;

#[launch]
fn rocket() -> _ {
    rocket::build()
    .attach(blog::stage())
    .attach(comment::stage())
    .attach(admin::stage())
}
