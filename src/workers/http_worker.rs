// This is an http worker that can be used with the graph_walker for example to scrape the web or a list of urls etc
use std::time::Duration;
use std::{env, thread};
use ureq::Error::Status;
use ureq::{Error, MiddlewareNext, Request, Response};

fn headers_middleware() -> impl ureq::Middleware {
    let key = "NOTION_TOKEN";
    let token = match env::var(key) {
        Ok(token) => token,
        Err(e) => {
            println!("couldn't find env var {key}: {e}");
            panic!("{e}");
        }
    };
    move |req: Request, next: MiddlewareNext| -> Result<Response, Error> {
        next.handle(
            req.set("authorization", format!("Bearer {token}").as_str())
                .set("content-type", "application/json")
                .set("Notion-Version", "2022-06-28"),
        )
    }
}

pub fn get_with_retry(agent: ureq::Agent, url: &str) -> Result<Response, Error> {
    for _ in 1..4 {
        match agent.get(url).call() {
            Err(Status(503, r)) | Err(Status(429, r)) => {
                let retry: Option<u64> = r.header("retry-after").and_then(|h| h.parse().ok());
                let retry = retry.unwrap_or(5);
                println!("{} for {}, retry in {}", r.status(), r.get_url(), retry);
                thread::sleep(Duration::from_secs(retry));
            }
            result => return result,
        };
    }
    // Ran out of retries; try one last time and return whatever result we get.
    agent.get(url).call()
}

pub fn get_http_agent() -> ureq::Agent {
    let agent: ureq::Agent = ureq::AgentBuilder::new()
        .timeout_read(Duration::from_secs(5))
        .timeout_write(Duration::from_secs(5))
        .https_only(true)
        .middleware(headers_middleware())
        .build();
    return agent;
}
