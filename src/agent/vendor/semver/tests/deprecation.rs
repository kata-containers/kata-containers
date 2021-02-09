extern crate semver;

#[test]
fn test_regressions() {
    use semver::VersionReq;
    use semver::ReqParseError;

    let versions = vec![
        (".*", VersionReq::any()),
        ("0.1.0.", VersionReq::parse("0.1.0").unwrap()),
        ("0.3.1.3", VersionReq::parse("0.3.13").unwrap()),
        ("0.2*", VersionReq::parse("0.2.*").unwrap()),
        ("*.0", VersionReq::any()),
    ];

    for (version, requirement) in versions.into_iter() {
        let parsed = VersionReq::parse(version);
        let error = parsed.err().unwrap();

        assert_eq!(ReqParseError::DeprecatedVersionRequirement(requirement), error);
    }
}
