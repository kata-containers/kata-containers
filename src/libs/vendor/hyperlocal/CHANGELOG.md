# 0.8.0

* upgrade to tokio 1.0 and hyper 0.14 [#44](https://github.com/softprops/hyperlocal/pull/44)
* port ci from Travis CI to GitHub Actions
* `main` is the new default GitHub branch

# 0.7.0

* reimplement server for `std::future` (`async`/`await`)
* upgrade to tokio 0.2
* add `SocketIncoming` interface

# 0.6.0

* upgrade to hyper 0.13
* upgrade hex to 0.3 [#15](https://github.com/softprops/hyperlocal/pull/15)
* move from tokio-core to tokio 0.1 [#16](https://github.com/softprops/hyperlocal/pull/16)
* don't explicitly block on unix socket connection [#18](https://github.com/softprops/hyperlocal/pull/18)
* provide a more flexible set of Server interfaces and to align more closely with those of hyper's default server bindings [#19](https://github.com/softprops/hyperlocal/pull/19)

You'll want to use `hyperlocal::server::Server` where you would have used `hyperlocal::server::Http` in the past and use
`hyperlocal::server::Http` for a lower level interfaces that give you more control over "driving" your server.

# 0.5.0

* upgrade to hyper 0.12 [#11](https://github.com/softprops/hyperlocal/pull/11)
* expose the [SocketAddr](https://doc.rust-lang.org/std/os/unix/net/struct.SocketAddr.html) servers listen on with `Server#local_addr`

# 0.4.1

* implement Clone for `UnixConnector` [@letmutx](https://github.com/softprops/hyperlocal/pull/7)

# 0.4.0

* refactor for async hyper
* `hyperlocal::DomainUrl` is now `hyperlocal::Uri` the semantics are the same but the name now matches hyper's new name can can be lifted into hypers type

```rust
let uri: hyper:Uri =
   hyperlocal::Uri(
     "path/to/server.sock",
     "/foo/bar?baz=boom"
   ).into();
```
* `hyperlocal::UnitSocketConnector` is now just `hyperlocal::UnixConnector` to be more inline with the naming conventions behind`hyper::HttpConnector` and `hyper_tls::HttpsConnector`
* `hyperlocal::UnixSocketServer` is now  `hyperlocal::server::Http` to be more inline with hyper naming conventions

# 0.3.0

* enable using unix_socket from stdlib. [#4](https://github.com/softprops/hyperlocal/pull/4)
* upgrade to hyper 0.10

# 0.2.0

* upgraded to hyper 0.9 and transitively url 1.0


# 0.1.0

Initial release
