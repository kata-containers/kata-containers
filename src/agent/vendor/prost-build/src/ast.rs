use prost_types;
use prost_types::source_code_info::Location;

/// Comments on a Protobuf item.
#[derive(Debug)]
pub struct Comments {
    /// Leading detached blocks of comments.
    pub leading_detached: Vec<Vec<String>>,

    /// Leading comments.
    pub leading: Vec<String>,

    /// Trailing comments.
    pub trailing: Vec<String>,
}

impl Comments {
    pub(crate) fn from_location(location: &Location) -> Comments {
        fn get_lines<S>(comments: S) -> Vec<String>
        where
            S: AsRef<str>,
        {
            comments.as_ref().lines().map(str::to_owned).collect()
        }

        let leading_detached = location
            .leading_detached_comments
            .iter()
            .map(get_lines)
            .collect();
        let leading = location
            .leading_comments
            .as_ref()
            .map_or(Vec::new(), get_lines);
        let trailing = location
            .trailing_comments
            .as_ref()
            .map_or(Vec::new(), get_lines);
        Comments {
            leading_detached: leading_detached,
            leading: leading,
            trailing: trailing,
        }
    }

    /// Appends the comments to a buffer with indentation.
    ///
    /// Each level of indentation corresponds to four space (' ') characters.
    pub fn append_with_indent(&self, indent_level: u8, buf: &mut String) {
        // Append blocks of detached comments.
        for detached_block in &self.leading_detached {
            for line in detached_block {
                for _ in 0..indent_level {
                    buf.push_str("    ");
                }
                buf.push_str("//");
                buf.push_str(line);
                buf.push_str("\n");
            }
            buf.push_str("\n");
        }

        // Append leading comments.
        for line in &self.leading {
            for _ in 0..indent_level {
                buf.push_str("    ");
            }
            buf.push_str("///");
            buf.push_str(line);
            buf.push_str("\n");
        }

        // Append an empty comment line if there are leading and trailing comments.
        if !self.leading.is_empty() && !self.trailing.is_empty() {
            for _ in 0..indent_level {
                buf.push_str("    ");
            }
            buf.push_str("///\n");
        }

        // Append trailing comments.
        for line in &self.trailing {
            for _ in 0..indent_level {
                buf.push_str("    ");
            }
            buf.push_str("///");
            buf.push_str(line);
            buf.push_str("\n");
        }
    }
}

/// A service descriptor.
#[derive(Debug)]
pub struct Service {
    /// The service name in Rust style.
    pub name: String,
    /// The service name as it appears in the .proto file.
    pub proto_name: String,
    /// The package name as it appears in the .proto file.
    pub package: String,
    /// The service comments.
    pub comments: Comments,
    /// The service methods.
    pub methods: Vec<Method>,
    /// The service options.
    pub options: prost_types::ServiceOptions,
}

/// A service method descriptor.
#[derive(Debug)]
pub struct Method {
    /// The name of the method in Rust style.
    pub name: String,
    /// The name of the method as it appears in the .proto file.
    pub proto_name: String,
    /// The method comments.
    pub comments: Comments,
    /// The input Rust type.
    pub input_type: String,
    /// The output Rust type.
    pub output_type: String,
    /// The input Protobuf type.
    pub input_proto_type: String,
    /// The output Protobuf type.
    pub output_proto_type: String,
    /// The method options.
    pub options: prost_types::MethodOptions,
    /// Identifies if client streams multiple client messages.
    pub client_streaming: bool,
    /// Identifies if server streams multiple server messages.
    pub server_streaming: bool,
}
