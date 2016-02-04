use std::io::Read;
use std::collections::HashMap;
use std::fs;

use iron::prelude::*;
use iron::status;
use router::Router;
use rustc_serialize::json::{self, Json};
use rusqlite::Connection;

use super::{persistent, index_not_found_response};
use super::super::{Globals, Index, mapping, Document, query};


pub fn view_post_bulk(req: &mut Request) -> IronResult<Response> {
    let ref glob = get_globals!(req);

    // Lock index array
    let mut indices = glob.indices.write().unwrap();

    // Load data from body
    let mut payload = String::new();
    req.body.read_to_string(&mut payload).unwrap();

    let mut items = Vec::new();

    // Iterate
    let mut payload_lines = payload.split('\n');
    loop {
        let action_line = payload_lines.next();

        // Check if end of input
        if action_line == None || action_line == Some("") { break; }

        // Parse action line
        let action_json = parse_json!(&action_line.unwrap());

        // Check action
        // Action should be an object with only one key, the key name indicates the action and
        // the value is the parameters for that action
        let action_name = action_json.as_object().unwrap().keys().nth(0).unwrap();
        let action_params = action_json.as_object().unwrap().get(action_name).unwrap()
                                                            .as_object().unwrap();

        let doc_id = action_params.get("_id").unwrap().as_string().unwrap();
        let doc_type = action_params.get("_type").unwrap().as_string().unwrap();
        let doc_index = action_params.get("_index").unwrap().as_string().unwrap();

        match action_name.as_ref() {
            "index" => {
                let doc_line = payload_lines.next();
                let doc_json = parse_json!(&doc_line.unwrap());;

                // Find index
                let mut index = match indices.get_mut(doc_index) {
                    Some(index) => index,
                    None => {
                        return Ok(index_not_found_response());
                    }
                };

                // Find mapping
                let mut mapping = match index.mappings.get_mut(doc_type) {
                    Some(mapping) => mapping,
                    None => {
                        let mut response = Response::with((status::NotFound,
                                                           "{\"message\": \"Mapping not found\"}"));
                        response.headers.set_raw("Content-Type", vec![b"application/json".to_vec()]);
                        return Ok(response);
                    }
                };

                // Create and insert document
                let doc = Document::from_json(doc_json);
                index.docs.insert(doc_id.clone().to_owned(), doc);

                // Insert into "items" array
                let mut item = HashMap::new();
                // TODO: "create" may not always be right
                item.insert("create", action_params.clone());
                items.push(item);
            }
            _ => {
                warn!("Unrecognised action! {}", action_name);
            }
        }
    }

    return json_response!(status::Ok, format!("{{\"took\": {}, \"items\": {}}}", items.len(), json::encode(&items).unwrap()));
}