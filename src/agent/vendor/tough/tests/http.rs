mod test_utils;

/// Instead of guarding every individual thing with `#[cfg(feature = "http")]`, use a module.
#[cfg(feature = "http")]
mod http_happy {
    use crate::test_utils::{read_to_end, test_data};
    use httptest::{matchers::*, responders::*, Expectation, Server};
    use std::fs::File;
    use std::str::FromStr;
    use tough::{DefaultTransport, HttpTransport, RepositoryLoader, TargetName, Transport};
    use url::Url;

    /// Set an expectation in a test HTTP server which serves a file from `tuf-reference-impl`.
    fn create_successful_get(relative_path: &str) -> httptest::Expectation {
        let repo_dir = test_data().join("tuf-reference-impl");
        let file_bytes = std::fs::read(&repo_dir.join(relative_path)).unwrap();
        Expectation::matching(request::method_path("GET", format!("/{}", relative_path)))
            .times(1)
            .respond_with(
                status_code(200)
                    .append_header("content-type", "application/octet-stream")
                    .body(file_bytes),
            )
    }

    /// Set an expectation in a test HTTP server to return a `403 Forbidden` status code.
    /// This is necessary for objects like `x.root.json` as tough will continue to increment
    /// `x.root.json` until it receives either `403 Forbidden` or `404 NotFound`.
    /// S3 returns `403 Forbidden` when requesting a file that does not exist.
    fn create_unsuccessful_get(relative_path: &str) -> httptest::Expectation {
        Expectation::matching(request::method_path("GET", format!("/{}", relative_path)))
            .times(1)
            .respond_with(status_code(403))
    }

    /// Test that `tough` works with a healthy HTTP server.
    #[test]
    fn test_http_transport_happy_case() {
        run_http_test(HttpTransport::default());
    }

    /// Test that `DefaultTransport` works over HTTP when the `http` feature is enabled.
    #[test]
    fn test_http_default_transport() {
        run_http_test(DefaultTransport::default());
    }

    fn run_http_test<T: Transport + 'static>(transport: T) {
        let server = Server::run();
        let repo_dir = test_data().join("tuf-reference-impl");
        server.expect(create_successful_get("metadata/timestamp.json"));
        server.expect(create_successful_get("metadata/snapshot.json"));
        server.expect(create_successful_get("metadata/targets.json"));
        server.expect(create_successful_get("metadata/role1.json"));
        server.expect(create_successful_get("metadata/role2.json"));
        server.expect(create_successful_get("targets/file1.txt"));
        server.expect(create_successful_get("targets/file2.txt"));
        server.expect(create_unsuccessful_get("metadata/2.root.json"));
        let metadata_base_url = Url::from_str(server.url_str("/metadata").as_str()).unwrap();
        let targets_base_url = Url::from_str(server.url_str("/targets").as_str()).unwrap();
        let repo = RepositoryLoader::new(
            File::open(repo_dir.join("metadata").join("1.root.json")).unwrap(),
            metadata_base_url,
            targets_base_url,
        )
        .transport(transport)
        .load()
        .unwrap();

        let file1 = TargetName::new("file1.txt").unwrap();
        assert_eq!(
            read_to_end(repo.read_target(&file1).unwrap().unwrap()),
            &b"This is an example target file."[..]
        );
        let file2 = TargetName::new("file2.txt").unwrap();
        assert_eq!(
            read_to_end(repo.read_target(&file2).unwrap().unwrap()),
            &b"This is an another example target file."[..]
        );
        assert_eq!(
            repo.targets()
                .signed
                .targets
                .get(&file1)
                .unwrap()
                .custom
                .get("file_permissions")
                .unwrap(),
            "0644"
        );
    }
}

#[cfg(feature = "http")]
#[cfg(feature = "integ")]
mod http_integ {
    use crate::test_utils::test_data;
    use std::fs::File;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};
    use tough::{HttpTransportBuilder, RepositoryLoader};
    use url::Url;

    pub fn integ_dir() -> PathBuf {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.pop();
        p = p.join("integ");
        p
    }

    /// Returns a command object that runs the provided script under BASH , wether we are under cygwin or unix.
    pub fn bash_base() -> Command {
        // if under cygwin, run the bash script under cygwin64 bash
        if cfg!(target_os = "windows") {
            let mut command = Command::new("c:\\cygwin64\\bin\\bash");
            command.arg("-l");
            return command;
        } else {
            return Command::new("bash");
        }
    }

    pub fn tuf_reference_impl() -> PathBuf {
        test_data().join("tuf-reference-impl")
    }

    pub fn tuf_reference_impl_metadata() -> PathBuf {
        tuf_reference_impl().join("metadata")
    }

    pub fn tuf_reference_impl_root_json() -> PathBuf {
        tuf_reference_impl_metadata().join("1.root.json")
    }

    /// Test `tough` using faulty HTTP connections.
    ///
    /// This test requires `docker` and should be disabled for PRs because it will not work with our
    /// current CI setup. It works by starting HTTP services in containers which serve the tuf-
    /// reference-impl through fault-ridden HTTP. We load the repo many times in a loop, and
    /// statistically exercise many of the retry code paths. In particular, the server aborts during
    /// the send which exercises the range-header retry in the `Read` loop, and 5XX's are also sent
    /// triggering retries in the `fetch` loop.
    #[test]
    fn test_retries() {
        use std::ffi::OsString;
        // run docker images to create a faulty http representation of tuf-reference-impl

        // Get the "run.sh" path
        let script_path = integ_dir()
            .join("failure-server")
            .join("run.sh")
            .into_os_string()
            .into_string()
            .unwrap();

        // Run it under BASH
        let output = bash_base()
            .arg(OsString::from(script_path))
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()
            .expect("failed to start server with docker containers");

        if !output.status.success() {
            panic!("Failed to run integration test HTTP servers, is docker running?");
        }

        // load the tuf-reference-impl repo via http repeatedly through faulty proxies
        for i in 0..5 {
            let transport = HttpTransportBuilder::new()
                // the service we have created is very toxic with many failures, so we will do a
                // large number of retries, enough that we can be reasonably assured that we will
                // always succeed.
                .tries(200)
                // we don't want the test to take forever so we use small pauses
                .initial_backoff(std::time::Duration::from_nanos(100))
                .max_backoff(std::time::Duration::from_millis(1))
                .build();
            let root_path = tuf_reference_impl_root_json();

            RepositoryLoader::new(
                File::open(&root_path).unwrap(),
                Url::parse("http://localhost:10103/metadata").unwrap(),
                Url::parse("http://localhost:10103/targets").unwrap(),
            )
            .transport(transport)
            .load()
            .unwrap();
            println!("{}:{} SUCCESSFULLY LOADED THE REPO {}", file!(), line!(), i,);
        }

        // stop and delete the docker containers, images and network
        let output = bash_base()
            .arg(
                integ_dir()
                    .join("failure-server")
                    .join("teardown.sh")
                    .into_os_string(),
            )
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()
            .expect("failed to delete docker objects");
        assert!(output.status.success());
    }
}
