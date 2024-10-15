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
- [ ] GraphQL
- [ ] ...

## Platform compatibility

The project supports both the `stable-x86_64-unknown-linux-gnu` and `wasm32-unknown-unknown` targets.

## Installation

`cargo add suparust`

## Usage

Examples will come soon.

## Contributing

Contributions are welcome. Please try to add tests for any new features or bug fixes.

As the library is in early development, please feel free to suggest refactorings or improvements to the API.

Some goals for the project:

- To be as standards- and [guidelines-compliant](https://rust-lang.github.io/api-guidelines/checklist.html) as possible
- To use other crates for features where there is a good fit (e.g. the `postgrest` crate for PostgREST)

## License

SPDX-License-Identifier: Apache-2.0 OR MIT
