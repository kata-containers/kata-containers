#![warn(rust_2018_idioms, single_use_lifetimes)]
#![allow(dead_code)]

// Refs: https://doc.rust-lang.org/reference/attributes.html

#[macro_use]
mod auxiliary;

use pin_project::pin_project;
use std::{marker::PhantomPinned, pin::Pin};

#[cfg(target_os = "linux")]
struct Linux;
#[cfg(not(target_os = "linux"))]
struct Other;

// Use this type to check that `cfg(any())` is working properly.
struct Any(PhantomPinned);

#[test]
fn cfg() {
    // structs

    #[pin_project(project_replace)]
    struct SameName {
        #[cfg(target_os = "linux")]
        #[pin]
        inner: Linux,
        #[cfg(not(target_os = "linux"))]
        #[pin]
        inner: Other,
        #[cfg(any())]
        #[pin]
        any: Any,
    }

    assert_unpin!(SameName);

    #[cfg(target_os = "linux")]
    let _ = SameName { inner: Linux };
    #[cfg(not(target_os = "linux"))]
    let _ = SameName { inner: Other };

    #[pin_project(project_replace)]
    struct DifferentName {
        #[cfg(target_os = "linux")]
        #[pin]
        l: Linux,
        #[cfg(not(target_os = "linux"))]
        #[pin]
        o: Other,
        #[cfg(any())]
        #[pin]
        a: Any,
    }

    assert_unpin!(DifferentName);

    #[cfg(target_os = "linux")]
    let _ = DifferentName { l: Linux };
    #[cfg(not(target_os = "linux"))]
    let _ = DifferentName { o: Other };

    #[pin_project(project_replace)]
    struct TupleStruct(
        #[cfg(target_os = "linux")]
        #[pin]
        Linux,
        #[cfg(not(target_os = "linux"))]
        #[pin]
        Other,
        #[cfg(any())]
        #[pin]
        Any,
    );

    assert_unpin!(TupleStruct);

    #[cfg(target_os = "linux")]
    let _ = TupleStruct(Linux);
    #[cfg(not(target_os = "linux"))]
    let _ = TupleStruct(Other);

    // enums

    #[pin_project(
        project = VariantProj,
        project_ref = VariantProjRef,
        project_replace = VariantProjOwn,
    )]
    enum Variant {
        #[cfg(target_os = "linux")]
        Inner(#[pin] Linux),
        #[cfg(not(target_os = "linux"))]
        Inner(#[pin] Other),

        #[cfg(target_os = "linux")]
        Linux(#[pin] Linux),
        #[cfg(not(target_os = "linux"))]
        Other(#[pin] Other),
        #[cfg(any())]
        Any(#[pin] Any),
    }

    assert_unpin!(Variant);

    #[cfg(target_os = "linux")]
    let _ = Variant::Inner(Linux);
    #[cfg(not(target_os = "linux"))]
    let _ = Variant::Inner(Other);

    #[cfg(target_os = "linux")]
    let _ = Variant::Linux(Linux);
    #[cfg(not(target_os = "linux"))]
    let _ = Variant::Other(Other);

    #[pin_project(
        project = FieldProj,
        project_ref = FieldProjRef,
        project_replace = FieldProjOwn,
    )]
    enum Field {
        SameName {
            #[cfg(target_os = "linux")]
            #[pin]
            inner: Linux,
            #[cfg(not(target_os = "linux"))]
            #[pin]
            inner: Other,
            #[cfg(any())]
            #[pin]
            any: Any,
        },
        DifferentName {
            #[cfg(target_os = "linux")]
            #[pin]
            l: Linux,
            #[cfg(not(target_os = "linux"))]
            #[pin]
            w: Other,
            #[cfg(any())]
            #[pin]
            any: Any,
        },
        TupleVariant(
            #[cfg(target_os = "linux")]
            #[pin]
            Linux,
            #[cfg(not(target_os = "linux"))]
            #[pin]
            Other,
            #[cfg(any())]
            #[pin]
            Any,
        ),
    }

    assert_unpin!(Field);

    #[cfg(target_os = "linux")]
    let _ = Field::SameName { inner: Linux };
    #[cfg(not(target_os = "linux"))]
    let _ = Field::SameName { inner: Other };

    #[cfg(target_os = "linux")]
    let _ = Field::DifferentName { l: Linux };
    #[cfg(not(target_os = "linux"))]
    let _ = Field::DifferentName { w: Other };

    #[cfg(target_os = "linux")]
    let _ = Field::TupleVariant(Linux);
    #[cfg(not(target_os = "linux"))]
    let _ = Field::TupleVariant(Other);
}

#[test]
fn cfg_attr() {
    #[pin_project(project_replace)]
    struct SameCfg {
        #[cfg(target_os = "linux")]
        #[cfg_attr(target_os = "linux", pin)]
        inner: Linux,
        #[cfg(not(target_os = "linux"))]
        #[cfg_attr(not(target_os = "linux"), pin)]
        inner: Other,
        #[cfg(any())]
        #[cfg_attr(any(), pin)]
        any: Any,
    }

    assert_unpin!(SameCfg);

    #[cfg(target_os = "linux")]
    let mut x = SameCfg { inner: Linux };
    #[cfg(not(target_os = "linux"))]
    let mut x = SameCfg { inner: Other };

    let x = Pin::new(&mut x).project();
    #[cfg(target_os = "linux")]
    let _: Pin<&mut Linux> = x.inner;
    #[cfg(not(target_os = "linux"))]
    let _: Pin<&mut Other> = x.inner;

    #[pin_project(project_replace)]
    struct DifferentCfg {
        #[cfg(target_os = "linux")]
        #[cfg_attr(target_os = "linux", pin)]
        inner: Linux,
        #[cfg(not(target_os = "linux"))]
        #[cfg_attr(target_os = "linux", pin)]
        inner: Other,
        #[cfg(any())]
        #[cfg_attr(any(), pin)]
        any: Any,
    }

    assert_unpin!(DifferentCfg);

    #[cfg(target_os = "linux")]
    let mut x = DifferentCfg { inner: Linux };
    #[cfg(not(target_os = "linux"))]
    let mut x = DifferentCfg { inner: Other };

    let x = Pin::new(&mut x).project();
    #[cfg(target_os = "linux")]
    let _: Pin<&mut Linux> = x.inner;
    #[cfg(not(target_os = "linux"))]
    let _: &mut Other = x.inner;

    #[cfg_attr(not(any()), pin_project)]
    struct Foo<T> {
        #[cfg_attr(not(any()), pin)]
        inner: T,
    }

    assert_unpin!(Foo<()>);
    assert_not_unpin!(Foo<PhantomPinned>);

    let mut x = Foo { inner: 0_u8 };
    let x = Pin::new(&mut x).project();
    let _: Pin<&mut u8> = x.inner;
}

#[test]
fn cfg_attr_any_packed() {
    // Since `cfg(any())` can never be true, it is okay for this to pass.
    #[pin_project(project_replace)]
    #[cfg_attr(any(), repr(packed))]
    struct Struct {
        #[pin]
        f: u32,
    }
}
