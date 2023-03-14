#[allow(clippy::blacklisted_name)]
#[cfg(all(unix, feature = "async-io"))]
use std::os::unix::net::UnixStream;
use std::{collections::HashMap, convert::TryInto};
#[cfg(all(unix, not(feature = "async-io")))]
use tokio::net::UnixStream;

use async_channel::{bounded, Sender};
use event_listener::Event;
use futures_util::StreamExt;
use ntest::timeout;
use serde::{Deserialize, Serialize};
use test_log::test;
use tracing::{debug, instrument};
use zbus::{
    block_on,
    fdo::{ObjectManager, ObjectManagerProxy},
    DBusError,
};
use zvariant::{DeserializeDict, OwnedValue, SerializeDict, Type, Value};

use zbus::{
    dbus_interface, dbus_proxy, CacheProperties, Connection, ConnectionBuilder, InterfaceRef,
    MessageHeader, MessageType, ObjectServer, SignalContext,
};

#[derive(Debug, Deserialize, Serialize, Type)]
pub struct ArgStructTest {
    foo: i32,
    bar: String,
}

// Mimic a NetworkManager interface property that's a dict. This tests ability to use a custom
// dict type using the `Type` And `*Dict` macros (issue #241).
#[derive(DeserializeDict, SerializeDict, Type, Debug, Value, OwnedValue, PartialEq, Eq)]
#[zvariant(signature = "dict")]
pub struct IP4Adress {
    prefix: u32,
    address: String,
}

#[dbus_proxy(gen_blocking = false)]
trait MyIface {
    fn ping(&self) -> zbus::Result<u32>;

    fn quit(&self) -> zbus::Result<()>;

    fn test_header(&self) -> zbus::Result<()>;

    fn test_error(&self) -> zbus::Result<()>;

    fn test_single_struct_arg(&self, arg: ArgStructTest) -> zbus::Result<()>;

    fn test_single_struct_ret(&self) -> zbus::Result<ArgStructTest>;

    fn test_multi_ret(&self) -> zbus::Result<(i32, String)>;

    fn test_hashmap_return(&self) -> zbus::Result<HashMap<String, String>>;

    fn create_obj(&self, key: &str) -> zbus::Result<()>;

    fn destroy_obj(&self, key: &str) -> zbus::Result<()>;

    #[dbus_proxy(property)]
    fn count(&self) -> zbus::Result<u32>;

    #[dbus_proxy(property)]
    fn set_count(&self, count: u32) -> zbus::Result<()>;

    #[dbus_proxy(property)]
    fn hash_map(&self) -> zbus::Result<HashMap<String, String>>;

    #[dbus_proxy(property)]
    fn address_data(&self) -> zbus::Result<IP4Adress>;

    #[dbus_proxy(property)]
    fn address_data2(&self) -> zbus::Result<IP4Adress>;
}

#[derive(Debug, Clone)]
enum NextAction {
    Quit,
    CreateObj(String),
    DestroyObj(String),
}

#[derive(Debug)]
struct MyIfaceImpl {
    next_tx: Sender<NextAction>,
    count: u32,
}

impl MyIfaceImpl {
    fn new(next_tx: Sender<NextAction>) -> Self {
        Self { next_tx, count: 0 }
    }
}

/// Custom D-Bus error type.
#[derive(Debug, DBusError)]
#[dbus_error(prefix = "org.freedesktop.MyIface.Error")]
enum MyIfaceError {
    SomethingWentWrong(String),
    #[dbus_error(zbus_error)]
    ZBus(zbus::Error),
}

#[dbus_interface(interface = "org.freedesktop.MyIface")]
impl MyIfaceImpl {
    #[instrument]
    async fn ping(&mut self, #[zbus(signal_context)] ctxt: SignalContext<'_>) -> u32 {
        self.count += 1;
        if self.count % 3 == 0 {
            MyIfaceImpl::alert_count(&ctxt, self.count)
                .await
                .expect("Failed to emit signal");
            debug!("emitted `AlertCount` signal.");
        } else {
            debug!("Didn't emit `AlertCount` signal.");
        }
        self.count
    }

    #[instrument]
    async fn quit(&self) {
        debug!("Client asked to quit.");
        self.next_tx.send(NextAction::Quit).await.unwrap();
    }

    #[instrument]
    fn test_header(&self, #[zbus(header)] header: MessageHeader<'_>) {
        debug!("`TestHeader` called.");
        assert_eq!(header.message_type().unwrap(), MessageType::MethodCall);
        assert_eq!(header.member().unwrap().unwrap(), "TestHeader");
    }

    #[instrument]
    fn test_error(&self) -> zbus::fdo::Result<()> {
        debug!("`TestError` called.");
        Err(zbus::fdo::Error::Failed("error raised".to_string()))
    }

    #[instrument]
    fn test_custom_error(&self) -> Result<(), MyIfaceError> {
        debug!("`TestCustomError` called.");
        Err(MyIfaceError::SomethingWentWrong("oops".to_string()))
    }

    #[instrument]
    fn test_single_struct_arg(
        &self,
        arg: ArgStructTest,
        #[zbus(header)] header: MessageHeader<'_>,
    ) -> zbus::fdo::Result<()> {
        debug!("`TestSingleStructArg` called.");
        assert_eq!(header.signature()?.unwrap(), "(is)");
        assert_eq!(arg.foo, 1);
        assert_eq!(arg.bar, "TestString");

        Ok(())
    }

    #[instrument]
    fn test_single_struct_ret(&self) -> zbus::fdo::Result<ArgStructTest> {
        debug!("`TestSingleStructRet` called.");
        Ok(ArgStructTest {
            foo: 42,
            bar: String::from("Meaning of life"),
        })
    }

    #[instrument]
    #[dbus_interface(out_args("foo", "bar"))]
    fn test_multi_ret(&self) -> zbus::fdo::Result<(i32, String)> {
        debug!("`TestMultiRet` called.");
        Ok((42, String::from("Meaning of life")))
    }

    #[instrument]
    async fn test_hashmap_return(&self) -> zbus::fdo::Result<HashMap<String, String>> {
        debug!("`TestHashmapReturn` called.");
        let mut map = HashMap::new();
        map.insert("hi".into(), "hello".into());
        map.insert("bye".into(), "now".into());

        Ok(map)
    }

    #[instrument]
    async fn create_obj(&self, key: String) {
        debug!("`CreateObj` called.");
        self.next_tx.send(NextAction::CreateObj(key)).await.unwrap();
    }

    #[instrument]
    async fn create_obj_inside(
        &self,
        #[zbus(object_server)] object_server: &ObjectServer,
        key: String,
    ) {
        debug!("`CreateObjInside` called.");
        object_server
            .at(
                format!("/zbus/test/{}", key),
                MyIfaceImpl::new(self.next_tx.clone()),
            )
            .await
            .unwrap();
    }

    #[instrument]
    async fn destroy_obj(&self, key: String) {
        debug!("`DestroyObj` called.");
        self.next_tx
            .send(NextAction::DestroyObj(key))
            .await
            .unwrap();
    }

    #[instrument]
    #[dbus_interface(property)]
    fn set_count(&mut self, val: u32) -> zbus::fdo::Result<()> {
        debug!("`Count` setter called.");
        if val == 42 {
            return Err(zbus::fdo::Error::InvalidArgs("Tsss tsss!".to_string()));
        }
        self.count = val;
        Ok(())
    }

    #[instrument]
    #[dbus_interface(property)]
    fn count(&self) -> u32 {
        debug!("`Count` getter called.");
        self.count
    }

    #[instrument]
    #[dbus_interface(property)]
    async fn hash_map(&self) -> HashMap<String, String> {
        debug!("`HashMap` getter called.");
        self.test_hashmap_return().await.unwrap()
    }

    #[instrument]
    #[dbus_interface(property)]
    fn address_data(&self) -> IP4Adress {
        debug!("`AddressData` getter called.");
        IP4Adress {
            address: "127.0.0.1".to_string(),
            prefix: 1234,
        }
    }

    // On the bus, this should return the same value as address_data above. We want to test if
    // this works both ways.
    #[instrument]
    #[dbus_interface(property)]
    fn address_data2(&self) -> HashMap<String, OwnedValue> {
        debug!("`AddressData2` getter called.");
        let mut map = HashMap::new();
        map.insert("address".into(), Value::from("127.0.0.1").into());
        map.insert("prefix".into(), 1234u32.into());

        map
    }

    #[dbus_interface(signal)]
    async fn alert_count(ctxt: &SignalContext<'_>, val: u32) -> zbus::Result<()>;
}

fn check_hash_map(map: HashMap<String, String>) {
    assert_eq!(map["hi"], "hello");
    assert_eq!(map["bye"], "now");
}

fn check_ipv4_address(address: IP4Adress) {
    assert_eq!(
        address,
        IP4Adress {
            address: "127.0.0.1".to_string(),
            prefix: 1234,
        }
    );
}

#[instrument]
async fn my_iface_test(conn: Connection, event: Event) -> zbus::Result<u32> {
    debug!("client side starting..");
    let proxy = MyIfaceProxy::builder(&conn)
        .destination("org.freedesktop.MyService")?
        .path("/org/freedesktop/MyService")?
        // the server isn't yet running
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
    debug!("Created: {:?}", proxy);
    let props_proxy = zbus::fdo::PropertiesProxy::builder(&conn)
        .destination("org.freedesktop.MyService")?
        .path("/org/freedesktop/MyService")?
        .build()
        .await?;
    debug!("Created: {:?}", props_proxy);

    let mut props_changed_stream = props_proxy.receive_properties_changed().await?;
    debug!("Created: {:?}", props_changed_stream);
    event.notify(1);
    debug!("Notified service that client is ready");

    match props_changed_stream.next().await {
        Some(changed) => {
            assert_eq!(
                *changed.args()?.changed_properties().keys().next().unwrap(),
                "Count"
            );
        }
        None => panic!(""),
    };
    drop(props_changed_stream);

    proxy.ping().await?;
    assert_eq!(proxy.count().await?, 1);
    assert_eq!(proxy.cached_count()?, None);

    proxy.test_header().await?;
    proxy
        .test_single_struct_arg(ArgStructTest {
            foo: 1,
            bar: "TestString".into(),
        })
        .await?;
    check_hash_map(proxy.test_hashmap_return().await?);
    check_hash_map(proxy.hash_map().await?);
    check_ipv4_address(proxy.address_data().await?);
    check_ipv4_address(proxy.address_data2().await?);

    #[cfg(feature = "xml")]
    {
        let xml = proxy.introspect().await?;
        debug!("Introspection: {}", xml);
        let node = zbus::xml::Node::from_reader(xml.as_bytes())?;
        let ifaces = node.interfaces();
        let iface = ifaces
            .iter()
            .find(|i| i.name() == "org.freedesktop.MyIface")
            .unwrap();
        let methods = iface.methods();
        for method in methods {
            if method.name() != "TestSingleStructRet" && method.name() != "TestMultiRet" {
                continue;
            }
            let args = method.args();
            let mut out_args = args.iter().filter(|a| a.direction().unwrap() == "out");

            if method.name() == "TestSingleStructRet" {
                assert_eq!(args.len(), 1);
                assert_eq!(out_args.next().unwrap().ty(), "(is)");
                assert!(out_args.next().is_none());
            } else {
                assert_eq!(args.len(), 2);
                let foo = out_args.find(|a| a.name() == Some("foo")).unwrap();
                assert_eq!(foo.ty(), "i");
                let bar = out_args.find(|a| a.name() == Some("bar")).unwrap();
                assert_eq!(bar.ty(), "s");
            }
        }
    }
    // build-time check to see if macro is doing the right thing.
    let _ = proxy.test_single_struct_ret().await?.foo;
    let _ = proxy.test_multi_ret().await?.1;

    let val = proxy.ping().await?;

    let obj_manager_proxy = ObjectManagerProxy::builder(&conn)
        .destination("org.freedesktop.MyService")?
        .path("/zbus/test")?
        .build()
        .await?;
    debug!("Created: {:?}", obj_manager_proxy);
    let mut ifaces_added_stream = obj_manager_proxy.receive_interfaces_added().await?;
    debug!("Created: {:?}", ifaces_added_stream);

    // Must process in parallel, so the stream listener does not block receiving
    // the method return message.
    let (ifaces_added, _) = futures_util::future::join(
        async {
            let ret = ifaces_added_stream.next().await.unwrap();
            drop(ifaces_added_stream);
            ret
        },
        async {
            proxy.create_obj("MyObj").await.unwrap();
        },
    )
    .await;

    assert_eq!(ifaces_added.args()?.object_path(), "/zbus/test/MyObj");
    let args = ifaces_added.args()?;
    let ifaces = args.interfaces_and_properties();
    let _ = ifaces.get("org.freedesktop.MyIface").unwrap();
    // TODO: Check if the properties are correct.

    // issue#207: interface panics on incorrect number of args.
    assert!(proxy.call_method("CreateObj", &()).await.is_err());

    let my_obj_proxy = MyIfaceProxy::builder(&conn)
        .destination("org.freedesktop.MyService")?
        .path("/zbus/test/MyObj")?
        .build()
        .await?;
    debug!("Created: {:?}", my_obj_proxy);
    my_obj_proxy.receive_count_changed().await;
    // Calling this after creating the stream was panicking if the property doesn't get cached
    // before the call (MR !460).
    my_obj_proxy.cached_count()?;
    assert_eq!(my_obj_proxy.count().await?, 0);
    assert_eq!(my_obj_proxy.cached_count()?, Some(0));
    assert_eq!(
        my_obj_proxy.cached_property_raw("Count").as_deref(),
        Some(&Value::from(0u32))
    );
    my_obj_proxy.ping().await?;

    let mut ifaces_removed_stream = obj_manager_proxy.receive_interfaces_removed().await?;
    debug!("Created: {:?}", ifaces_removed_stream);
    // Must process in parallel, so the stream listener does not block receiving
    // the method return message.
    let (ifaces_removed, _) = futures_util::future::join(
        async {
            let ret = ifaces_removed_stream.next().await.unwrap();
            drop(ifaces_removed_stream);
            ret
        },
        async {
            proxy.destroy_obj("MyObj").await.unwrap();
        },
    )
    .await;

    let args = ifaces_removed.args()?;
    assert_eq!(args.object_path(), "/zbus/test/MyObj");
    assert_eq!(args.interfaces(), &["org.freedesktop.MyIface"]);

    assert!(my_obj_proxy.introspect().await.is_err());
    assert!(my_obj_proxy.ping().await.is_err());

    // Make sure methods modifying the ObjectServer can be called without
    // deadlocks.
    proxy
        .call_method("CreateObjInside", &("CreatedInside"))
        .await?;
    let created_inside_proxy = MyIfaceProxy::builder(&conn)
        .destination("org.freedesktop.MyService")?
        .path("/zbus/test/CreatedInside")?
        .build()
        .await?;
    created_inside_proxy.ping().await?;
    proxy.destroy_obj("CreatedInside").await?;

    proxy.quit().await?;
    Ok(val)
}

#[test]
#[timeout(15000)]
fn iface_and_proxy() {
    block_on(iface_and_proxy_(false));
}

#[cfg(unix)]
#[test]
#[timeout(15000)]
fn iface_and_proxy_unix_p2p() {
    block_on(iface_and_proxy_(true));
}

#[instrument]
async fn iface_and_proxy_(p2p: bool) {
    let event = event_listener::Event::new();
    let guid = zbus::Guid::generate();

    let (service_conn_builder, client_conn_builder) = if p2p {
        #[cfg(unix)]
        {
            let (p0, p1) = UnixStream::pair().unwrap();

            (
                ConnectionBuilder::unix_stream(p0).server(&guid).p2p(),
                ConnectionBuilder::unix_stream(p1).p2p(),
            )
        }

        #[cfg(windows)]
        {
            #[cfg(feature = "async-io")]
            {
                let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
                let addr = listener.local_addr().unwrap();
                let p1 = std::net::TcpStream::connect(addr).unwrap();
                let p0 = listener.incoming().next().unwrap().unwrap();

                (
                    ConnectionBuilder::tcp_stream(p0).server(&guid).p2p(),
                    ConnectionBuilder::tcp_stream(p1).p2p(),
                )
            }

            #[cfg(not(feature = "async-io"))]
            {
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                let addr = listener.local_addr().unwrap();
                let p1 = tokio::net::TcpStream::connect(addr).await.unwrap();
                let p0 = listener.accept().await.unwrap().0;

                (
                    ConnectionBuilder::tcp_stream(p0).server(&guid).p2p(),
                    ConnectionBuilder::tcp_stream(p1).p2p(),
                )
            }
        }
    } else {
        let service_conn_builder = ConnectionBuilder::session()
            .unwrap()
            .name("org.freedesktop.MyService")
            .unwrap()
            .name("org.freedesktop.MyService.foo")
            .unwrap()
            .name("org.freedesktop.MyService.bar")
            .unwrap();
        let client_conn_builder = ConnectionBuilder::session().unwrap();

        (service_conn_builder, client_conn_builder)
    };
    debug!(
        "Client connection builder created: {:?}",
        client_conn_builder
    );
    debug!(
        "Service connection builder created: {:?}",
        service_conn_builder
    );
    let (next_tx, next_rx) = bounded(64);
    let iface = MyIfaceImpl::new(next_tx.clone());
    let service_conn_builder = service_conn_builder
        .serve_at("/org/freedesktop/MyService", iface)
        .unwrap()
        .serve_at("/zbus/test", ObjectManager)
        .unwrap();
    debug!("ObjectServer set-up.");

    let (service_conn, client_conn) =
        futures_util::try_join!(service_conn_builder.build(), client_conn_builder.build(),)
            .unwrap();
    debug!("Client connection created: {:?}", client_conn);
    debug!("Service connection created: {:?}", service_conn);

    let listen = event.listen();
    let child = async_std::task::spawn(my_iface_test(client_conn.clone(), event));
    debug!("Child task spawned.");
    // Wait for the listener to be ready
    listen.await;
    debug!("Child task signaled it's ready.");

    let iface: InterfaceRef<MyIfaceImpl> = service_conn
        .object_server()
        .interface("/org/freedesktop/MyService")
        .await
        .unwrap();
    iface
        .get()
        .await
        .count_changed(iface.signal_context())
        .await
        .unwrap();
    debug!("`PropertiesChanged` emitted for `Count` property.");

    loop {
        MyIfaceImpl::alert_count(iface.signal_context(), 51)
            .await
            .unwrap();
        debug!("`AlertCount` signal emitted.");

        match next_rx.recv().await.unwrap() {
            NextAction::Quit => break,
            NextAction::CreateObj(key) => {
                let path = format!("/zbus/test/{}", key);
                service_conn
                    .object_server()
                    .at(path.clone(), MyIfaceImpl::new(next_tx.clone()))
                    .await
                    .unwrap();
                debug!("Object `{path}` added.");
            }
            NextAction::DestroyObj(key) => {
                let path = format!("/zbus/test/{}", key);
                service_conn
                    .object_server()
                    .remove::<MyIfaceImpl, _>(path.clone())
                    .await
                    .unwrap();
                debug!("Object `{path}` removed.");
            }
        }
    }
    debug!("Server done.");

    // don't close the connection before we end the loop
    drop(client_conn);
    debug!("Connection closed.");

    let val = child.await.unwrap();
    debug!("Client task done.");
    assert_eq!(val, 2);

    if p2p {
        debug!("p2p connection, no need to release names..");
        return;
    }

    // Release primary name explicitly and let others be released implicitly.
    assert_eq!(
        service_conn.release_name("org.freedesktop.MyService").await,
        Ok(true)
    );
    debug!("Bus name `org.freedesktop.MyService` released.");
    assert_eq!(
        service_conn
            .release_name("org.freedesktop.MyService.foo")
            .await,
        Ok(true)
    );
    debug!("Bus name `org.freedesktop.MyService.foo` released.");
    assert_eq!(
        service_conn
            .release_name("org.freedesktop.MyService.bar")
            .await,
        Ok(true)
    );
    debug!("Bus name `org.freedesktop.MyService.bar` released.");

    // Let's ensure all names were released.
    let proxy = zbus::fdo::DBusProxy::new(&service_conn).await.unwrap();
    debug!("DBusProxy created to ensure all names were released.");
    assert_eq!(
        proxy
            .name_has_owner("org.freedesktop.MyService".try_into().unwrap())
            .await,
        Ok(false)
    );
    assert_eq!(
        proxy
            .name_has_owner("org.freedesktop.MyService.foo".try_into().unwrap())
            .await,
        Ok(false)
    );
    assert_eq!(
        proxy
            .name_has_owner("org.freedesktop.MyService.bar".try_into().unwrap())
            .await,
        Ok(false)
    );
    debug!("Bus confirmed that all names were definitely released.");
}
