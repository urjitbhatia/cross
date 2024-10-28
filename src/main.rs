mod scrapers;
mod taskgraph;
mod textparsers;
mod workers;

extern crate crossbeam_channel;
extern crate threadpool;

use crossbeam_deque::Worker;

use std::{fs, num::NonZeroUsize, thread};

fn main() {
    // this is an example running on the notion api
    let starting_page_id = "12345".to_string(); // starting page id/name
    let starting_page_url = URL_TMPL!(starting_page_id);
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
    taskgraph::walk(
        initial,
        10,
        |(page_url, page_id): (String, String),
         w: &Worker<(String, String)>|
         -> Result<Option<String>, std::fmt::Error> {
            let client = workers::get_http_agent();
            let doc = scrapers::read_page(client, page_url, page_id.clone(), w)
                .ok_or(std::fmt::Error)?
                .join("\n");
            let filename = format!("./doc_{page_id}.md");
            match fs::write(&filename, doc).map_err(|_| std::fmt::Error) {
                Err(_) => Result::Err(std::fmt::Error),
                Ok(_) => Result::Ok(Some(filename)),
            }
        },
    );

    println!("done with all of the work");
}
