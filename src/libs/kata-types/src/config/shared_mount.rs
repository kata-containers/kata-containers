use std::io::Result;

use regex::Regex;

use crate::eother;

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct SharedMount {
    /// Name is used to identify a pair of shared mount points.
    /// This field cannot be omitted.
    #[serde(default)]
    pub name: String,

    /// Src_ctr is used to specify the name of the source container.
    /// This field cannot be omitted.
    #[serde(default)]
    pub src_ctr: String,

    /// Src_path is used to specify the path to the shared mount point in the source container.
    /// Src_path must conform to the regular expression `^(/[-\w.]+)+/?$` and cannot contain `/../`.
    /// This field cannot be omitted.
    #[serde(default)]
    pub src_path: String,

    /// Dst_ctr is used to specify the name of the destination container.
    /// This field cannot be omitted.
    #[serde(default)]
    pub dst_ctr: String,

    /// Dst_path is used to specify the destination path where the shared mount point will be mounted.
    /// Dst_path must conform to the regular expression `^(/[-\w.]+)+/?$` and cannot contain `/../`.
    /// This field cannot be omitted.
    #[serde(default)]
    pub dst_path: String,
}

impl SharedMount {
    pub fn validate(&self) -> Result<()> {
        if self.name.is_empty() {
            return Err(eother!("shared_mount: field 'name' couldn't be empty."));
        }
        if self.src_ctr.is_empty() {
            return Err(eother!("shared_mount: field 'src_ctr' couldn't be empty."));
        }
        if self.dst_ctr.is_empty() {
            return Err(eother!("shared_mount: field 'dst_ctr' couldn't be empty."));
        }
        if self.src_path.is_empty() {
            return Err(eother!("shared_mount: field 'src_path' couldn't be empty."));
        }
        if self.dst_path.is_empty() {
            return Err(eother!("shared_mount: field 'dst_path' couldn't be empty."));
        }

        let re = match Regex::new(r"^(/[-\w.]+)+/?$") {
            Ok(re) => re,
            Err(e) => return Err(eother!("Compiling the regular expression failed: {}.", e)),
        };
        if !re.is_match(&self.src_path) {
            return Err(eother!("shared_mount '{}': src_path is invalid. It must be an absolute path and can only contain letters, numbers, hyphens(-), underscores(_) and dots(.).", self.name));
        }
        let dirs: Vec<&str> = self.src_path.split('/').collect();
        for dir in dirs {
            if dir == ".." {
                return Err(eother!(
                    "shared_mount '{}': src_path couldn't contain '..' directory.",
                    self.name
                ));
            }
        }
        if !re.is_match(&self.dst_path) {
            return Err(eother!("shared_mount '{}': dst_path is invalid. It must be an absolute path and can only contain letters, numbers, hyphens(-), underscores(_) and dots(.).", self.name));
        }
        let dirs: Vec<&str> = self.dst_path.split('/').collect();
        for dir in dirs {
            if dir == ".." {
                return Err(eother!(
                    "shared_mount '{}': dst_path couldn't contain '..' directory.",
                    self.name
                ));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate() {
        #[derive(Debug)]
        struct TestData<'a> {
            shared_mount_annotation: &'a str,
            result: bool,
            message: &'a str,
        }

        let tests = &[
            TestData {
                shared_mount_annotation: r#"
                {
                    "name": "test",
                    "src_ctr": "sidecar",
                    "src_path": "/mnt/storage",
                    "dst_ctr": "app",
                    "dst_path": "/mnt/storage"
                }"#,
                result: true,
                message: "",
            },
            TestData {
                shared_mount_annotation: r#"
                {
                    "src_ctr": "sidecar",
                    "src_path": "/mnt/storage",
                    "dst_ctr": "app",
                    "dst_path": "/mnt/storage"
                }"#,
                result: false,
                message: "shared_mount: field 'name' couldn't be empty.",
            },
            TestData {
                shared_mount_annotation: r#"
                {
                    "name": "test",
                    "src": "sidecar",
                    "src_path": "/mnt/storage",
                    "dst_ctr": "app",
                    "dst_path": "/mnt/storage"
                }"#,
                result: false,
                message: "shared_mount: field 'src_ctr' couldn't be empty.",
            },
            TestData {
                shared_mount_annotation: r#"
                {
                    "name": "test",
                    "src_ctr": "sidecar",
                    "src_dir": "/mnt/storage",
                    "dst_ctr": "app",
                    "dst_path": "/mnt/storage"
                }"#,
                result: false,
                message: "shared_mount: field 'src_path' couldn't be empty.",
            },
            TestData {
                shared_mount_annotation: r#"
                {
                    "name": "test",
                    "src_ctr": "sidecar",
                    "src_path": "/mnt/storage",
                    "dst_container": "app",
                    "dst_path": "/mnt/storage"
                }"#,
                result: false,
                message: "shared_mount: field 'dst_ctr' couldn't be empty.",
            },
            TestData {
                shared_mount_annotation: r#"
                {
                    "name": "test",
                    "src_ctr": "sidecar",
                    "src_path": "/mnt/storage",
                    "dst_ctr": "app",
                    "path": "/mnt/storage"
                }"#,
                result: false,
                message: "shared_mount: field 'dst_path' couldn't be empty.",
            },
            TestData {
                shared_mount_annotation: r#"
                {
                    "name": "test",
                    "src_ctr": "sidecar",
                    "src_path": "/_._/._/_/._",
                    "dst_ctr": "app",
                    "dst_path": "/-.-/.-/-/.-"
                }"#,
                result: true,
                message: "",
            },
            TestData {
                shared_mount_annotation: r#"
                {
                    "name": "test",
                    "src_ctr": "sidecar",
                    "src_path": "~/storage",
                    "dst_ctr": "app",
                    "dst_path": "/mnt/storage"
                }"#,
                result: false,
                message: "shared_mount 'test': src_path is invalid. It must be an absolute path and can only contain letters, numbers, hyphens(-), underscores(_) and dots(.).",
            },
            TestData {
                shared_mount_annotation: r#"
                {
                    "name": "test",
                    "src_ctr": "sidecar",
                    "src_path": "/mnt/storage",
                    "dst_ctr": "app",
                    "dst_path": "/mnt/storage|ls"
                }"#,
                result: false,
                message: "shared_mount 'test': dst_path is invalid. It must be an absolute path and can only contain letters, numbers, hyphens(-), underscores(_) and dots(.).",
            },
            TestData {
                shared_mount_annotation: r#"
                {
                    "name": "test",
                    "src_ctr": "sidecar",
                    "src_path": "/../mnt/storage",
                    "dst_ctr": "app",
                    "dst_path": "/mnt/storage"
                }"#,
                result: false,
                message: "shared_mount 'test': src_path couldn't contain '..' directory.",
            },
            TestData {
                shared_mount_annotation: r#"
                {
                    "name": "test",
                    "src_ctr": "sidecar",
                    "src_path": "/mnt/storage",
                    "dst_ctr": "app",
                    "dst_path": "/../mnt/storage"
                }"#,
                result: false,
                message: "shared_mount 'test': dst_path couldn't contain '..' directory.",
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let m: SharedMount = serde_json::from_str(d.shared_mount_annotation).unwrap();
            let result = m.validate();

            let msg = format!("{}, result: {:?}", msg, result);

            assert_eq!(result.is_ok(), d.result, "{}", msg);

            if !d.result {
                assert_eq!(result.unwrap_err().to_string(), d.message, "{}", msg);
            }
        }
    }
}
