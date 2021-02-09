#[macro_use]
extern crate log;

#[test]
fn base() {
    info!("hello");
    info!("hello",);
}

#[test]
fn base_expr_context() {
    let _ = info!("hello");
}

#[test]
fn with_args() {
    info!("hello {}", "cats");
    info!("hello {}", "cats",);
    info!("hello {}", "cats",);
}

#[test]
fn with_args_expr_context() {
    match "cats" {
        cats => info!("hello {}", cats),
    };
}

#[test]
fn with_named_args() {
    let cats = "cats";

    info!("hello {cats}", cats = cats);
    info!("hello {cats}", cats = cats,);
    info!("hello {cats}", cats = cats,);
}
