# Suparust

This is a library for interfacing with projects hosted using [Supabase](https://supabase.io/).

The library is in early development, so expect breaking API changes to occur.

## Features

The goal is to support as much of the Supabase API as possible. Currently, the following features are supported:

- [ ] Auth
    - [x] Login with email and password
    - [x] Logout
    - [ ] ... more to come
- [x] Postgrest
- [ ] Storage
    - [x] Delete object
    - [x] Get object
    - [x] Update object
    - [x] Upload object
    - [x] List objects
    - [ ] ... more to come
- [ ] GraphQL
- [ ] ...

## Platform compatibility

The project supports both the `stable-x86_64-unknown-linux-gnu` and `wasm32-unknown-unknown` targets.

## Installation

`cargo add suparust`

## Usage

```rust
let client = suparust::Supabase::new(
    "https://your.postgrest.endpoint",
    "your_api_key",
    None,
    suparust::SessionChangeListener::Ignore);

client.login_with_email(
    "myemail@example.com",
    "mypassword").await?;

#[derive(serde::Deserialize)]
struct MyStruct {
    id: i64,
    field: String
}

// Postgrest example (see postgrest crate for more details on API)
let table_contents = client
    .from("your_table")
    .await?
    .select("*")
    .execute()
    .await?
    .json::<Vec<MyStruct> > ();
```

More examples will come as the library matures.

## Contributing

Contributions are welcome. Please try to add tests for any new features or bug fixes.

As the library is in early development, please feel free to suggest refactorings or improvements to the API.

Some goals for the project:

- To be as standards- and [guidelines-compliant](https://rust-lang.github.io/api-guidelines/checklist.html) as possible
- To use other crates for features where there is a good fit (e.g. the `postgrest` crate for PostgREST)

## License

SPDX-License-Identifier: Apache-2.0 OR MIT
