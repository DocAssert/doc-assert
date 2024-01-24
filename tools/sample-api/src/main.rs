#[macro_use] extern crate rocket;

mod domain;
mod blog;
mod comment;

#[launch]
fn rocket() -> _ {
    rocket::build()
    .attach(blog::stage())
    .attach(comment::stage())
}
