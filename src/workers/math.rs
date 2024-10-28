// This is an http worker that can be used with the graph_walker for example to scrape the web or a list of urls etc
use std::time::Duration;
use std::{env, thread};
use ureq::Error::Status;
use ureq::{Error, MiddlewareNext, Request, Response};

