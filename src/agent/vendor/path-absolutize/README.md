Path Absolutize
====================

[![Build Status](https://travis-ci.org/magiclen/path-absolutize.svg?branch=master)](https://travis-ci.org/magiclen/path-absolutize)

This is a library for extending `Path` and `PathBuf` in order to get an absolute path and remove the containing dots.

The difference between `absolutize` and `canonicalize` methods is that `absolutize` does not care about whether the file exists and what the file really is.

Please read the following examples to know the parsing rules.

## Examples

There are two methods you can use.

### absolutize

Get an absolute path.

The dots in a path will be parsed even if it is already an absolute path (which means the path starts with a `MAIN_SEPARATOR` on Unix-like systems).
    
```rust
extern crate path_absolutize;
    
use std::path::Path;
    
use path_absolutize::*;
    
let p = Path::new("/path/to/123/456");
    
assert_eq!("/path/to/123/456", p.absolutize().unwrap().to_str().unwrap());
```
    
```rust
extern crate path_absolutize;
    
use std::path::Path;
    
use path_absolutize::*;
    
let p = Path::new("/path/to/./123/../456");
    
assert_eq!("/path/to/456", p.absolutize().unwrap().to_str().unwrap());
```
    
If a path starts with a single dot, the dot means **current working directory**. 
    
```rust
extern crate path_absolutize;
    
use std::path::Path;
use std::env;
    
use path_absolutize::*;
    
let p = Path::new("./path/to/123/456");
    
assert_eq!(Path::join(env::current_dir().unwrap().as_path(), Path::new("path/to/123/456")).to_str().unwrap(), p.absolutize().unwrap().to_str().unwrap());
```


If a path starts with a pair of dots, the dots means the parent of **current working directory**. If **current working directory** is **root**, the parent is still **root**.

```rust
extern crate path_absolutize;
    
use std::path::Path;
use std::env;
    
use path_absolutize::*;
    
let p = Path::new("../path/to/123/456");

let cwd = env::current_dir().unwrap();
    
let cwd_parent = cwd.parent();
    
match cwd_parent {
   Some(cwd_parent) => {
       assert_eq!(Path::join(&cwd_parent, Path::new("path/to/123/456")).to_str().unwrap(), p.absolutize().unwrap().to_str().unwrap());
   }
   None => {
       assert_eq!(Path::join(Path::new("/"), Path::new("path/to/123/456")).to_str().unwrap(), p.absolutize().unwrap().to_str().unwrap());
   }
}
```

A path which does not start with a `MAIN_SEPARATOR`, **Single Dot** and **Double Dots**, will act like having a single dot at the start when `absolutize` method is used.
    
```rust
extern crate path_absolutize;
    
use std::path::Path;
use std::env;
    
use path_absolutize::*;
    
let p = Path::new("path/to/123/456");
    
assert_eq!(Path::join(env::current_dir().unwrap().as_path(), Path::new("path/to/123/456")).to_str().unwrap(), p.absolutize().unwrap().to_str().unwrap());
```
    
```rust
extern crate path_absolutize;
    
use std::path::Path;
use std::env;
    
use path_absolutize::*;
    
let p = Path::new("path/../../to/123/456");

let cwd = env::current_dir().unwrap();
    
let cwd_parent = cwd.parent();
    
match cwd_parent {
   Some(cwd_parent) => {
       assert_eq!(Path::join(&cwd_parent, Path::new("to/123/456")).to_str().unwrap(), p.absolutize().unwrap().to_str().unwrap());
   }
   None => {
       assert_eq!(Path::join(Path::new("/"), Path::new("to/123/456")).to_str().unwrap(), p.absolutize().unwrap().to_str().unwrap());
   }
}
```

### absolutize_virtually

Get an absolute path **only under a specific directory**.

The dots in a path will be parsed even if it is already an absolute path (which means the path starts with a `MAIN_SEPARATOR` on Unix-like systems).
    
```rust
extern crate path_absolutize;
    
use std::path::Path;
    
use path_absolutize::*;
    
let p = Path::new("/path/to/123/456");
    
assert_eq!("/path/to/123/456", p.absolutize_virtually("/").unwrap().to_str().unwrap());
```
    
```rust
extern crate path_absolutize;
    
use std::path::Path;
    
use path_absolutize::*;
    
let p = Path::new("/path/to/./123/../456");
    
assert_eq!("/path/to/456", p.absolutize_virtually("/").unwrap().to_str().unwrap());
```
    
Every absolute path should under the virtual root.
    
```rust
extern crate path_absolutize;
    
use std::path::Path;
    
use std::io::ErrorKind;
    
use path_absolutize::*;
    
let p = Path::new("/path/to/123/456");
    
assert_eq!(ErrorKind::InvalidInput, p.absolutize_virtually("/virtual/root").unwrap_err().kind());
```
    
Every relative path should under the virtual root.
    
```rust
extern crate path_absolutize;
    
use std::path::Path;
    
use std::io::ErrorKind;
    
use path_absolutize::*;
    
let p = Path::new("./path/to/123/456");
    
assert_eq!(ErrorKind::InvalidInput, p.absolutize_virtually("/virtual/root").unwrap_err().kind());
```
    
```rust
extern crate path_absolutize;
    
use std::path::Path;
    
use std::io::ErrorKind;
    
use path_absolutize::*;
    
let p = Path::new("../path/to/123/456");
    
assert_eq!(ErrorKind::InvalidInput, p.absolutize_virtually("/virtual/root").unwrap_err().kind());
```
    
A path which does not start with a `MAIN_SEPARATOR`, **Single Dot** and **Double Dots**, will be located in the virtual root after the `absolutize_virtually` method is used.
    
```rust
extern crate path_absolutize;
    
use std::path::Path;
    
use path_absolutize::*;
    
let p = Path::new("path/to/123/456");
    
assert_eq!("/virtual/root/path/to/123/456", p.absolutize_virtually("/virtual/root").unwrap().to_str().unwrap());
```
    
```rust
extern crate path_absolutize;
    
use std::path::Path;
    
use path_absolutize::*;
    
let p = Path::new("path/to/../../../../123/456");
    
assert_eq!("/virtual/root/123/456", p.absolutize_virtually("/virtual/root").unwrap().to_str().unwrap());
```

## Crates.io

https://crates.io/crates/path-absolutize

## Documentation

https://docs.rs/path-absolutize

## License

[MIT](LICENSE)