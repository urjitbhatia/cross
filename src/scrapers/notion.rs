
macro_rules! URL_TMPL {
    ($block_id: expr) => {
        format!(
            "https://api.notion.com/v1/blocks/{}/children?page_size=100",
            $block_id
        )
    };
}
macro_rules! NEXT_CURSOR_URL_TMPL {
    ($block_id: expr, $start_cursor_id: expr) => {
        format!(
            "https://api.notion.com/v1/blocks/{}/children?page_size=100&start_cursor={}",
            $block_id, $start_cursor_id
        )
    };
}

fn read_page(
    http_client: Agent,
    mut page_url: String,
    page_id: String,
    w: &Worker<(String, String)>,
) -> Option<Vec<String>> {
    let mut page_contents: Vec<String> = Vec::new();

    loop {
        println!(
            "Reading data for: {page_id} thread: {:?}",
            thread::current().id()
        );

        let data = match make_json_api_call(http_client.clone(), page_url.clone()) {
            Ok(v) => v,
            Err(e) => {
                println!(
                    "Failed to fetch data from http req: {e} status code: {:?}",
                    e
                );
                break;
            }
        };

        // println!("{}", serde_json::to_string_pretty(&data).unwrap());
        let Some(results) = data["results"].as_array() else { continue; };

        for result in results {
            if result["type"].as_str().unwrap_or_default() == "link_to_page" {
                let child_page_id = result["link_to_page"]["page_id"].as_str().unwrap();
                println!("Pushing links_to_page link to worker: {child_page_id}");
                w.push((
                    String::from(BLOCK_CHILD_URL_TMPL!(child_page_id)),
                    String::from(child_page_id),
                ));
            }

            {
                let Some(result_type) = result["type"].as_str() else { continue; };
                let Some(result) = result[result_type].as_object() else { continue; };
                match parse_rich_text(result, result_type) {
                    Some(r) => page_contents.push(r),
                    None => (),
                }
            }

            // Children should be considered a part of the same document
            if result["has_children"].as_bool().unwrap_or_default() {
                if let Some(child_page_id) = result["id"].as_str() {
                    if result["type"].as_str().unwrap_or_default() == "child_page" {
                        // these should be considered as new documents
                        println!("Pushing child_page link to worker: {child_page_id}");
                        w.push((
                            String::from(BLOCK_CHILD_URL_TMPL!(child_page_id)),
                            String::from(child_page_id),
                        ));
                    } else {
                        let child_lines = read_page(
                            http_client.clone(),
                            BLOCK_CHILD_URL_TMPL!(child_page_id),
                            String::from(child_page_id),
                            w,
                        );
                        child_lines
                            .unwrap_or_default()
                            .iter()
                            .for_each(|l| page_contents.push(format!("\t{l}")));
                    }
                }
            }
        }

        // Handle Pagination - fetch more results if has next cursor
        if data["has_more"].as_bool().unwrap_or_default() {
            match data["next_cursor"].as_str() {
                Some(cur) => {
                    page_url = String::from(BLOCK_NEXT_CURSOR_URL_TMPL!(page_id.clone(), cur));
                }
                None => {
                    println!("Failed to unmarshal next cursor {}", data["next_cursor"]);
                    break;
                }
            };
        } else {
            break;
        }
    }
    Some(page_contents)
}
