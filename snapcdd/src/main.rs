#![allow(clippy::unit_arg)]

use rocket::{get, put, routes, State};
use sled::Db;

#[get("/protocol_versions")]
fn protocol_versions() -> &'static str {
    "v1"
}

#[get("/v1/object/by-id/<id>")]
fn get_object(db: &State<Db>, id: String) -> Vec<u8> {
    db.get(id)
        .expect("failed to get data")
        .expect("data not found")
        .to_vec()
}

#[put("/v1/object/by-id/<id>", data = "<data>")]
fn put_object(db: &State<Db>, id: String, data: Vec<u8>) {
    db.insert(id, data).expect("failed to insert data");
}

#[tokio::main]
async fn main() {
    let db = sled::open("db").expect("failed to open database");

    rocket::build()
        .mount("/", routes![protocol_versions, get_object, put_object])
        .manage(db)
        .launch()
        .await
        .expect("failed to launch");
}
