use rustc_serialize::json::Json;

use query::Query;
use query::parser::{QueryParseContext, QueryParseError, parse as parse_query};


pub fn parse(context: &QueryParseContext, json: &Json) -> Result<Query, QueryParseError> {
    let filters = try!(json.as_array().ok_or(QueryParseError::ExpectedArray));
    let mut sub_queries = Vec::new();

    for filter in filters.iter() {
        sub_queries.push(try!(parse_query(context, filter)));
    }

    Ok(Query::Bool {
        must: vec![],
        must_not: vec![],
        should: sub_queries,
        filter: vec![],
        minimum_should_match: 1,
        boost: 1.0f64,
    })
}