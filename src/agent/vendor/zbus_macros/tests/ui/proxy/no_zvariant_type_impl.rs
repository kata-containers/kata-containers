use serde::{Deserialize, Serialize};
use zbus::fdo;
use zbus_macros::dbus_proxy;

#[derive(Deserialize, Serialize)]
struct Foo;

#[dbus_proxy(
    interface = "org.freedesktop.zbus.Test",
    default_service = "org.freedesktop.zbus",
    default_path = "/org/freedesktop/zbus/test"
)]
trait Test {
    fn invalid_arg(&self, arg: Foo) -> zbus::Result<()>;

    fn invalid_result(&self) -> zbus::Result<Foo>;

    #[dbus_proxy(property)]
    fn invalid_property(&self) -> fdo::Result<Foo>;
}

fn main() {}
