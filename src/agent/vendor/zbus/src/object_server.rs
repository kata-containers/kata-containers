use async_lock::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::{
    collections::{hash_map::Entry, HashMap},
    convert::TryInto,
    fmt::Write,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    sync::Arc,
};

use static_assertions::assert_impl_all;
use zbus_names::InterfaceName;
use zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value};

use crate::{
    fdo,
    fdo::{Introspectable, ManagedObjects, ObjectManager, Peer, Properties},
    Connection, DispatchResult, Error, Interface, Message, MessageType, Result, SignalContext,
    WeakConnection,
};

/// Opaque structure that derefs to an `Interface` type.
pub struct InterfaceDeref<'d, I> {
    iface: RwLockReadGuard<'d, dyn Interface>,
    phantom: PhantomData<I>,
}

impl<I> Deref for InterfaceDeref<'_, I>
where
    I: Interface,
{
    type Target = I;

    fn deref(&self) -> &I {
        self.iface.downcast_ref::<I>().unwrap()
    }
}

/// Opaque structure that mutably derefs to an `Interface` type.
pub struct InterfaceDerefMut<'d, I> {
    iface: RwLockWriteGuard<'d, dyn Interface>,
    phantom: PhantomData<I>,
}

impl<I> Deref for InterfaceDerefMut<'_, I>
where
    I: Interface,
{
    type Target = I;

    fn deref(&self) -> &I {
        self.iface.downcast_ref::<I>().unwrap()
    }
}

impl<I> DerefMut for InterfaceDerefMut<'_, I>
where
    I: Interface,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.iface.downcast_mut::<I>().unwrap()
    }
}

/// Wrapper over an interface, along with its corresponding `SignalContext`
/// instance. A reference to the underlying interface may be obtained via
/// [`InterfaceRef::get`] and [`InterfaceRef::get_mut`].
pub struct InterfaceRef<I> {
    ctxt: SignalContext<'static>,
    lock: Arc<RwLock<dyn Interface>>,
    phantom: PhantomData<I>,
}

impl<I> InterfaceRef<I>
where
    I: 'static,
{
    /// Get a reference to the underlying interface.
    ///
    /// **WARNING:** If methods (e.g property setters) in `ObjectServer` require `&mut self`
    /// `ObjectServer` will not be able to access the interface in question until all references
    /// of this method are dropped, it is highly recommended that the scope of the interface
    /// returned is restricted.
    ///
    pub async fn get(&self) -> InterfaceDeref<'_, I> {
        let iface = self.lock.read().await;

        iface
            .downcast_ref::<I>()
            .expect("Unexpected interface type");

        InterfaceDeref {
            iface,
            phantom: PhantomData,
        }
    }

    /// Get a reference to the underlying interface.
    ///
    /// **WARNINGS:** Since the `ObjectServer` will not be able to access the interface in question
    /// until the return value of this method is dropped, it is highly recommended that the scope
    /// of the interface returned is restricted.
    ///
    /// # Errors
    ///
    /// If the interface at this instance's path is not valid, `Error::InterfaceNotFound` error is
    /// returned.
    ///
    /// # Examples
    ///
    /// ```no_run
    ///# use std::error::Error;
    ///# use async_io::block_on;
    ///# use zbus::{Connection, ObjectServer, SignalContext, dbus_interface};
    ///
    /// struct MyIface(u32);
    ///
    /// #[dbus_interface(name = "org.myiface.MyIface")]
    /// impl MyIface {
    ///    #[dbus_interface(property)]
    ///    async fn count(&self) -> u32 {
    ///        self.0
    ///    }
    /// }
    ///
    ///# block_on(async {
    /// // Setup connection and object_server etc here and then in another part of the code:
    ///# let connection = Connection::session().await?;
    ///#
    ///# let path = "/org/zbus/path";
    ///# connection.object_server().at(path, MyIface(22)).await?;
    /// let mut object_server = connection.object_server();
    /// let iface_ref = object_server.interface::<_, MyIface>(path).await?;
    /// let mut iface = iface_ref.get_mut().await;
    /// iface.0 = 42;
    /// iface.count_changed(iface_ref.signal_context()).await?;
    ///# Ok::<_, Box<dyn Error + Send + Sync>>(())
    ///# })?;
    ///#
    ///# Ok::<_, Box<dyn Error + Send + Sync>>(())
    /// ```
    pub async fn get_mut(&self) -> InterfaceDerefMut<'_, I> {
        let mut iface = self.lock.write().await;

        iface
            .downcast_ref::<I>()
            .expect("Unexpected interface type");
        iface
            .downcast_mut::<I>()
            .expect("Unexpected interface type");

        InterfaceDerefMut {
            iface,
            phantom: PhantomData,
        }
    }

    pub fn signal_context(&self) -> &SignalContext<'static> {
        &self.ctxt
    }
}

#[derive(Default, derivative::Derivative)]
#[derivative(Debug)]
pub(crate) struct Node {
    path: OwnedObjectPath,
    children: HashMap<String, Node>,
    #[derivative(Debug = "ignore")]
    interfaces: HashMap<InterfaceName<'static>, Arc<RwLock<dyn Interface>>>,
}

impl Node {
    pub(crate) fn new(path: OwnedObjectPath) -> Self {
        let mut node = Self {
            path,
            ..Default::default()
        };
        node.at(Peer::name(), || Arc::new(RwLock::new(Peer)));
        node.at(Introspectable::name(), || {
            Arc::new(RwLock::new(Introspectable))
        });
        node.at(Properties::name(), || Arc::new(RwLock::new(Properties)));

        node
    }

    // Get the child Node at path.
    pub(crate) fn get_child(&self, path: &ObjectPath<'_>) -> Option<&Node> {
        let mut node = self;

        for i in path.split('/').skip(1) {
            if i.is_empty() {
                continue;
            }
            match node.children.get(i) {
                Some(n) => node = n,
                None => return None,
            }
        }

        Some(node)
    }

    // Get the child Node at path. Optionally create one if it doesn't exist.
    // It also returns the path of parent node that implements ObjectManager (if any). If multiple
    // parents implement it (they shouldn't), then the closest one is returned.
    fn get_child_mut(
        &mut self,
        path: &ObjectPath<'_>,
        create: bool,
    ) -> (Option<&mut Node>, Option<ObjectPath<'_>>) {
        let mut node = self;
        let mut node_path = String::new();
        let mut obj_manager_path = None;

        for i in path.split('/').skip(1) {
            if i.is_empty() {
                continue;
            }

            if node.interfaces.contains_key(&ObjectManager::name()) {
                obj_manager_path = Some((*node.path).clone());
            }

            write!(&mut node_path, "/{}", i).unwrap();
            match node.children.entry(i.into()) {
                Entry::Vacant(e) => {
                    if create {
                        let path = node_path.as_str().try_into().expect("Invalid Object Path");
                        node = e.insert(Node::new(path));
                    } else {
                        return (None, obj_manager_path);
                    }
                }
                Entry::Occupied(e) => node = e.into_mut(),
            }
        }

        (Some(node), obj_manager_path)
    }

    pub(crate) fn interface_lock(
        &self,
        interface_name: InterfaceName<'_>,
    ) -> Option<Arc<RwLock<dyn Interface>>> {
        self.interfaces.get(&interface_name).cloned()
    }

    fn remove_interface(&mut self, interface_name: InterfaceName<'static>) -> bool {
        self.interfaces.remove(&interface_name).is_some()
    }

    fn is_empty(&self) -> bool {
        !self.interfaces.keys().any(|k| {
            *k != Peer::name()
                && *k != Introspectable::name()
                && *k != Properties::name()
                && *k != ObjectManager::name()
        })
    }

    fn remove_node(&mut self, node: &str) -> bool {
        self.children.remove(node).is_some()
    }

    // Takes a closure so caller can avoid having to create an Arc & RwLock in case interface was
    // already added.
    fn at<F>(&mut self, name: InterfaceName<'static>, iface_creator: F) -> bool
    where
        F: FnOnce() -> Arc<RwLock<dyn Interface>>,
    {
        match self.interfaces.entry(name) {
            Entry::Vacant(e) => e.insert(iface_creator()),
            Entry::Occupied(_) => return false,
        };

        true
    }

    #[async_recursion::async_recursion]
    async fn introspect_to_writer<W: Write + Send>(&self, writer: &mut W, level: usize) {
        if level == 0 {
            writeln!(
                writer,
                r#"
<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
<node>"#
            )
            .unwrap();
        }

        for iface in self.interfaces.values() {
            iface.read().await.introspect_to_writer(writer, level + 2);
        }

        for (path, node) in &self.children {
            let level = level + 2;
            writeln!(
                writer,
                "{:indent$}<node name=\"{}\">",
                "",
                path,
                indent = level
            )
            .unwrap();
            node.introspect_to_writer(writer, level).await;
            writeln!(writer, "{:indent$}</node>", "", indent = level).unwrap();
        }

        if level == 0 {
            writeln!(writer, "</node>").unwrap();
        }
    }

    pub(crate) async fn introspect(&self) -> String {
        let mut xml = String::with_capacity(1024);

        self.introspect_to_writer(&mut xml, 0).await;

        xml
    }

    #[async_recursion::async_recursion]
    pub(crate) async fn get_managed_objects(&self) -> ManagedObjects {
        // Recursively get all properties of all interfaces of descendants.
        let mut managed_objects = ManagedObjects::new();
        for node in self.children.values() {
            let mut interfaces = HashMap::new();
            for iface_name in node.interfaces.keys().filter(|n| {
                // Filter standard interfaces.
                *n != &Peer::name()
                    && *n != &Introspectable::name()
                    && *n != &Properties::name()
                    && *n != &ObjectManager::name()
            }) {
                let props = node.get_properties(iface_name.clone()).await;
                interfaces.insert(iface_name.clone().into(), props);
            }
            managed_objects.insert(node.path.clone(), interfaces);
            managed_objects.extend(node.get_managed_objects().await);
        }

        managed_objects
    }

    async fn get_properties(
        &self,
        interface_name: InterfaceName<'_>,
    ) -> HashMap<String, OwnedValue> {
        self.interface_lock(interface_name)
            .expect("Interface was added but not found")
            .read()
            .await
            .get_all()
            .await
    }
}

/// An object server, holding server-side D-Bus objects & interfaces.
///
/// Object servers hold interfaces on various object paths, and expose them over D-Bus.
///
/// All object paths will have the standard interfaces implemented on your behalf, such as
/// `org.freedesktop.DBus.Introspectable` or `org.freedesktop.DBus.Properties`.
///
/// # Example
///
/// This example exposes the `org.myiface.Example.Quit` method on the `/org/zbus/path`
/// path.
///
/// ```no_run
///# use std::error::Error;
/// use zbus::{Connection, ObjectServer, dbus_interface};
/// use std::sync::Arc;
/// use event_listener::Event;
///# use async_io::block_on;
///
/// struct Example {
///     // Interfaces are owned by the ObjectServer. They can have
///     // `&mut self` methods.
///     quit_event: Event,
/// }
///
/// impl Example {
///     fn new(quit_event: Event) -> Self {
///         Self { quit_event }
///     }
/// }
///
/// #[dbus_interface(name = "org.myiface.Example")]
/// impl Example {
///     // This will be the "Quit" D-Bus method.
///     async fn quit(&mut self) {
///         self.quit_event.notify(1);
///     }
///
///     // See `dbus_interface` documentation to learn
///     // how to expose properties & signals as well.
/// }
///
///# block_on(async {
/// let connection = Connection::session().await?;
///
/// let quit_event = Event::new();
/// let quit_listener = quit_event.listen();
/// let interface = Example::new(quit_event);
/// connection
///     .object_server()
///     .at("/org/zbus/path", interface)
///     .await?;
///
/// quit_listener.await;
///# Ok::<_, Box<dyn Error + Send + Sync>>(())
///# });
///# Ok::<_, Box<dyn Error + Send + Sync>>(())
/// ```
#[derive(Debug)]
pub struct ObjectServer {
    conn: WeakConnection,
    root: RwLock<Node>,
}

assert_impl_all!(ObjectServer: Send, Sync, Unpin);

impl ObjectServer {
    /// Creates a new D-Bus `ObjectServer`.
    pub(crate) fn new(conn: &Connection) -> Self {
        Self {
            conn: conn.into(),
            root: RwLock::new(Node::new("/".try_into().expect("zvariant bug"))),
        }
    }

    pub(crate) fn root(&self) -> &RwLock<Node> {
        &self.root
    }

    /// Register a D-Bus [`Interface`] at a given path. (see the example above)
    ///
    /// Typically you'd want your interfaces to be registered immediately after the associated
    /// connection is established and therefore use [`zbus::ConnectionBuilder::serve_at`] instead.
    /// However, there are situations where you'd need to register interfaces dynamically and that's
    /// where this method becomes useful.
    ///
    /// If the interface already exists at this path, returns false.
    pub async fn at<'p, P, I>(&self, path: P, iface: I) -> Result<bool>
    where
        I: Interface,
        P: TryInto<ObjectPath<'p>>,
        P::Error: Into<Error>,
    {
        self.at_ready(path, I::name(), move || Arc::new(RwLock::new(iface)))
            .await
    }

    /// Same as `at` but expects an interface already in `Arc<RwLock<dyn Interface>>` form.
    // FIXME: Better name?
    pub(crate) async fn at_ready<'node, 'p, P, F>(
        &'node self,
        path: P,
        name: InterfaceName<'static>,
        iface_creator: F,
    ) -> Result<bool>
    where
        // Needs to be hardcoded as 'static instead of 'p like most other
        // functions, due to https://github.com/rust-lang/rust/issues/63033
        // (It doesn't matter a whole lot since this is an internal-only API
        // anyway.)
        P: TryInto<ObjectPath<'p>>,
        P::Error: Into<Error>,
        F: FnOnce() -> Arc<RwLock<dyn Interface + 'static>>,
    {
        let path = path.try_into().map_err(Into::into)?;
        let mut root = self.root().write().await;
        let (node, manager_path) = root.get_child_mut(&path, true);
        let node = node.unwrap();
        let added = node.at(name.clone(), iface_creator);
        if added {
            if name == ObjectManager::name() {
                // Just added an object manager. Need to signal all managed objects under it.
                let ctxt = SignalContext::new(&self.connection(), path)?;
                let objects = node.get_managed_objects().await;
                for (path, owned_interfaces) in objects {
                    let interfaces = owned_interfaces
                        .iter()
                        .map(|(i, props)| {
                            let props = props
                                .iter()
                                .map(|(k, v)| (k.as_str(), Value::from(v)))
                                .collect();
                            (i.into(), props)
                        })
                        .collect();
                    ObjectManager::interfaces_added(&ctxt, &path, &interfaces).await?;
                }
            } else if let Some(manager_path) = manager_path {
                let ctxt = SignalContext::new(&self.connection(), manager_path.clone())?;
                let mut interfaces = HashMap::new();
                let owned_props = node.get_properties(name.clone()).await;
                let props = owned_props
                    .iter()
                    .map(|(k, v)| (k.as_str(), Value::from(v)))
                    .collect();
                interfaces.insert(name, props);

                ObjectManager::interfaces_added(&ctxt, &path, &interfaces).await?;
            }
        }

        Ok(added)
    }

    /// Unregister a D-Bus [`Interface`] at a given path.
    ///
    /// If there are no more interfaces left at that path, destroys the object as well.
    /// Returns whether the object was destroyed.
    pub async fn remove<'p, I, P>(&self, path: P) -> Result<bool>
    where
        I: Interface,
        P: TryInto<ObjectPath<'p>>,
        P::Error: Into<Error>,
    {
        let path = path.try_into().map_err(Into::into)?;
        let mut root = self.root.write().await;
        let (node, manager_path) = root.get_child_mut(&path, false);
        let node = node.ok_or(Error::InterfaceNotFound)?;
        if !node.remove_interface(I::name()) {
            return Err(Error::InterfaceNotFound);
        }
        if let Some(manager_path) = manager_path {
            let ctxt = SignalContext::new(&self.connection(), manager_path.clone())?;
            ObjectManager::interfaces_removed(&ctxt, &path, &[I::name()]).await?;
        }
        if node.is_empty() {
            let mut path_parts = path.rsplit('/').filter(|i| !i.is_empty());
            let last_part = path_parts.next().unwrap();
            let ppath = ObjectPath::from_string_unchecked(
                path_parts.fold(String::new(), |a, p| format!("/{}{}", p, a)),
            );
            root.get_child_mut(&ppath, false)
                .0
                .unwrap()
                .remove_node(last_part);
            return Ok(true);
        }
        Ok(false)
    }

    /// Get the interface at the given path.
    ///
    /// # Errors
    ///
    /// If the interface is not registered at the given path, `Error::InterfaceNotFound` error is
    /// returned.
    ///
    /// # Examples
    ///
    /// The typical use of this is property changes outside of a dispatched handler:
    ///
    /// ```no_run
    ///# use std::error::Error;
    ///# use zbus::{Connection, InterfaceDerefMut, ObjectServer, SignalContext, dbus_interface};
    ///# use async_io::block_on;
    ///#
    /// struct MyIface(u32);
    ///
    /// #[dbus_interface(name = "org.myiface.MyIface")]
    /// impl MyIface {
    ///      #[dbus_interface(property)]
    ///      async fn count(&self) -> u32 {
    ///          self.0
    ///      }
    /// }
    ///
    ///# block_on(async {
    ///# let connection = Connection::session().await?;
    ///#
    ///# let path = "/org/zbus/path";
    ///# connection.object_server().at(path, MyIface(0)).await?;
    /// let iface_ref = connection
    ///     .object_server()
    ///     .interface::<_, MyIface>(path).await?;
    /// let mut iface = iface_ref.get_mut().await;
    /// iface.0 = 42;
    /// iface.count_changed(iface_ref.signal_context()).await?;
    ///# Ok::<_, Box<dyn Error + Send + Sync>>(())
    ///# })?;
    ///#
    ///# Ok::<_, Box<dyn Error + Send + Sync>>(())
    /// ```
    pub async fn interface<'p, P, I>(&self, path: P) -> Result<InterfaceRef<I>>
    where
        I: Interface,
        P: TryInto<ObjectPath<'p>>,
        P::Error: Into<Error>,
    {
        let path = path.try_into().map_err(Into::into)?;
        let root = self.root().read().await;
        let node = root.get_child(&path).ok_or(Error::InterfaceNotFound)?;

        let lock = node
            .interface_lock(I::name())
            .ok_or(Error::InterfaceNotFound)?
            .clone();

        // Ensure what we return can later be dowcasted safely.
        lock.read()
            .await
            .downcast_ref::<I>()
            .ok_or(Error::InterfaceNotFound)?;

        let conn = self.connection();
        // SAFETY: We know that there is a valid path on the node as we already converted w/o error.
        let ctxt = SignalContext::new(&conn, path).unwrap().into_owned();

        Ok(InterfaceRef {
            ctxt,
            lock,
            phantom: PhantomData,
        })
    }

    async fn dispatch_method_call_try(
        &self,
        connection: &Connection,
        msg: &Message,
    ) -> fdo::Result<Result<()>> {
        let path = msg
            .path()
            .ok_or_else(|| fdo::Error::Failed("Missing object path".into()))?;
        let iface = msg
            .interface()
            // TODO: In the absence of an INTERFACE field, if two or more interfaces on the same object
            // have a method with the same name, it is undefined which of those methods will be
            // invoked. Implementations may choose to either return an error, or deliver the message
            // as though it had an arbitrary one of those interfaces.
            .ok_or_else(|| fdo::Error::Failed("Missing interface".into()))?;
        let member = msg
            .member()
            .ok_or_else(|| fdo::Error::Failed("Missing member".into()))?;

        // Ensure the root lock isn't held while dispatching the message. That
        // way, the object server can be mutated during that time.
        let iface = {
            let root = self.root.read().await;
            let node = root
                .get_child(&path)
                .ok_or_else(|| fdo::Error::UnknownObject(format!("Unknown object '{}'", path)))?;

            node.interface_lock(iface.as_ref()).ok_or_else(|| {
                fdo::Error::UnknownInterface(format!("Unknown interface '{}'", iface))
            })?
        };

        let read_lock = iface.read().await;
        match read_lock.call(self, connection, msg, member.as_ref()) {
            DispatchResult::NotFound => {
                return Err(fdo::Error::UnknownMethod(format!(
                    "Unknown method '{}'",
                    member
                )));
            }
            DispatchResult::Async(f) => {
                return Ok(f.await);
            }
            DispatchResult::RequiresMut => {}
        }
        drop(read_lock);
        let mut write_lock = iface.write().await;
        match write_lock.call_mut(self, connection, msg, member.as_ref()) {
            DispatchResult::NotFound => {}
            DispatchResult::RequiresMut => {}
            DispatchResult::Async(f) => {
                return Ok(f.await);
            }
        }
        drop(write_lock);
        Err(fdo::Error::UnknownMethod(format!(
            "Unknown method '{}'",
            member
        )))
    }

    async fn dispatch_method_call(&self, connection: &Connection, msg: &Message) -> Result<()> {
        match self.dispatch_method_call_try(connection, msg).await {
            Err(e) => {
                let hdr = msg.header()?;
                connection.reply_dbus_error(&hdr, e).await?;
                Ok(())
            }
            Ok(r) => r,
        }
    }

    /// Dispatch an incoming message to a registered interface.
    ///
    /// The object server will handle the message by:
    ///
    /// - looking up the called object path & interface,
    ///
    /// - calling the associated method if one exists,
    ///
    /// - returning a message (responding to the caller with either a return or error message) to
    ///   the caller through the associated server connection.
    ///
    /// Returns an error if the message is malformed, true if it's handled, false otherwise.
    pub(crate) async fn dispatch_message(&self, msg: &Message) -> Result<bool> {
        match msg.message_type() {
            MessageType::MethodCall => {
                let conn = self.connection();
                self.dispatch_method_call(&conn, msg).await?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    pub(crate) fn connection(&self) -> Connection {
        self.conn
            .upgrade()
            .expect("ObjectServer can't exist w/o an associated Connection")
    }
}

impl From<crate::blocking::ObjectServer> for ObjectServer {
    fn from(server: crate::blocking::ObjectServer) -> Self {
        server.into_inner()
    }
}
