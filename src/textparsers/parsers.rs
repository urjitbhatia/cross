use std::{convert::identity, error::Error};

use serde_json::Value;
use ureq::Agent;

use crate::workers::get_with_retry;

pub fn parse_rich_text(
    result: &serde_json::Map<String, Value>,
    result_type: &str,
) -> Option<String> {
    if !result.contains_key("rich_text") {
        return None;
    }

    let prefix = match result_type {
        "bulleted_list_item" => "* ",
        "code" => "\n```\n",
        _ => "",
    };
    let suffix = match result_type {
        "paragraph" => "\n",
        "code" => "\n```",
        _ => "",
    };

    let l = result["rich_text"]
        .as_array()?
        .iter()
        .map(|rt| rt.as_object())
        .filter_map(identity)
        .map(|rt| {
            let href = rt["href"].as_str().unwrap_or_default();
            let val = if rt.contains_key("plain_text") {
                rt["plain_text"].as_str()
            } else if rt.contains_key("text") {
                rt["text"].as_str()
            } else {
                None
            };
            val.map(|val| {
                if href == "" {
                    String::from(val)
                } else {
                    format!("[{}]({})", val, href)
                }
            })
        })
        .filter_map(identity)
        .fold(String::from(prefix), |acc, line| acc + &*line);

    Some(l + suffix)
}

pub fn make_json_api_call(agent: Agent, url: String) -> Result<Value, Box<dyn Error>> {
    let json: Value = get_with_retry(agent, url.as_str())?.into_json()?;
    Ok(json)
}
