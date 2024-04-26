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

#[macro_use]
extern crate rocket;

use domain::FaultCounter;
use rocket::serde::json::{json, Json, Value};

mod admin;
mod blog;
mod comment;
mod domain;

#[get("/", format = "json")]
async fn status(counter: domain::FaultState<'_>) -> Json<Value> {
    let mut counter = counter.lock().await;
    Json(json!({
        "faulty": counter.is_faulty(),
    }))
}


#[launch]
fn rocket() -> _ {
    rocket::build()
        .attach(blog::stage())
        .attach(comment::stage())
        .attach(admin::stage())
        .mount("/status", routes![status])
        .manage(domain::FaultMtx::new(FaultCounter::new(5)))
}
