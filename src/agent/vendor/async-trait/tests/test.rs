#![cfg_attr(
    async_trait_nightly_testing,
    feature(min_specialization, min_const_generics)
)]

use async_trait::async_trait;

pub mod executor;

// Dummy module to check that the expansion refer to rust's core crate
mod core {}

#[async_trait]
trait Trait {
    type Assoc;

    async fn selfvalue(self)
    where
        Self: Sized,
    {
    }

    async fn selfref(&self) {}

    async fn selfmut(&mut self) {}

    async fn required() -> Self::Assoc;

    async fn elided_lifetime(_x: &str) {}

    async fn explicit_lifetime<'a>(_x: &'a str) {}

    async fn generic_type_param<T: Send>(x: Box<T>) -> T {
        *x
    }

    async fn calls(&self) {
        self.selfref().await;
        Self::elided_lifetime("").await;
        <Self>::elided_lifetime("").await;
    }

    async fn calls_mut(&mut self) {
        self.selfmut().await;
    }
}

struct Struct;

#[async_trait]
impl Trait for Struct {
    type Assoc = ();

    async fn selfvalue(self) {}

    async fn selfref(&self) {}

    async fn selfmut(&mut self) {}

    async fn required() -> Self::Assoc {}

    async fn elided_lifetime(_x: &str) {}

    async fn explicit_lifetime<'a>(_x: &'a str) {}

    async fn generic_type_param<T: Send>(x: Box<T>) -> T {
        *x
    }

    async fn calls(&self) {
        self.selfref().await;
        Self::elided_lifetime("").await;
        <Self>::elided_lifetime("").await;
    }

    async fn calls_mut(&mut self) {
        self.selfmut().await;
    }
}

pub async fn test() {
    let mut s = Struct;
    s.selfref().await;
    s.selfmut().await;
    s.selfvalue().await;

    Struct::required().await;
    Struct::elided_lifetime("").await;
    Struct::explicit_lifetime("").await;
    Struct::generic_type_param(Box::new("")).await;

    let mut s = Struct;
    s.calls().await;
    s.calls_mut().await;
}

pub async fn test_object_safe_without_default() {
    #[async_trait]
    trait ObjectSafe {
        async fn f(&self);
    }

    #[async_trait]
    impl ObjectSafe for Struct {
        async fn f(&self) {}
    }

    let object = &Struct as &dyn ObjectSafe;
    object.f().await;
}

pub async fn test_object_safe_with_default() {
    #[async_trait]
    trait ObjectSafe: Sync {
        async fn f(&self) {}
    }

    #[async_trait]
    impl ObjectSafe for Struct {
        async fn f(&self) {}
    }

    let object = &Struct as &dyn ObjectSafe;
    object.f().await;
}

pub async fn test_object_no_send() {
    #[async_trait(?Send)]
    trait ObjectSafe: Sync {
        async fn f(&self) {}
    }

    #[async_trait(?Send)]
    impl ObjectSafe for Struct {
        async fn f(&self) {}
    }

    let object = &Struct as &dyn ObjectSafe;
    object.f().await;
}

#[async_trait]
pub unsafe trait UnsafeTrait {}

#[async_trait]
unsafe impl UnsafeTrait for () {}

#[async_trait]
pub(crate) unsafe trait UnsafeTraitPubCrate {}

#[async_trait]
unsafe trait UnsafeTraitPrivate {}

// https://github.com/dtolnay/async-trait/issues/1
pub mod issue1 {
    use async_trait::async_trait;

    #[async_trait]
    trait Issue1 {
        async fn f<U>(&self);
    }

    #[async_trait]
    impl<T: Sync> Issue1 for Vec<T> {
        async fn f<U>(&self) {}
    }
}

// https://github.com/dtolnay/async-trait/issues/2
pub mod issue2 {
    use async_trait::async_trait;
    use std::future::Future;

    #[async_trait]
    pub trait Issue2: Future {
        async fn flatten(self) -> <Self::Output as Future>::Output
        where
            Self::Output: Future + Send,
            Self: Sized,
        {
            let nested_future = self.await;
            nested_future.await
        }
    }
}

// https://github.com/dtolnay/async-trait/issues/9
pub mod issue9 {
    use async_trait::async_trait;

    #[async_trait]
    pub trait Issue9: Sized + Send {
        async fn f(_x: Self) {}
    }
}

// https://github.com/dtolnay/async-trait/issues/11
pub mod issue11 {
    use async_trait::async_trait;
    use std::sync::Arc;

    #[async_trait]
    trait Issue11 {
        async fn example(self: Arc<Self>);
    }

    struct Struct;

    #[async_trait]
    impl Issue11 for Struct {
        async fn example(self: Arc<Self>) {}
    }
}

// https://github.com/dtolnay/async-trait/issues/15
pub mod issue15 {
    use async_trait::async_trait;
    use std::marker::PhantomData;

    trait Trait {}

    #[async_trait]
    trait Issue15 {
        async fn myfn(&self, _: PhantomData<dyn Trait + Send>) {}
    }
}

// https://github.com/dtolnay/async-trait/issues/17
pub mod issue17 {
    use async_trait::async_trait;

    #[async_trait]
    trait Issue17 {
        async fn f(&self);
    }

    struct Struct {
        string: String,
    }

    #[async_trait]
    impl Issue17 for Struct {
        async fn f(&self) {
            println!("{}", self.string);
        }
    }
}

// https://github.com/dtolnay/async-trait/issues/23
pub mod issue23 {
    use async_trait::async_trait;

    #[async_trait]
    pub trait Issue23 {
        async fn f(self);

        async fn g(mut self)
        where
            Self: Sized,
        {
            do_something(&mut self);
        }
    }

    struct S {}

    #[async_trait]
    impl Issue23 for S {
        async fn f(mut self) {
            do_something(&mut self);
        }
    }

    fn do_something<T>(_: &mut T) {}
}

// https://github.com/dtolnay/async-trait/issues/25
#[cfg(async_trait_nightly_testing)]
pub mod issue25 {
    use crate::executor;
    use async_trait::async_trait;
    use std::fmt::{Display, Write};

    #[async_trait]
    trait AsyncToString {
        async fn async_to_string(&self) -> String;
    }

    #[async_trait]
    impl AsyncToString for String {
        async fn async_to_string(&self) -> String {
            "special".to_owned()
        }
    }

    macro_rules! hide_from_stable_parser {
        ($($tt:tt)*) => {
            $($tt)*
        };
    }

    hide_from_stable_parser! {
        #[async_trait]
        impl<T: ?Sized + Display + Sync> AsyncToString for T {
            default async fn async_to_string(&self) -> String {
                let mut buf = String::new();
                buf.write_fmt(format_args!("{}", self)).unwrap();
                buf
            }
        }
    }

    #[test]
    fn test() {
        let fut = true.async_to_string();
        assert_eq!(executor::block_on_simple(fut), "true");

        let string = String::new();
        let fut = string.async_to_string();
        assert_eq!(executor::block_on_simple(fut), "special");
    }
}

// https://github.com/dtolnay/async-trait/issues/28
pub mod issue28 {
    use async_trait::async_trait;

    struct Str<'a>(&'a str);

    #[async_trait]
    trait Trait1<'a> {
        async fn f(x: Str<'a>) -> &'a str;
        async fn g(x: Str<'a>) -> &'a str {
            x.0
        }
    }

    #[async_trait]
    impl<'a> Trait1<'a> for str {
        async fn f(x: Str<'a>) -> &'a str {
            x.0
        }
    }

    #[async_trait]
    trait Trait2 {
        async fn f();
    }

    #[async_trait]
    impl<'a> Trait2 for &'a () {
        async fn f() {}
    }

    #[async_trait]
    trait Trait3<'a, 'b> {
        async fn f(_: &'a &'b ()); // chain 'a and 'b
        async fn g(_: &'b ()); // chain 'b only
        async fn h(); // do not chain
    }
}

// https://github.com/dtolnay/async-trait/issues/31
pub mod issue31 {
    use async_trait::async_trait;

    pub struct Struct<'a> {
        pub name: &'a str,
    }

    #[async_trait]
    pub trait Trait<'a> {
        async fn hello(thing: Struct<'a>) -> String;
        async fn hello_twice(one: Struct<'a>, two: Struct<'a>) -> String {
            let str1 = Self::hello(one).await;
            let str2 = Self::hello(two).await;
            str1 + &str2
        }
    }
}

// https://github.com/dtolnay/async-trait/issues/42
pub mod issue42 {
    use async_trait::async_trait;

    #[async_trait]
    pub trait Context: Sized {
        async fn from_parts() -> Self;
    }

    pub struct TokenContext;

    #[async_trait]
    impl Context for TokenContext {
        async fn from_parts() -> TokenContext {
            TokenContext
        }
    }
}

// https://github.com/dtolnay/async-trait/issues/44
pub mod issue44 {
    use async_trait::async_trait;

    #[async_trait]
    pub trait StaticWithWhereSelf
    where
        Box<Self>: Sized,
        Self: Sized + Send,
    {
        async fn get_one() -> u8 {
            1
        }
    }

    pub struct Struct;

    #[async_trait]
    impl StaticWithWhereSelf for Struct {}
}

// https://github.com/dtolnay/async-trait/issues/45
pub mod issue45 {
    use crate::executor;
    use async_trait::async_trait;
    use std::fmt::Debug;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};
    use tracing::event::Event;
    use tracing::field::{Field, Visit};
    use tracing::span::{Attributes, Id, Record};
    use tracing::{info, instrument, subscriber, Metadata, Subscriber};

    #[async_trait]
    pub trait Parent {
        async fn foo(&mut self, v: usize);
    }

    #[async_trait]
    pub trait Child {
        async fn bar(&self);
    }

    #[derive(Debug)]
    struct Impl(usize);

    #[async_trait]
    impl Parent for Impl {
        #[instrument]
        async fn foo(&mut self, v: usize) {
            self.0 = v;
            self.bar().await;
        }
    }

    #[async_trait]
    impl Child for Impl {
        // Let's check that tracing detects the renaming of the `self` variable
        // too, as tracing::instrument is not going to be able to skip the
        // `self` argument if it can't find it in the function signature.
        #[instrument(skip(self))]
        async fn bar(&self) {
            info!(val = self.0);
        }
    }

    // A simple subscriber implementation to test the behavior of async-trait
    // with tokio-rs/tracing. This implementation is not robust against race
    // conditions, but it's not an issue here as we are only polling on a single
    // future at a time.
    #[derive(Debug)]
    struct SubscriberInner {
        current_depth: AtomicU64,
        // We assert that nested functions work. If the fix were to break, we
        // would see two top-level functions instead of `bar` nested in `foo`.
        max_depth: AtomicU64,
        max_span_id: AtomicU64,
        // Name of the variable / value / depth when the event was recorded.
        value: Mutex<Option<(&'static str, u64, u64)>>,
    }

    #[derive(Debug, Clone)]
    struct TestSubscriber {
        inner: Arc<SubscriberInner>,
    }

    impl TestSubscriber {
        fn new() -> Self {
            TestSubscriber {
                inner: Arc::new(SubscriberInner {
                    current_depth: AtomicU64::new(0),
                    max_depth: AtomicU64::new(0),
                    max_span_id: AtomicU64::new(1),
                    value: Mutex::new(None),
                }),
            }
        }
    }

    struct U64Visitor(Option<(&'static str, u64)>);

    impl Visit for U64Visitor {
        fn record_debug(&mut self, _field: &Field, _value: &dyn Debug) {}

        fn record_u64(&mut self, field: &Field, value: u64) {
            self.0 = Some((field.name(), value));
        }
    }

    impl Subscriber for TestSubscriber {
        fn enabled(&self, _metadata: &Metadata) -> bool {
            true
        }
        fn new_span(&self, _span: &Attributes) -> Id {
            Id::from_u64(self.inner.max_span_id.fetch_add(1, Ordering::AcqRel))
        }
        fn record(&self, _span: &Id, _values: &Record) {}
        fn record_follows_from(&self, _span: &Id, _follows: &Id) {}
        fn event(&self, event: &Event) {
            let mut visitor = U64Visitor(None);
            event.record(&mut visitor);
            if let Some((s, v)) = visitor.0 {
                let current_depth = self.inner.current_depth.load(Ordering::Acquire);
                *self.inner.value.lock().unwrap() = Some((s, v, current_depth));
            }
        }
        fn enter(&self, _span: &Id) {
            let old_depth = self.inner.current_depth.fetch_add(1, Ordering::AcqRel);
            if old_depth + 1 > self.inner.max_depth.load(Ordering::Acquire) {
                self.inner.max_depth.fetch_add(1, Ordering::AcqRel);
            }
        }
        fn exit(&self, _span: &Id) {
            self.inner.current_depth.fetch_sub(1, Ordering::AcqRel);
        }
    }

    #[test]
    fn tracing() {
        // Create the future outside of the subscriber, as no call to tracing
        // should be made until the future is polled.
        let mut struct_impl = Impl(0);
        let fut = struct_impl.foo(5);
        let subscriber = TestSubscriber::new();
        subscriber::with_default(subscriber.clone(), || executor::block_on_simple(fut));
        // Did we enter bar inside of foo?
        assert_eq!(subscriber.inner.max_depth.load(Ordering::Acquire), 2);
        // Have we exited all spans?
        assert_eq!(subscriber.inner.current_depth.load(Ordering::Acquire), 0);
        // Did we create only two spans? Note: spans start at 1, hence the -1.
        assert_eq!(subscriber.inner.max_span_id.load(Ordering::Acquire) - 1, 2);
        // Was the value recorded at the right depth i.e. in the right function?
        // If so, was it the expected value?
        assert_eq!(*subscriber.inner.value.lock().unwrap(), Some(("val", 5, 2)));
    }
}

// https://github.com/dtolnay/async-trait/issues/46
pub mod issue46 {
    use async_trait::async_trait;

    macro_rules! implement_commands_workaround {
        ($tyargs:tt : $ty:tt) => {
            #[async_trait]
            pub trait AsyncCommands1: Sized {
                async fn f<$tyargs: $ty>(&mut self, x: $tyargs) {
                    self.f(x).await
                }
            }
        };
    }

    implement_commands_workaround!(K: Send);

    macro_rules! implement_commands {
        ($tyargs:ident : $ty:ident) => {
            #[async_trait]
            pub trait AsyncCommands2: Sized {
                async fn f<$tyargs: $ty>(&mut self, x: $tyargs) {
                    self.f(x).await
                }
            }
        };
    }

    implement_commands!(K: Send);
}

// https://github.com/dtolnay/async-trait/issues/53
pub mod issue53 {
    use async_trait::async_trait;

    pub struct Unit;
    pub struct Tuple(u8);
    pub struct Struct {
        pub x: u8,
    }

    #[async_trait]
    pub trait Trait {
        async fn method();
    }

    #[async_trait]
    impl Trait for Unit {
        async fn method() {
            let _ = Self;
        }
    }

    #[async_trait]
    impl Trait for Tuple {
        async fn method() {
            let _ = Self(0);
        }
    }

    #[async_trait]
    impl Trait for Struct {
        async fn method() {
            let _ = Self { x: 0 };
        }
    }

    #[async_trait]
    impl Trait for std::marker::PhantomData<Struct> {
        async fn method() {
            let _ = Self;
        }
    }
}

// https://github.com/dtolnay/async-trait/issues/57
#[cfg(async_trait_nightly_testing)]
pub mod issue57 {
    use crate::executor;
    use async_trait::async_trait;

    #[async_trait]
    trait Trait {
        async fn const_generic<T: Send, const C: usize>(_: [T; C]) {}
    }

    struct Struct;

    #[async_trait]
    impl Trait for Struct {
        async fn const_generic<T: Send, const C: usize>(_: [T; C]) {}
    }

    #[test]
    fn test() {
        let fut = Struct::const_generic([0; 10]);
        executor::block_on_simple(fut);
    }
}

// https://github.com/dtolnay/async-trait/issues/68
pub mod issue68 {
    #[rustversion::since(1.40)] // procedural macros cannot expand to macro definitions in 1.39.
    #[async_trait::async_trait]
    pub trait Example {
        async fn method(&self) {
            macro_rules! t {
                () => {{
                    let _: &Self = self;
                }};
            }
            t!();
        }
    }
}

// https://github.com/dtolnay/async-trait/issues/73
pub mod issue73 {
    use async_trait::async_trait;

    #[async_trait]
    pub trait Example {
        const ASSOCIATED: &'static str;

        async fn associated(&self) {
            println!("Associated:{}", Self::ASSOCIATED);
        }
    }
}

// https://github.com/dtolnay/async-trait/issues/81
pub mod issue81 {
    use async_trait::async_trait;

    #[async_trait]
    pub trait Trait {
        async fn handle(&self);
    }

    pub enum Enum {
        Variant,
    }

    #[async_trait]
    impl Trait for Enum {
        async fn handle(&self) {
            let Enum::Variant = self;
            let Self::Variant = self;
        }
    }
}

// https://github.com/dtolnay/async-trait/issues/83
pub mod issue83 {
    #![allow(clippy::needless_arbitrary_self_type)]

    use async_trait::async_trait;

    #[async_trait]
    pub trait Trait {
        async fn f(&self) {}
        async fn g(self: &Self) {}
    }
}

// https://github.com/dtolnay/async-trait/issues/85
pub mod issue85 {
    #![deny(non_snake_case)]

    use async_trait::async_trait;

    #[async_trait]
    pub trait Trait {
        #[allow(non_snake_case)]
        async fn camelCase();
    }

    pub struct Struct;

    #[async_trait]
    impl Trait for Struct {
        async fn camelCase() {}
    }
}

// https://github.com/dtolnay/async-trait/issues/87
pub mod issue87 {
    use async_trait::async_trait;

    #[async_trait]
    pub trait Trait {
        async fn f(&self);
    }

    pub enum Tuple {
        V(),
    }

    pub enum Struct {
        V {},
    }

    #[async_trait]
    impl Trait for Tuple {
        async fn f(&self) {
            let Tuple::V() = self;
            let Self::V() = self;
            let _ = Self::V;
            let _ = Self::V();
        }
    }

    #[async_trait]
    impl Trait for Struct {
        async fn f(&self) {
            let Struct::V {} = self;
            let Self::V {} = self;
            let _ = Self::V {};
        }
    }
}

// https://github.com/dtolnay/async-trait/issues/89
pub mod issue89 {
    #![allow(bare_trait_objects)]

    use async_trait::async_trait;

    #[async_trait]
    trait Trait {
        async fn f(&self);
    }

    #[async_trait]
    impl Trait for Send + Sync {
        async fn f(&self) {}
    }

    #[async_trait]
    impl Trait for dyn Fn(i8) + Send + Sync {
        async fn f(&self) {}
    }

    #[async_trait]
    impl Trait for (dyn Fn(u8) + Send + Sync) {
        async fn f(&self) {}
    }
}

// https://github.com/dtolnay/async-trait/issues/92
pub mod issue92 {
    use async_trait::async_trait;

    macro_rules! mac {
        ($($tt:tt)*) => {
            $($tt)*
        };
    }

    pub struct Struct<T> {
        _x: T,
    }

    impl<T> Struct<T> {
        const ASSOCIATED1: &'static str = "1";
        async fn associated1() {}
    }

    #[async_trait]
    pub trait Trait
    where
        mac!(Self): Send,
    {
        const ASSOCIATED2: &'static str;
        type Associated2;

        #[allow(path_statements, clippy::no_effect)]
        async fn associated2(&self) {
            // trait items
            mac!(let _: Self::Associated2;);
            mac!(let _: <Self>::Associated2;);
            mac!(let _: <Self as Trait>::Associated2;);
            mac!(Self::ASSOCIATED2;);
            mac!(<Self>::ASSOCIATED2;);
            mac!(<Self as Trait>::ASSOCIATED2;);
            mac!(let _ = Self::associated2(self););
            mac!(let _ = <Self>::associated2(self););
            mac!(let _ = <Self as Trait>::associated2(self););
        }
    }

    #[async_trait]
    impl<T: Send + Sync> Trait for Struct<T>
    where
        mac!(Self): Send,
    {
        const ASSOCIATED2: &'static str = "2";
        type Associated2 = ();

        #[allow(path_statements, clippy::no_effect)]
        async fn associated2(&self) {
            // inherent items
            mac!(Self::ASSOCIATED1;);
            mac!(<Self>::ASSOCIATED1;);
            mac!(let _ = Self::associated1(););
            mac!(let _ = <Self>::associated1(););

            // trait items
            mac!(let _: <Self as Trait>::Associated2;);
            mac!(Self::ASSOCIATED2;);
            mac!(<Self>::ASSOCIATED2;);
            mac!(<Self as Trait>::ASSOCIATED2;);
            mac!(let _ = Self::associated2(self););
            mac!(let _ = <Self>::associated2(self););
            mac!(let _ = <Self as Trait>::associated2(self););
        }
    }

    pub struct Unit;

    #[async_trait]
    impl Trait for Unit {
        const ASSOCIATED2: &'static str = "2";
        type Associated2 = ();

        async fn associated2(&self) {
            mac!(let Self: Self = *self;);
        }
    }
}

// https://github.com/dtolnay/async-trait/issues/92#issuecomment-683370136
pub mod issue92_2 {
    use async_trait::async_trait;

    macro_rules! mac {
        ($($tt:tt)*) => {
            $($tt)*
        };
    }

    pub trait Trait1 {
        fn func1();
    }

    #[async_trait]
    pub trait Trait2: Trait1 {
        async fn func2() {
            mac!(Self::func1());

            macro_rules! mac2 {
                ($($tt:tt)*) => {
                    Self::func1();
                };
            }
            mac2!();
        }
    }
}

// https://github.com/dtolnay/async-trait/issues/104
pub mod issue104 {
    use async_trait::async_trait;

    #[async_trait]
    trait T1 {
        async fn id(&self) -> i32;
    }

    macro_rules! impl_t1 {
        ($ty:ty, $id:expr) => {
            #[async_trait]
            impl T1 for $ty {
                async fn id(&self) -> i32 {
                    $id
                }
            }
        };
    }

    struct Foo;

    impl_t1!(Foo, 1);
}

// https://github.com/dtolnay/async-trait/issues/106
pub mod issue106 {
    use async_trait::async_trait;
    use std::future::Future;

    #[async_trait]
    pub trait ProcessPool: Send + Sync {
        type ThreadPool;

        async fn spawn<F, Fut, T>(&self, work: F) -> T
        where
            F: FnOnce(&Self::ThreadPool) -> Fut + Send,
            Fut: Future<Output = T> + 'static;
    }

    #[async_trait]
    impl<P: ?Sized> ProcessPool for &P
    where
        P: ProcessPool,
    {
        type ThreadPool = P::ThreadPool;

        async fn spawn<F, Fut, T>(&self, work: F) -> T
        where
            F: FnOnce(&Self::ThreadPool) -> Fut + Send,
            Fut: Future<Output = T> + 'static,
        {
            (**self).spawn(work).await
        }
    }
}

// https://github.com/dtolnay/async-trait/issues/110
pub mod issue110 {
    #![deny(clippy::all)]

    use async_trait::async_trait;
    use std::marker::PhantomData;

    #[async_trait]
    pub trait Loader {
        async fn load(&self, key: &str);
    }

    pub struct AwsEc2MetadataLoader<'a> {
        marker: PhantomData<&'a ()>,
    }

    #[async_trait]
    impl Loader for AwsEc2MetadataLoader<'_> {
        async fn load(&self, _key: &str) {}
    }
}

// https://github.com/dtolnay/async-trait/issues/120
pub mod issue120 {
    #![deny(clippy::trivially_copy_pass_by_ref)]

    use async_trait::async_trait;

    #[async_trait]
    trait Trait {
        async fn f(&self);
    }

    #[async_trait]
    impl Trait for () {
        async fn f(&self) {}
    }
}

// https://github.com/dtolnay/async-trait/issues/123
pub mod issue123 {
    use async_trait::async_trait;

    #[async_trait]
    trait Trait<T = ()> {
        async fn f(&self) -> &str
        where
            T: 'async_trait,
        {
            "default"
        }
    }

    #[async_trait]
    impl<T> Trait<T> for () {}
}

// https://github.com/dtolnay/async-trait/issues/129
pub mod issue129 {
    #![deny(clippy::pedantic)]

    use async_trait::async_trait;

    #[async_trait]
    pub trait TestTrait {
        async fn a(_b: u8, c: u8) -> u8 {
            c
        }
    }

    pub struct TestStruct;

    #[async_trait]
    impl TestTrait for TestStruct {
        async fn a(_b: u8, c: u8) -> u8 {
            c
        }
    }
}

// https://github.com/dtolnay/async-trait/issues/134
#[cfg(async_trait_nightly_testing)]
pub mod issue134 {
    use async_trait::async_trait;

    #[async_trait]
    trait TestTrait {
        async fn run<const DUMMY: bool>(self)
        where
            Self: Sized,
        {
        }
    }

    pub struct TestStruct;

    #[async_trait]
    impl TestTrait for TestStruct {
        async fn run<const DUMMY: bool>(self)
        where
            Self: Sized,
        {
        }
    }
}
