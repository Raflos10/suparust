use httptest::matchers::{contains, eq, json_decoded, request, url_decoded};
use httptest::{all_of, responders, Expectation};

#[tokio::test]
async fn test_supabase() {
    env_logger::init();

    let mut server = httptest::Server::run();

    let dummy_apikey = "dummy_apikey";

    let client = crate::Supabase::new(&server.url_str(""), dummy_apikey, None);

    let dummy_username = "dummy_username";
    let dummy_password = "dummy_password";
    let dummy_refresh_token = "dummy_refresh_token";
    let dummy_access_token = "dummy_access_token";
    let dummy_expiration = chrono::Utc::now().timestamp() + 3600; // One hour ahead

    server.expect(
        Expectation::matching(all_of!(
            request::method("POST"),
            request::path("//auth/v1/token"),
            request::query(url_decoded(contains(("grant_type", "password")))),
            request::headers(contains(("apikey", dummy_apikey))),
            request::body(json_decoded(eq(serde_json::json!({
                "email": dummy_username,
                "password": dummy_password,
            }))))
        ))
        .respond_with(responders::json_encoded(serde_json::json!({
            "access_token": dummy_access_token,
            "expires_at": dummy_expiration,
            "refresh_token": dummy_refresh_token,
        }))),
    );

    let received_refresh_token = client
        .authorize(dummy_username, dummy_password)
        .await
        .unwrap();

    assert_eq!(received_refresh_token, dummy_refresh_token);
    server.verify_and_clear();

    let dummy_table = "table";

    #[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq, Clone)]
    struct DummyTableStruct {
        id: i32,
        name: String,
    }

    let dummy_table_content = vec![DummyTableStruct {
        id: 1,
        name: "John Doe".to_string(),
    }];

    server.expect(
        Expectation::matching(all_of!(
            request::method("GET"),
            request::path("//table"),
            request::query(url_decoded(contains(("select", "*")))),
            request::headers(contains(("apikey", dummy_apikey))),
            request::headers(contains((
                "authorization",
                format!("Bearer {dummy_access_token}")
            )))
        ))
        .respond_with(responders::json_encoded(dummy_table_content.clone())),
    );

    let response = client
        .from(dummy_table)
        .await
        .unwrap()
        .select("*")
        .execute()
        .await
        .unwrap()
        .json::<Vec<DummyTableStruct>>()
        .await
        .unwrap();

    assert_eq!(response, dummy_table_content);
}
