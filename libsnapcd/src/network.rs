use std::io::Read;

#[derive(Debug)]
pub struct Connection {
    pub url: String,
}

impl Connection {
    pub fn get(&self, key: crate::key::Key) -> crate::object::Object {
        let u = format!("{}/v1/object/by-id/{}", self.url, key.as_user_key());

        let mut out = Vec::new();

        // TODO: DOS ATTACK POSSIBLE HERE
        let mut reader = ureq::get(&u).call().unwrap().into_reader();
        reader.read_to_end(&mut out);

        minicbor::decode(&out).unwrap()
    }

    pub fn put(&self, key: crate::key::Key, data: &crate::object::Object) {
        let u = format!("{}/v1/object/by-id/{}", self.url, key.as_user_key());

        let body = minicbor::to_vec(&data).unwrap();

        ureq::put(&u).send_bytes(&body).unwrap();
    }
}
