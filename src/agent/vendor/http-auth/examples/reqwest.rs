// Copyright (C) 2021 Scott Lamb <slamb@slamb.org>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Verbose example of making authenticated requests with the `reqwest` crate.

use std::convert::TryFrom;

use reqwest::header::HeaderValue;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (url, username, password) = match &args[..] {
        [_program, url, username, password] => (url, username, password),
        [program, ..] => {
            eprintln!("expected {} URL USERNAME PASSWORD", program);
            std::process::exit(1);
        }
        [] => panic!("no commandline arguments, not even argv[0]"),
    };
    let url = reqwest::Url::try_from(url.as_str()).unwrap();

    // Create a client which doesn't follow redirects. The URI used below won't
    // be correct with reqwest's automatic redirect handling.
    let client = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let first_resp = client.get(url.clone()).send().unwrap();
    if first_resp.status() != reqwest::StatusCode::UNAUTHORIZED {
        eprintln!(
            "Server returned status {} without authentication!",
            first_resp.status()
        );
        std::process::exit(1);
    }

    let mut pw_client = http_auth::PasswordClient::try_from(
        first_resp
            .headers()
            .get_all(reqwest::header::WWW_AUTHENTICATE),
    )
    .unwrap();
    println!("Password challenge client: {:#?}", &pw_client);
    let authorization = pw_client
        .respond(&http_auth::PasswordParams {
            username,
            password,

            // Note that URI is typically a path.
            uri: url.path(),
            method: reqwest::Method::GET.as_str(),
            body: Some(&[]),
        })
        .unwrap();
    println!("Authorization: {}", &authorization);
    let mut authorization = HeaderValue::try_from(authorization).unwrap();
    authorization.set_sensitive(true);
    let second_resp = client
        .get(url)
        .header(reqwest::header::AUTHORIZATION, authorization)
        .send()
        .unwrap();
    println!(
        "After authorization, server returned status {}",
        second_resp.status()
    );
}
