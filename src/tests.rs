use httptest::matchers::{contains, eq, json_decoded, request, url_decoded};
use httptest::{all_of, responders, Expectation};

fn new_dummy_session(prefix: &str, expiration: std::time::SystemTime) -> crate::auth::Session {
    crate::auth::Session {
        provider_token: None,
        provider_refresh_token: None,
        access_token: format!("{prefix}_access_token"),
        token_type: "bearer".to_string(),
        expires_in: expiration
            .duration_since(std::time::SystemTime::now())
            .unwrap()
            .as_secs() as i64,
        expires_at: expiration
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        refresh_token: format!("{prefix}_refresh_token"),
        user: Default::default(),
    }
}

#[tokio::test]
async fn test_supabase() {
    env_logger::init();

    let mut server = httptest::Server::run();

    let dummy_apikey = "dummy_apikey";

    let client = crate::Supabase::new(
        &server.url_str(""),
        dummy_apikey,
        None,
        crate::auth::SessionChangeListener::Ignore,
    );

    let dummy_username = "dummy_username";
    let dummy_password = "dummy_password";
    let dummy_session = new_dummy_session(
        "dummy",
        std::time::SystemTime::now() + std::time::Duration::from_secs(3600),
    );

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
        .respond_with(responders::json_encoded(dummy_session.clone())),
    );

    let received_session = client
        .login_with_email(dummy_username, dummy_password)
        .await
        .unwrap();

    assert_eq!(received_session, dummy_session);
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
            request::path(format!("//rest/v1/{}", dummy_table)),
            request::query(url_decoded(contains(("select", "*")))),
            request::headers(contains(("apikey", dummy_apikey))),
            request::headers(contains((
                "authorization",
                format!("Bearer {}", dummy_session.access_token)
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

fn expect_refresh_token(
    server: &mut httptest::Server,
    api_key: &str,
    old_refresh_token: &str,
    new_session: &crate::auth::Session,
) {
    server.expect(
        Expectation::matching(all_of!(
            request::method("POST"),
            request::path("//auth/v1/token"),
            request::query(url_decoded(contains(("grant_type", "refresh_token")))),
            request::headers(contains(("apikey", api_key.to_string()))),
            request::body(json_decoded(eq(serde_json::json!({
                "refresh_token": old_refresh_token.to_string(),
            }))))
        ))
        .respond_with(responders::json_encoded(new_session)),
    );
}

enum RefreshTokenTest {
    Postgrest,
    Storage,
}

#[test_case::test_case(RefreshTokenTest::Postgrest)]
#[test_case::test_case(RefreshTokenTest::Storage)]
#[tokio::test]
async fn check_refresh_token(test_type: RefreshTokenTest) {
    //env_logger::init();
    let mut server = httptest::Server::run();

    let dummy_apikey = "dummy_apikey";

    let dummy_session = new_dummy_session(
        "dummy",
        std::time::SystemTime::now() + std::time::Duration::from_secs(30), // In half a minute
    );

    let client = crate::Supabase::new(
        &server.url_str(""),
        dummy_apikey,
        Some(dummy_session.clone()),
        crate::auth::SessionChangeListener::Ignore,
    );

    let renewed_session = new_dummy_session(
        "renewed",
        std::time::SystemTime::now() + std::time::Duration::from_secs(300),
    );

    expect_refresh_token(
        &mut server,
        dummy_apikey,
        &dummy_session.refresh_token,
        &renewed_session,
    );

    match test_type {
        RefreshTokenTest::Postgrest => {
            let dummy_table = "table";
            server.expect(
                Expectation::matching(all_of!(
                    request::method("GET"),
                    request::path(format!("//rest/v1/{}", dummy_table)),
                    request::query(url_decoded(contains(("select", "*")))),
                    request::headers(contains(("apikey", dummy_apikey))),
                    request::headers(contains((
                        "authorization",
                        format!("Bearer {}", renewed_session.access_token)
                    )))
                ))
                .respond_with(responders::json_encoded(Vec::<i64>::new())),
            );

            let _ = client
                .from(dummy_table)
                .await
                .unwrap()
                .select("*")
                .execute()
                .await
                .unwrap()
                .json::<Vec<i64>>()
                .await
                .unwrap();
        }
        RefreshTokenTest::Storage => {
            let dummy_prefix = "dummy";
            server.expect(
                Expectation::matching(all_of!(
                    request::method("POST"),
                    request::path(format!("//storage/v1/object/list/{dummy_prefix}")),
                    request::headers(contains(("apikey", dummy_apikey))),
                    request::headers(contains((
                        "authorization",
                        format!("Bearer {}", renewed_session.access_token)
                    )))
                ))
                .respond_with(responders::json_encoded(serde_json::json!([]))),
            );

            let _ = client
                .storage()
                .await
                .unwrap()
                .object()
                .list(
                    dummy_prefix,
                    crate::storage::object::ListRequest::new("something".to_string()),
                )
                .await
                .unwrap();
        }
    }
}
