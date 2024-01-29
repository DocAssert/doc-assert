#[cfg(test)]
#[test]
fn compare_jsons() {
    use assert_json_diff::assert_json_matches_no_panic;
    use assert_json_diff::{CompareMode, Config};
    use serde_json::json;

    let a = json!({
        "data": {
            "users": [
                {
                    "country": {
                        "name": "Sweden",
                        "cities": [
                            {
                                "id": 1,
                                "name": "Stockholm"
                            },
                            {
                                "id": 2,
                                "name": "Gothenburg"
                            }
                        ]
                    },
                    "id": 1
                },
                {
                    "id": 2,
                    "country": {
                        "name": "Denmark",
                        "cities": [
                            {
                                "id": 3,
                                "name": "Copenhagen"
                            },
                            {
                                "id": 4,
                                "name": "Aarhus"
                            }
                        ]
                    }
                }
            ]
        }
    });

    let b = json!({
        "data": {
            "users": [
                {
                    "country": {
                        "name": "Sweden",
                        "cities": [
                            {
                                "id": 1,
                                "name": "Stockholm"
                            },
                            {
                                "id": 2,
                                "name": "Gothenburg"
                            }
                        ]
                    },
                    "id": 1
                },
                {
                    "id": 2,
                    "country": {
                        "cities": [
                            {
                                "name": "Copenhagen",
                                "id": 3
                            },
                            {
                                "name": "Aarhus",
                                "id": 4
                            }
                        ],
                        "name": "Denmark"
                    }
                }
            ]
        }
    });

    let result = assert_json_matches_no_panic(&a, &b, Config::new(CompareMode::Strict));
    match result {
        Ok(_) => println!("Success!"),
        Err(e) => println!("Error: {}", e),
    }
}
