use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::hash::{BuildHasherDefault, Hasher};
use std::marker::PhantomData;
use std::sync::Arc;

thread_local! {
    static CURRENT_CONTEXT: RefCell<Context> = RefCell::new(Context::default());
    static DEFAULT_CONTEXT: Context = Context::default();
}

/// An execution-scoped collection of values.
///
/// A [`Context`] is a propagation mechanism which carries execution-scoped
/// values across API boundaries and between logically associated execution
/// units. Cross-cutting concerns access their data in-process using the same
/// shared context object.
///
/// [`Context`]s are immutable, and their write operations result in the creation
/// of a new context containing the original values and the new specified values.
///
/// ## Context state
///
/// Concerns can create and retrieve their local state in the current execution
/// state represented by a context through the [`get`] and [`with_value`]
/// methods. It is recommended to use application-specific types when storing new
/// context values to avoid unintentionally overwriting existing state.
///
/// ## Managing the current context
///
/// Contexts can be associated with the caller's current execution unit on a
/// given thread via the [`attach`] method, and previous contexts can be restored
/// by dropping the returned [`ContextGuard`]. Context can be nested, and will
/// restore their parent outer context when detached on drop. To access the
/// values of the context, a snapshot can be created via the [`Context::current`]
/// method.
///
/// [`Context::current`]: Context::current()
/// [`get`]: Context::get()
/// [`with_value`]: Context::with_value()
/// [`attach`]: Context::attach()
///
/// # Examples
///
/// ```
/// use opentelemetry::Context;
///
/// // Application-specific `a` and `b` values
/// #[derive(Debug, PartialEq)]
/// struct ValueA(&'static str);
/// #[derive(Debug, PartialEq)]
/// struct ValueB(u64);
///
/// let _outer_guard = Context::new().with_value(ValueA("a")).attach();
///
/// // Only value a has been set
/// let current = Context::current();
/// assert_eq!(current.get::<ValueA>(), Some(&ValueA("a")));
/// assert_eq!(current.get::<ValueB>(), None);
///
/// {
///     let _inner_guard = Context::current_with_value(ValueB(42)).attach();
///     // Both values are set in inner context
///     let current = Context::current();
///     assert_eq!(current.get::<ValueA>(), Some(&ValueA("a")));
///     assert_eq!(current.get::<ValueB>(), Some(&ValueB(42)));
/// }
///
/// // Resets to only the `a` value when inner guard is dropped
/// let current = Context::current();
/// assert_eq!(current.get::<ValueA>(), Some(&ValueA("a")));
/// assert_eq!(current.get::<ValueB>(), None);
/// ```
#[derive(Clone, Default)]
pub struct Context {
    entries: HashMap<TypeId, Arc<dyn Any + Sync + Send>, BuildHasherDefault<IdHasher>>,
}

impl Context {
    /// Creates an empty `Context`.
    ///
    /// The context is initially created with a capacity of 0, so it will not
    /// allocate. Use [`with_value`] to create a new context that has entries.
    ///
    /// [`with_value`]: Context::with_value()
    pub fn new() -> Self {
        Context::default()
    }

    /// Returns an immutable snapshot of the current thread's context.
    ///
    /// # Examples
    ///
    /// ```
    /// use opentelemetry::Context;
    ///
    /// #[derive(Debug, PartialEq)]
    /// struct ValueA(&'static str);
    ///
    /// fn do_work() {
    ///     assert_eq!(Context::current().get(), Some(&ValueA("a")));
    /// }
    ///
    /// let _guard = Context::new().with_value(ValueA("a")).attach();
    /// do_work()
    /// ```
    pub fn current() -> Self {
        get_current(|cx| cx.clone())
    }

    /// Returns a clone of the current thread's context with the given value.
    ///
    /// This is a more efficient form of `Context::current().with_value(value)`
    /// as it avoids the intermediate context clone.
    ///
    /// # Examples
    ///
    /// ```
    /// use opentelemetry::Context;
    ///
    /// // Given some value types defined in your application
    /// #[derive(Debug, PartialEq)]
    /// struct ValueA(&'static str);
    /// #[derive(Debug, PartialEq)]
    /// struct ValueB(u64);
    ///
    /// // You can create and attach context with the first value set to "a"
    /// let _guard = Context::new().with_value(ValueA("a")).attach();
    ///
    /// // And create another context based on the fist with a new value
    /// let all_current_and_b = Context::current_with_value(ValueB(42));
    ///
    /// // The second context now contains all the current values and the addition
    /// assert_eq!(all_current_and_b.get::<ValueA>(), Some(&ValueA("a")));
    /// assert_eq!(all_current_and_b.get::<ValueB>(), Some(&ValueB(42)));
    /// ```
    pub fn current_with_value<T: 'static + Send + Sync>(value: T) -> Self {
        let mut new_context = Context::current();
        new_context
            .entries
            .insert(TypeId::of::<T>(), Arc::new(value));

        new_context
    }

    /// Returns a reference to the entry for the corresponding value type.
    ///
    /// # Examples
    ///
    /// ```
    /// use opentelemetry::Context;
    ///
    /// // Given some value types defined in your application
    /// #[derive(Debug, PartialEq)]
    /// struct ValueA(&'static str);
    /// #[derive(Debug, PartialEq)]
    /// struct MyUser();
    ///
    /// let cx = Context::new().with_value(ValueA("a"));
    ///
    /// // Values can be queried by type
    /// assert_eq!(cx.get::<ValueA>(), Some(&ValueA("a")));
    ///
    /// // And return none if not yet set
    /// assert_eq!(cx.get::<MyUser>(), None);
    /// ```
    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.entries
            .get(&TypeId::of::<T>())
            .and_then(|rc| (&*rc).downcast_ref())
    }

    /// Returns a copy of the context with the new value included.
    ///
    /// # Examples
    ///
    /// ```
    /// use opentelemetry::Context;
    ///
    /// // Given some value types defined in your application
    /// #[derive(Debug, PartialEq)]
    /// struct ValueA(&'static str);
    /// #[derive(Debug, PartialEq)]
    /// struct ValueB(u64);
    ///
    /// // You can create a context with the first value set to "a"
    /// let cx_with_a = Context::new().with_value(ValueA("a"));
    ///
    /// // And create another context based on the fist with a new value
    /// let cx_with_a_and_b = cx_with_a.with_value(ValueB(42));
    ///
    /// // The first context is still available and unmodified
    /// assert_eq!(cx_with_a.get::<ValueA>(), Some(&ValueA("a")));
    /// assert_eq!(cx_with_a.get::<ValueB>(), None);
    ///
    /// // The second context now contains both values
    /// assert_eq!(cx_with_a_and_b.get::<ValueA>(), Some(&ValueA("a")));
    /// assert_eq!(cx_with_a_and_b.get::<ValueB>(), Some(&ValueB(42)));
    /// ```
    pub fn with_value<T: 'static + Send + Sync>(&self, value: T) -> Self {
        let mut new_context = self.clone();
        new_context
            .entries
            .insert(TypeId::of::<T>(), Arc::new(value));

        new_context
    }

    /// Replaces the current context on this thread with this context.
    ///
    /// Dropping the returned [`ContextGuard`] will reset the current context to the
    /// previous value.
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// use opentelemetry::Context;
    ///
    /// #[derive(Debug, PartialEq)]
    /// struct ValueA(&'static str);
    ///
    /// let my_cx = Context::new().with_value(ValueA("a"));
    ///
    /// // Set the current thread context
    /// let cx_guard = my_cx.attach();
    /// assert_eq!(Context::current().get::<ValueA>(), Some(&ValueA("a")));
    ///
    /// // Drop the guard to restore the previous context
    /// drop(cx_guard);
    /// assert_eq!(Context::current().get::<ValueA>(), None);
    /// ```
    ///
    /// Guards do not need to be explicitly dropped:
    ///
    /// ```
    /// use opentelemetry::Context;
    ///
    /// #[derive(Debug, PartialEq)]
    /// struct ValueA(&'static str);
    ///
    /// fn my_function() -> String {
    ///     // attach a context the duration of this function.
    ///     let my_cx = Context::new().with_value(ValueA("a"));
    ///     // NOTE: a variable name after the underscore is **required** or rust
    ///     // will drop the guard, restoring the previous context _immediately_.
    ///     let _guard = my_cx.attach();
    ///
    ///     // anything happening in functions we call can still access my_cx...
    ///     my_other_function();
    ///
    ///     // returning from the function drops the guard, exiting the span.
    ///     return "Hello world".to_owned();
    /// }
    ///
    /// fn my_other_function() {
    ///     // ...
    /// }
    /// ```
    /// Sub-scopes may be created to limit the duration for which the span is
    /// entered:
    ///
    /// ```
    /// use opentelemetry::Context;
    ///
    /// #[derive(Debug, PartialEq)]
    /// struct ValueA(&'static str);
    ///
    /// let my_cx = Context::new().with_value(ValueA("a"));
    ///
    /// {
    ///     let _guard = my_cx.attach();
    ///
    ///     // the current context can access variables in
    ///     assert_eq!(Context::current().get::<ValueA>(), Some(&ValueA("a")));
    ///
    ///     // exiting the scope drops the guard, detaching the context.
    /// }
    ///
    /// // this is back in the default empty context
    /// assert_eq!(Context::current().get::<ValueA>(), None);
    /// ```
    pub fn attach(self) -> ContextGuard {
        let previous_cx = CURRENT_CONTEXT
            .try_with(|current| current.replace(self))
            .ok();

        ContextGuard {
            previous_cx,
            _marker: PhantomData,
        }
    }
}

impl fmt::Debug for Context {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Context")
            .field("entries", &self.entries.len())
            .finish()
    }
}

/// A guard that resets the current context to the prior context when dropped.
#[allow(missing_debug_implementations)]
pub struct ContextGuard {
    previous_cx: Option<Context>,
    // ensure this type is !Send as it relies on thread locals
    _marker: PhantomData<*const ()>,
}

impl Drop for ContextGuard {
    fn drop(&mut self) {
        if let Some(previous_cx) = self.previous_cx.take() {
            let _ = CURRENT_CONTEXT.try_with(|current| current.replace(previous_cx));
        }
    }
}

/// Executes a closure with a reference to this thread's current context.
///
/// Note: This function will panic if you attempt to attach another context
/// while the context is still borrowed.
fn get_current<F: FnMut(&Context) -> T, T>(mut f: F) -> T {
    CURRENT_CONTEXT
        .try_with(|cx| f(&*cx.borrow()))
        .unwrap_or_else(|_| DEFAULT_CONTEXT.with(|cx| f(&*cx)))
}

/// With TypeIds as keys, there's no need to hash them. They are already hashes
/// themselves, coming from the compiler. The IdHasher holds the u64 of
/// the TypeId, and then returns it, instead of doing any bit fiddling.
#[derive(Clone, Default, Debug)]
struct IdHasher(u64);

impl Hasher for IdHasher {
    fn write(&mut self, _: &[u8]) {
        unreachable!("TypeId calls write_u64");
    }

    #[inline]
    fn write_u64(&mut self, id: u64) {
        self.0 = id;
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_contexts() {
        #[derive(Debug, PartialEq)]
        struct ValueA(&'static str);
        #[derive(Debug, PartialEq)]
        struct ValueB(u64);
        let _outer_guard = Context::new().with_value(ValueA("a")).attach();

        // Only value `a` is set
        let current = Context::current();
        assert_eq!(current.get(), Some(&ValueA("a")));
        assert_eq!(current.get::<ValueB>(), None);

        {
            let _inner_guard = Context::current_with_value(ValueB(42)).attach();
            // Both values are set in inner context
            let current = Context::current();
            assert_eq!(current.get(), Some(&ValueA("a")));
            assert_eq!(current.get(), Some(&ValueB(42)));
        }

        // Resets to only value `a` when inner guard is dropped
        let current = Context::current();
        assert_eq!(current.get(), Some(&ValueA("a")));
        assert_eq!(current.get::<ValueB>(), None);
    }
}
