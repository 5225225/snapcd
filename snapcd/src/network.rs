#[derive(Debug)]
pub struct Connection {
    pub url: String,
}

impl Connection {
    pub fn get(&self, key: crate::key::Key) -> crate::object::Object {
        let u = format!("{}/v1/object/by-id/{}", self.url, key.as_user_key());

        let result = ureq::get(&u).call().unwrap().into_reader();

        serde_cbor::from_reader(result).unwrap()
    }

    pub fn put(&self, key: crate::key::Key, data: &crate::object::Object) {
        let u = format!("{}/v1/object/by-id/{}", self.url, key.as_user_key());

        let body = serde_cbor::to_vec(&data).unwrap();
        dbg!(body.len());

        ureq::put(&u).send_bytes(&body).unwrap();
    }
}
