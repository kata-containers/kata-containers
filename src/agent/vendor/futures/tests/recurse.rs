#[test]
fn lots() {
    use futures::executor::block_on;
    use futures::future::{self, FutureExt, BoxFuture};
    use std::sync::mpsc;
    use std::thread;

    fn do_it(input: (i32, i32)) -> BoxFuture<'static, i32> {
        let (n, x) = input;
        if n == 0 {
            future::ready(x).boxed()
        } else {
            future::ready((n - 1, x + n)).then(do_it).boxed()
        }
    }

    let (tx, rx) = mpsc::channel();
    thread::spawn(|| {
        block_on(do_it((1_000, 0)).map(move |x| tx.send(x).unwrap()))
    });
    assert_eq!(500_500, rx.recv().unwrap());
}
