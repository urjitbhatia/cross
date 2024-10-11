mod http_worker;
mod graph_walker;

extern crate crossbeam_channel;
extern crate threadpool;

use std::{convert::identity, env, fs, num::NonZeroUsize, thread, time::Duration};

use crossbeam_deque::Worker;
use reqwest::blocking::Response;
use reqwest::{self, header};
use serde_json::Value;
use ureq::{Agent, Error};


fn create_http_client() -> reqwest::blocking::Client {
    let key = "AUTH_TOKEN";
    let token = match env::var(key) {
        Ok(token) => token,
        Err(e) => {
            println!("couldn't find env var {key}: {e}");
            panic!("{e}");
        }
    };
    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        header::HeaderValue::from_str(format!("Bearer {token}").as_str()).unwrap(),
    );
    headers.insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("application/json"),
    );

    reqwest::blocking::Client::builder()
        .default_headers(headers)
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap()
}

fn main() {
    // this is an example running on the notion api
    let starting_page_id = "12345".to_string(); // starting page id/name
    let starting_page_url = BLOCK_CHILD_URL_TMPL!(starting_page_id);
    let initial = vec![(starting_page_url, starting_page_id)]; // worker process expects a tuple of (url, resource_id)

    // create a number of workers
    let num_workers: usize = match thread::available_parallelism() {
        // leave 2 for the OS
        Ok(t) => <NonZeroUsize as Into<usize>>::into(t) - 2,
        Err(_) => 4,
    };

    println!("Running with {num_workers} workers");

    // walk the graph in parallel - as soon as new links/jobs are created, other workers jump in
    // this worker function parses notion pages and saves them as markdown files
    graph_walker::walk(
        initial,
        10,
        |(page_url, page_id): (String, String), w: &Worker<(String, String)>| -> Option<()> {
            // let client = create_http_client();
            let client = http::get_http_agent();
            let doc = read_page(client, page_url, page_id.clone(), w)?.join("\n");
            let filename = format!("./doc_{page_id}.md");
            match fs::write(&filename, doc) {
                Ok(_) => println!("wrote doc: {}", filename),
                Err(e) => println!("Unable to write file. Error: {}", e),
            }

            None
        },
    );

    println!("done with all of the work");
}
