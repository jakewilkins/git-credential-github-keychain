
extern crate url;

use url::Url;

use tiny_http::{Server, Response};
use std::sync::mpsc;
use std::thread;
use std::collections::HashMap;

pub fn start() -> Option<String> {
    let mut code: Option<String> = None;
    let count = 0;
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let server = Server::http("127.0.0.1:8075").unwrap();

        for request in server.incoming_requests() {
            // println!("received request! method: {:?}, url: {:?}, headers: {:?}",
            //     request.method(),
            //     request.url(),
            //     request.headers()
            // );

            let url_string = format!("http://127.0.0.1:8075{}", request.url());
            let url = Url::parse(url_string.as_str()).expect("Could not parse a URL");
            let hash_query: HashMap<_, _> = url.query_pairs().into_owned().collect();

            if hash_query.contains_key("code") {
                tx.send(String::from(hash_query["code"].clone())).unwrap();
            };

            let response = Response::from_string("hello world");
            match request.respond(response) {
                _ => {},
            };
        }
    });

    loop {
        match rx.try_recv() {
            Ok(value) => {
                code = Some(value);
                break
            },
            _ => {}
        }

        if count > 60 {
            break
        }
    };

    code
}
