extern crate loom;

use loom::thread;

#[path = "../src/channel.rs"]
mod spmc;

struct DropCounter(usize);

impl Drop for DropCounter {
    fn drop(&mut self) {
        self.0 += 1;
        assert_eq!(self.0, 1, "DropCounter dropped too many times");
    }
}

fn msg() -> DropCounter {
    DropCounter(0)
}

#[test]
fn smoke() {

    loom::model(|| {
        let (mut tx, rx) = spmc::channel::<String>();

        let th = thread::spawn(move || {
            while let Ok(_s) = rx.recv() {
                // ok
            }
        });

        tx.send("hello".into()).unwrap();
        drop(tx);
        th.join().unwrap();
    });
}

#[test]
fn no_send() {
    loom::model(|| {
        let (tx, rx) = spmc::channel::<String>();

        let th = thread::spawn(move || {
            while let Ok(_s) = rx.recv() {
                unreachable!("no sends");
            }
        });

        drop(tx);
        th.join().unwrap();
    });
}

#[test]
fn multiple_threads_race() {
    loom::model(|| {
        let (mut tx, rx) = spmc::channel();


        let mut threads = Vec::new();

        threads.push(thread::spawn(move || {
            tx.send(msg()).unwrap();
            tx.send(msg()).unwrap();
        }));

        for _ in 0..2 {
            let rx = rx.clone();
            threads.push(thread::spawn(move || {
                let mut cnt = 0;
                while let Ok(_s) = rx.recv() {
                    cnt += 1;
                }
                drop(cnt);
            }));
        }

        for th in threads {
            th.join().unwrap();
        }
    });
}


#[test]
fn message_per_thread() {
    loom::model(|| {
        let (mut tx, rx) = spmc::channel();


        let mut threads = Vec::new();

        threads.push(thread::spawn(move || {
            tx.send(msg()).unwrap();
            tx.send(msg()).unwrap();
        }));

        for t in 0..2 {
            let rx = rx.clone();
            threads.push(thread::spawn(move || {
                match rx.recv() {
                    Ok(_s) => (),
                    Err(_e) => panic!("rx thread {} didn't get message", t),
                }
            }));
        }

        for th in threads {
            th.join().unwrap();
        }
    });
}

#[test]
fn extra_message() {
    loom::model(|| {
        let (mut tx, rx) = spmc::channel();


        let mut threads = Vec::new();

        threads.push(thread::spawn(move || {
            tx.send(msg()).unwrap();
            tx.send(msg()).unwrap();
            tx.send(msg()).unwrap();
        }));

        for t in 0..2 {
            let rx = rx.clone();
            threads.push(thread::spawn(move || {
                match rx.recv() {
                    Ok(_s) => (),
                    Err(_e) => panic!("rx thread {} didn't get message", t),
                }
            }));
        }

        for th in threads {
            th.join().unwrap();
        }
    });
}
