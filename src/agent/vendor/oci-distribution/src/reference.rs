use std::convert::TryFrom;
use std::error::Error;
use std::fmt;
use std::str::FromStr;

use crate::regexp;

/// NAME_TOTAL_LENGTH_MAX is the maximum total number of characters in a repository name.
const NAME_TOTAL_LENGTH_MAX: usize = 255;

const DOCKER_HUB_DOMAIN_LEGACY: &str = "index.docker.io";
const DOCKER_HUB_DOMAIN: &str = "docker.io";
const DOCKER_HUB_OFFICIAL_REPO_NAME: &str = "library";
const DEFAULT_TAG: &str = "latest";

/// Reasons that parsing a string as a Reference can fail.
#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    /// Invalid checksum digest format
    DigestInvalidFormat,
    /// Invalid checksum digest length
    DigestInvalidLength,
    /// Unsupported digest algorithm
    DigestUnsupported,
    /// Repository name must be lowercase
    NameContainsUppercase,
    /// Repository name must have at least one component
    NameEmpty,
    /// Repository name must not be more than NAME_TOTAL_LENGTH_MAX characters
    NameTooLong,
    /// Invalid reference format
    ReferenceInvalidFormat,
    /// Invalid tag format
    TagInvalidFormat,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::DigestInvalidFormat => write!(f, "invalid checksum digest format"),
            ParseError::DigestInvalidLength => write!(f, "invalid checksum digest length"),
            ParseError::DigestUnsupported => write!(f, "unsupported digest algorithm"),
            ParseError::NameContainsUppercase => write!(f, "repository name must be lowercase"),
            ParseError::NameEmpty => write!(f, "repository name must have at least one component"),
            ParseError::NameTooLong => write!(
                f,
                "repository name must not be more than {} characters",
                NAME_TOTAL_LENGTH_MAX
            ),
            ParseError::ReferenceInvalidFormat => write!(f, "invalid reference format"),
            ParseError::TagInvalidFormat => write!(f, "invalid tag format"),
        }
    }
}

impl Error for ParseError {}

/// Reference provides a general type to represent any way of referencing images within an OCI registry.
///
/// # Examples
///
/// Parsing a tagged image reference:
///
/// ```
/// use oci_distribution::Reference;
///
/// let reference: Reference = "docker.io/library/hello-world:latest".parse().unwrap();
///
/// assert_eq!("docker.io/library/hello-world:latest", reference.whole().as_str());
/// assert_eq!("docker.io", reference.registry());
/// assert_eq!("library/hello-world", reference.repository());
/// assert_eq!(Some("latest"), reference.tag());
/// assert_eq!(None, reference.digest());
/// ```
#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub struct Reference {
    registry: String,
    repository: String,
    tag: Option<String>,
    digest: Option<String>,
}

impl Reference {
    /// Create a Reference with a registry, repository and tag.
    pub fn with_tag(registry: String, repository: String, tag: String) -> Self {
        Self {
            registry,
            repository,
            tag: Some(tag),
            digest: None,
        }
    }

    /// Create a Reference with a registry, repository and digest.
    pub fn with_digest(registry: String, repository: String, digest: String) -> Self {
        Self {
            registry,
            repository,
            tag: None,
            digest: Some(digest),
        }
    }

    /// Resolve the registry address of a given `Reference`.
    ///
    /// Some registries, such as docker.io, uses a different address for the actual
    /// registry. This function implements such redirection.
    pub fn resolve_registry(&self) -> &str {
        let registry = self.registry();
        match registry {
            "docker.io" => "registry-1.docker.io",
            _ => registry,
        }
    }

    /// registry returns the name of the registry.
    pub fn registry(&self) -> &str {
        &self.registry
    }

    /// repository returns the name of the repository.
    pub fn repository(&self) -> &str {
        &self.repository
    }

    /// tag returns the object's tag, if present.
    pub fn tag(&self) -> Option<&str> {
        self.tag.as_deref()
    }

    /// digest returns the object's digest, if present.
    pub fn digest(&self) -> Option<&str> {
        self.digest.as_deref()
    }

    /// full_name returns the full repository name and path.
    fn full_name(&self) -> String {
        if self.registry() == "" {
            self.repository().to_string()
        } else {
            format!("{}/{}", self.registry(), self.repository())
        }
    }

    /// whole returns the whole reference.
    pub fn whole(&self) -> String {
        let mut s = self.full_name();
        if let Some(t) = self.tag() {
            if !s.is_empty() {
                s.push(':');
            }
            s.push_str(t);
        }
        if let Some(d) = self.digest() {
            if !s.is_empty() {
                s.push('@');
            }
            s.push_str(d);
        }
        s
    }
}

impl fmt::Display for Reference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.whole())
    }
}

impl FromStr for Reference {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Reference::try_from(s)
    }
}

impl TryFrom<String> for Reference {
    type Error = ParseError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        if s.is_empty() {
            return Err(ParseError::NameEmpty);
        }
        lazy_static! {
            static ref RE: regex::Regex = regexp::must_compile(regexp::REFERENCE_REGEXP);
        };
        let captures = match RE.captures(&s) {
            Some(caps) => caps,
            None => {
                return Err(ParseError::ReferenceInvalidFormat);
            }
        };
        let name = &captures[1];
        let mut tag = captures.get(2).map(|m| m.as_str().to_owned());
        let digest = captures.get(3).map(|m| m.as_str().to_owned());
        if tag.is_none() && digest.is_none() {
            tag = Some(DEFAULT_TAG.into());
        }
        let (registry, repository) = split_domain(name);
        let reference = Reference {
            registry,
            repository,
            tag,
            digest,
        };
        if reference.repository().len() > NAME_TOTAL_LENGTH_MAX {
            return Err(ParseError::NameTooLong);
        }
        // Digests much always be hex-encoded, ensuring that their hex portion will always be
        // size*2
        if reference.digest().is_some() {
            let d = reference.digest().unwrap();
            // FIXME: we should actually separate the algorithm from the digest
            // using regular expressions. This won't hold up if we support an
            // algorithm more or less than 6 characters like sha1024.
            if d.len() < 8 {
                return Err(ParseError::DigestInvalidFormat);
            }
            let algo = &d[0..6];
            let digest = &d[7..];
            match algo {
                "sha256" => {
                    if digest.len() != 64 {
                        return Err(ParseError::DigestInvalidLength);
                    }
                }
                "sha384" => {
                    if digest.len() != 96 {
                        return Err(ParseError::DigestInvalidLength);
                    }
                }
                "sha512" => {
                    if digest.len() != 128 {
                        return Err(ParseError::DigestInvalidLength);
                    }
                }
                _ => return Err(ParseError::DigestUnsupported),
            }
        }
        Ok(reference)
    }
}

impl TryFrom<&str> for Reference {
    type Error = ParseError;
    fn try_from(string: &str) -> Result<Self, Self::Error> {
        TryFrom::try_from(string.to_owned())
    }
}

impl From<Reference> for String {
    fn from(reference: Reference) -> Self {
        reference.whole()
    }
}

/// Splits a repository name to domain and remotename string.
/// If no valid domain is found, the default domain is used. Repository name
/// needs to be already validated before.
///
/// This function is a Rust rewrite of the official Go code used by Docker:
/// https://github.com/distribution/distribution/blob/41a0452eea12416aaf01bceb02a924871e964c67/reference/normalize.go#L87-L104
fn split_domain(name: &str) -> (String, String) {
    let mut domain: String;
    let mut remainder: String;

    match name.split_once('/') {
        None => {
            domain = DOCKER_HUB_DOMAIN.into();
            remainder = name.into();
        }
        Some((left, right)) => {
            if !(left.contains('.') || left.contains(':')) && left != "localhost" {
                domain = DOCKER_HUB_DOMAIN.into();
                remainder = name.into();
            } else {
                domain = left.into();
                remainder = right.into();
            }
        }
    }
    if domain == DOCKER_HUB_DOMAIN_LEGACY {
        domain = DOCKER_HUB_DOMAIN.into();
    }
    if domain == DOCKER_HUB_DOMAIN && !remainder.contains('/') {
        remainder = format!("{}/{}", DOCKER_HUB_OFFICIAL_REPO_NAME, remainder);
    }

    (domain, remainder)
}

#[cfg(test)]
mod test {
    use super::*;

    mod parse {
        use super::*;
        use rstest::rstest;

        #[rstest(input, registry, repository, tag, digest, whole,
            case("busybox", "docker.io", "library/busybox", Some("latest"), None, "docker.io/library/busybox:latest"),
            case("test.com:tag", "docker.io", "library/test.com", Some("tag"), None, "docker.io/library/test.com:tag"),
            case("test.com:5000", "docker.io", "library/test.com", Some("5000"), None, "docker.io/library/test.com:5000"),
            case("test.com/repo:tag", "test.com", "repo", Some("tag"), None, "test.com/repo:tag"),
            case("test:5000/repo", "test:5000", "repo", Some("latest"), None, "test:5000/repo:latest"),
            case("test:5000/repo:tag", "test:5000", "repo", Some("tag"), None, "test:5000/repo:tag"),
            case("test:5000/repo@sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff", "test:5000", "repo", None, Some("sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"), "test:5000/repo@sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"),
            case("test:5000/repo:tag@sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff", "test:5000", "repo", Some("tag"), Some("sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"), "test:5000/repo:tag@sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"),
            case("lowercase:Uppercase", "docker.io", "library/lowercase", Some("Uppercase"), None, "docker.io/library/lowercase:Uppercase"),
            case("sub-dom1.foo.com/bar/baz/quux", "sub-dom1.foo.com", "bar/baz/quux", Some("latest"), None, "sub-dom1.foo.com/bar/baz/quux:latest"),
            case("sub-dom1.foo.com/bar/baz/quux:some-long-tag", "sub-dom1.foo.com", "bar/baz/quux", Some("some-long-tag"), None, "sub-dom1.foo.com/bar/baz/quux:some-long-tag"),
            case("b.gcr.io/test.example.com/my-app:test.example.com", "b.gcr.io", "test.example.com/my-app", Some("test.example.com"), None, "b.gcr.io/test.example.com/my-app:test.example.com"),
            // ☃.com in punycode
            case("xn--n3h.com/myimage:xn--n3h.com", "xn--n3h.com", "myimage", Some("xn--n3h.com"), None, "xn--n3h.com/myimage:xn--n3h.com"),
            // 🐳.com in punycode
            case("xn--7o8h.com/myimage:xn--7o8h.com@sha512:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff", "xn--7o8h.com", "myimage", Some("xn--7o8h.com"), Some("sha512:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"), "xn--7o8h.com/myimage:xn--7o8h.com@sha512:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"),
            case("foo_bar.com:8080", "docker.io", "library/foo_bar.com", Some("8080"), None, "docker.io/library/foo_bar.com:8080" ),
            case("foo/foo_bar.com:8080", "docker.io", "foo/foo_bar.com", Some("8080"), None, "docker.io/foo/foo_bar.com:8080"),
            case("opensuse/leap:15.3", "docker.io", "opensuse/leap", Some("15.3"), None, "docker.io/opensuse/leap:15.3"),
        )]
        fn parse_good_reference(
            input: &str,
            registry: &str,
            repository: &str,
            tag: Option<&str>,
            digest: Option<&str>,
            whole: &str,
        ) {
            println!("input: {}", input);
            let reference = Reference::try_from(input).expect("could not parse reference");
            println!("{} -> {:?}", input, reference);
            assert_eq!(registry, reference.registry());
            assert_eq!(repository, reference.repository());
            assert_eq!(tag, reference.tag());
            assert_eq!(digest, reference.digest());
            assert_eq!(whole, reference.whole());
        }

        #[rstest(input, err,
            case("", ParseError::NameEmpty),
            case(":justtag", ParseError::ReferenceInvalidFormat),
            case("@sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff", ParseError::ReferenceInvalidFormat),
            case("repo@sha256:ffffffffffffffffffffffffffffffffff", ParseError::DigestInvalidLength),
            case("validname@invaliddigest:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff", ParseError::DigestUnsupported),
            // FIXME: should really pass a ParseError::NameContainsUppercase, but "invalid format" is good enough for now.
            case("Uppercase:tag", ParseError::ReferenceInvalidFormat),
            // FIXME: "Uppercase" is incorrectly handled as a domain-name here, and therefore passes.
            // https://github.com/docker/distribution/blob/master/reference/reference_test.go#L104-L109
            // case("Uppercase/lowercase:tag", ParseError::NameContainsUppercase),
            // FIXME: should really pass a ParseError::NameContainsUppercase, but "invalid format" is good enough for now.
            case("test:5000/Uppercase/lowercase:tag", ParseError::ReferenceInvalidFormat),
            case("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", ParseError::NameTooLong),
            case("aa/asdf$$^/aa", ParseError::ReferenceInvalidFormat)
        )]
        fn parse_bad_reference(input: &str, err: ParseError) {
            assert_eq!(Reference::try_from(input).unwrap_err(), err)
        }
    }
}
