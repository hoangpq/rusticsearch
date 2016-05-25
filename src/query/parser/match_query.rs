use rustc_serialize::json::Json;

use analysis::Analyzer;
use term::Term;
use mapping::FieldMapping;
use query::{Query, TermMatcher};
use query::parser::{QueryParseContext, QueryParseError};
use query::parser::utils::{parse_string, parse_float, Operator, parse_operator};


pub fn parse(context: &QueryParseContext, json: &Json) -> Result<Query, QueryParseError> {
    let object = try!(json.as_object().ok_or(QueryParseError::ExpectedObject));

    // Match queries are single-key objects. The key is the field name, the value is either a
    // string or a sub-object with extra configuration:
    //
    // {
    //     "foo": "bar"
    // }
    //
    // {
    //     "foo": {
    //         "query": "bar",
    //         "boost": 2.0
    //     }
    // }
    //
    let field_name = if object.len() == 1 {
        object.keys().collect::<Vec<_>>()[0]
    } else {
        return Err(QueryParseError::ExpectedSingleKey)
    };

    // Get mapping for field
    let field_mapping = context.index.get_field_mapping_by_name(field_name);

    // Get configuration
    let mut query = Json::Null;
    let mut boost = 1.0f64;
    let mut operator = Operator::Or;

    match object[field_name] {
        Json::String(ref string) => query = object[field_name].clone(),
        Json::Object(ref inner_object) => {
            let mut has_query_key = false;

            for (key, value) in inner_object.iter() {
                match key.as_ref() {
                    "query" => {
                        has_query_key = true;
                        query = value.clone();
                    }
                    "boost" => {
                        boost = try!(parse_float(value));
                    }
                    "operator" => {
                        operator = try!(parse_operator(value))
                    }
                    _ => return Err(QueryParseError::UnrecognisedKey(key.clone()))
                }
            }

            if !has_query_key {
                return Err(QueryParseError::ExpectedKey("query"))
            }
        }
        _ => return Err(QueryParseError::ExpectedObjectOrString),
    }

    // Tokenise query string
    let tokens = match field_mapping {
        Some(ref field_mapping) => {
            field_mapping.process_value_for_query(query.clone())
        }
        None => {
            // TODO: Raise error?
            warn!("Unknown field: {}", field_name);

            FieldMapping::default().process_value_for_query(query.clone())
        }
    };

    let tokens = match tokens {
        Some(tokens) => tokens,
        None => {
            // Couldn't convert the passed in value into tokens
            // TODO: Raise error
            warn!("Unprocessable query: {}", query);

            return Ok(Query::MatchNone);
        }
    };

    // Create a term query for each token
    let mut sub_queries = Vec::new();
    for token in tokens {
        sub_queries.push(Query::MatchTerm {
            field: field_name.clone(),
            term: token.term,
            matcher: TermMatcher::Exact,
            boost: 1.0f64,
        });
    }

    // Combine the term queries
    match operator {
        Operator::Or => {
            Ok(Query::Bool {
                must: vec![],
                must_not: vec![],
                should: sub_queries,
                filter: vec![],
                minimum_should_match: 1,
                boost: boost,
            })
        }
        Operator::And => {
            Ok(Query::Bool {
                must: sub_queries,
                must_not: vec![],
                should: vec![],
                filter: vec![],
                minimum_should_match: 0,
                boost: boost,
            })
        }
    }
}


#[cfg(test)]
mod tests {
    use rustc_serialize::json::Json;

    use term::Term;
    use query::{Query, TermMatcher};
    use query::parser::{QueryParseContext, QueryParseError};
    use index::Index;

    use super::parse;

    #[test]
    fn test_match_query() {
        let query = parse(&QueryParseContext::new(&Index::new()), &Json::from_str("
        {
            \"foo\": {
                \"query\": \"bar\"
            }
        }
        ").unwrap());

        assert_eq!(query, Ok(Query::Bool {
            must: vec![],
            must_not: vec![],
            should: vec![
                Query::MatchTerm {
                    field: "foo".to_string(),
                    term: Term::String("bar".to_string()),
                    boost: 1.0f64,
                    matcher: TermMatcher::Exact
                }
            ],
            filter: vec![],
            minimum_should_match: 1,
            boost: 1.0f64,
        }))
    }

    #[test]
    fn test_multi_term_match_query() {
        let query = parse(&QueryParseContext::new(&Index::new()), &Json::from_str("
        {
            \"foo\": {
                \"query\": \"bar baz\"
            }
        }
        ").unwrap());

        assert_eq!(query, Ok(Query::Bool {
            must: vec![],
            must_not: vec![],
            should: vec![
                Query::MatchTerm {
                    field: "foo".to_string(),
                    term: Term::String("bar".to_string()),
                    boost: 1.0f64,
                    matcher: TermMatcher::Exact
                },
                Query::MatchTerm {
                    field: "foo".to_string(),
                    term: Term::String("baz".to_string()),
                    boost: 1.0f64,
                    matcher: TermMatcher::Exact
                }
            ],
            filter: vec![],
            minimum_should_match: 1,
            boost: 1.0f64,
        }))
    }

    #[test]
    fn test_simple_multi_term_match_query() {
        let query = parse(&QueryParseContext::new(&Index::new()), &Json::from_str("
        {
            \"foo\": \"bar baz\"
        }
        ").unwrap());

        assert_eq!(query, Ok(Query::Bool {
            must: vec![],
            must_not: vec![],
            should: vec![
                Query::MatchTerm {
                    field: "foo".to_string(),
                    term: Term::String("bar".to_string()),
                    boost: 1.0f64,
                    matcher: TermMatcher::Exact
                },
                Query::MatchTerm {
                    field: "foo".to_string(),
                    term: Term::String("baz".to_string()),
                    boost: 1.0f64,
                    matcher: TermMatcher::Exact
                }
            ],
            filter: vec![],
            minimum_should_match: 1,
            boost: 1.0f64,
        }))
    }

    #[test]
    fn test_with_boost() {
        let query = parse(&QueryParseContext::new(&Index::new()), &Json::from_str("
        {
            \"foo\": {
                \"query\": \"bar\",
                \"boost\": 2.0
            }
        }
        ").unwrap());

        assert_eq!(query, Ok(Query::Bool {
            must: vec![],
            must_not: vec![],
            should: vec![
                Query::MatchTerm {
                    field: "foo".to_string(),
                    term: Term::String("bar".to_string()),
                    boost: 1.0f64,
                    matcher: TermMatcher::Exact
                }
            ],
            filter: vec![],
            minimum_should_match: 1,
            boost: 2.0f64,
        }))
    }

    #[test]
    fn test_with_boost_integer() {
        let query = parse(&QueryParseContext::new(&Index::new()), &Json::from_str("
        {
            \"foo\": {
                \"query\": \"bar\",
                \"boost\": 2
            }
        }
        ").unwrap());

        assert_eq!(query, Ok(Query::Bool {
            must: vec![],
            must_not: vec![],
            should: vec![
                Query::MatchTerm {
                    field: "foo".to_string(),
                    term: Term::String("bar".to_string()),
                    boost: 1.0f64,
                    matcher: TermMatcher::Exact
                }
            ],
            filter: vec![],
            minimum_should_match: 1,
            boost: 2.0f64,
        }))
    }

    #[test]
    fn test_with_and_operator() {
        let query = parse(&QueryParseContext::new(&Index::new()), &Json::from_str("
        {
            \"foo\": {
                \"query\": \"bar\",
                \"operator\": \"and\"
            }
        }
        ").unwrap());

        assert_eq!(query, Ok(Query::Bool {
            must: vec![
                Query::MatchTerm {
                    field: "foo".to_string(),
                    term: Term::String("bar".to_string()),
                    boost: 1.0f64,
                    matcher: TermMatcher::Exact
                }
            ],
            must_not: vec![],
            should: vec![],
            filter: vec![],
            minimum_should_match: 0,
            boost: 1.0f64,
        }))
    }

    #[test]
    fn test_gives_error_for_incorrect_type() {
        // Array
        let query = parse(&QueryParseContext::new(&Index::new()), &Json::from_str("
        [
            \"foo\"
        ]
        ").unwrap());

        assert_eq!(query, Err(QueryParseError::ExpectedObject));

        // Integer
        let query = parse(&QueryParseContext::new(&Index::new()), &Json::from_str("
        123
        ").unwrap());

        assert_eq!(query, Err(QueryParseError::ExpectedObject));

        // Float
        let query = parse(&QueryParseContext::new(&Index::new()), &Json::from_str("
        123.1234
        ").unwrap());

        assert_eq!(query, Err(QueryParseError::ExpectedObject));
    }

    #[test]
    fn test_gives_error_for_incorrect_boost_type() {
        // String
        let query = parse(&QueryParseContext::new(&Index::new()), &Json::from_str("
        {
            \"foo\": {
                \"query\": \"bar\",
                \"boost\": \"2\"
            }
        }
        ").unwrap());

        assert_eq!(query, Err(QueryParseError::ExpectedFloat));

        // Array
        let query = parse(&QueryParseContext::new(&Index::new()), &Json::from_str("
        {
            \"foo\": {
                \"query\": \"bar\",
                \"boost\": [2]
            }
        }
        ").unwrap());

        assert_eq!(query, Err(QueryParseError::ExpectedFloat));

        // Object
        let query = parse(&QueryParseContext::new(&Index::new()), &Json::from_str("
        {
            \"foo\": {
                \"query\": \"bar\",
                \"boost\": {
                    \"value\": 2
                }
            }
        }
        ").unwrap());

        assert_eq!(query, Err(QueryParseError::ExpectedFloat));
    }

    #[test]
    fn test_gives_error_for_missing_query() {
        let query = parse(&QueryParseContext::new(&Index::new()), &Json::from_str("
        {
            \"foo\": {
            }
        }
        ").unwrap());

        assert_eq!(query, Err(QueryParseError::ExpectedKey("query")));
    }

    #[test]
    fn test_gives_error_for_extra_key() {
        let query = parse(&QueryParseContext::new(&Index::new()), &Json::from_str("
        {
            \"foo\": {
                \"query\": \"bar\"
            },
            \"hello\": \"world\"
        }
        ").unwrap());

        assert_eq!(query, Err(QueryParseError::ExpectedSingleKey));
    }

    #[test]
    fn test_gives_error_for_extra_inner_key() {
        let query = parse(&QueryParseContext::new(&Index::new()), &Json::from_str("
        {
            \"foo\": {
                \"query\": \"bar\",
                \"hello\": \"world\"
            }
        }
        ").unwrap());

        assert_eq!(query, Err(QueryParseError::UnrecognisedKey("hello".to_string())));
    }
}
