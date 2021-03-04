#![allow(clippy::unit_arg)]

use rocket::{get, put, routes};

#[get("/protocol_versions")]
fn protocol_versions() -> &'static str {
    "v1"
}

#[get("/v1/object/by-id/<id>")]
fn get_object(id: String) {
    dbg!(&id);
}

#[put("/v1/object/by-id/<id>", data = "<data>")]
fn put_object(id: String, data: Vec<u8>) {
    dbg!(&id, &data);
}

#[tokio::main]
async fn main() {
    rocket::ignite()
        .mount("/", routes![protocol_versions, get_object, put_object])
        .launch()
        .await
        .expect("failed to launch");
}
