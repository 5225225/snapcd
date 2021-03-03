#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use] extern crate rocket;

#[get("/protocol_versions")]
fn protocol_versions() -> &'static str {
    "v1"
}

#[get("/v1/object/by-id/<id>")]
fn get_object(id: String) {
}

#[put("/v1/object/by-id/<id>", data="<data>")]
fn put_object(id: String, data: Vec<u8>) {
}

fn main() {
    rocket::ignite().mount("/", routes![protocol_versions, get_object, put_object]).launch();
}
