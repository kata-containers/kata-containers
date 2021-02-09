extern crate env_logger;
extern crate mio;
extern crate mio_named_pipes;
extern crate rand;
extern crate winapi;

#[macro_use]
extern crate log;

use std::fs::OpenOptions;
use std::io;
use std::io::prelude::*;
use std::os::windows::fs::*;
use std::os::windows::io::*;
use std::time::Duration;

use mio::{Events, Poll, PollOpt, Ready, Token};
use mio_named_pipes::NamedPipe;
use rand::Rng;
use winapi::um::winbase::*;

macro_rules! t {
    ($e:expr) => {
        match $e {
            Ok(e) => e,
            Err(e) => panic!("{} failed with {}", stringify!($e), e),
        }
    };
}

fn server() -> (NamedPipe, String) {
    let num: u64 = rand::thread_rng().gen();
    let name = format!(r"\\.\pipe\my-pipe-{}", num);
    let pipe = t!(NamedPipe::new(&name));
    (pipe, name)
}

fn client(name: &str) -> NamedPipe {
    let mut opts = OpenOptions::new();
    opts.read(true)
        .write(true)
        .custom_flags(FILE_FLAG_OVERLAPPED);
    let file = t!(opts.open(name));
    unsafe { NamedPipe::from_raw_handle(file.into_raw_handle()) }
}

fn pipe() -> (NamedPipe, NamedPipe) {
    let (pipe, name) = server();
    (pipe, client(&name))
}

#[test]
fn writable_after_register() {
    drop(env_logger::init());

    let (server, client) = pipe();
    let poll = t!(Poll::new());
    t!(poll.register(
        &server,
        Token(0),
        Ready::writable() | Ready::readable(),
        PollOpt::edge()
    ));
    t!(poll.register(&client, Token(1), Ready::writable(), PollOpt::edge()));

    let mut events = Events::with_capacity(128);
    t!(poll.poll(&mut events, None));

    let events = events.iter().collect::<Vec<_>>();
    debug!("events {:?}", events);
    assert!(events
        .iter()
        .any(|e| { e.token() == Token(0) && e.readiness() == Ready::writable() }));
    assert!(events
        .iter()
        .any(|e| { e.token() == Token(1) && e.readiness() == Ready::writable() }));
}

#[test]
fn write_then_read() {
    drop(env_logger::init());

    let (mut server, mut client) = pipe();
    let poll = t!(Poll::new());
    t!(poll.register(
        &server,
        Token(0),
        Ready::readable() | Ready::writable(),
        PollOpt::edge()
    ));
    t!(poll.register(
        &client,
        Token(1),
        Ready::readable() | Ready::writable(),
        PollOpt::edge()
    ));

    let mut events = Events::with_capacity(128);
    t!(poll.poll(&mut events, None));

    assert_eq!(t!(client.write(b"1234")), 4);

    loop {
        t!(poll.poll(&mut events, None));
        let events = events.iter().collect::<Vec<_>>();
        debug!("events {:?}", events);
        if let Some(event) = events.iter().find(|e| e.token() == Token(0)) {
            if event.readiness().is_readable() {
                break;
            }
        }
    }

    let mut buf = [0; 10];
    assert_eq!(t!(server.read(&mut buf)), 4);
    assert_eq!(&buf[..4], b"1234");
}

#[test]
fn connect_before_client() {
    drop(env_logger::init());

    let (server, name) = server();
    let poll = t!(Poll::new());
    t!(poll.register(
        &server,
        Token(0),
        Ready::readable() | Ready::writable(),
        PollOpt::edge()
    ));

    let mut events = Events::with_capacity(128);
    t!(poll.poll(&mut events, Some(Duration::new(0, 0))));
    let e = events.iter().collect::<Vec<_>>();
    debug!("events {:?}", e);
    assert_eq!(e.len(), 0);
    assert_eq!(
        server.connect().err().unwrap().kind(),
        io::ErrorKind::WouldBlock
    );

    let client = client(&name);
    t!(poll.register(
        &client,
        Token(1),
        Ready::readable() | Ready::writable(),
        PollOpt::edge()
    ));
    loop {
        t!(poll.poll(&mut events, None));
        let e = events.iter().collect::<Vec<_>>();
        debug!("events {:?}", e);
        if let Some(event) = e.iter().find(|e| e.token() == Token(0)) {
            if event.readiness().is_writable() {
                break;
            }
        }
    }
}

#[test]
fn connect_after_client() {
    drop(env_logger::init());

    let (server, name) = server();
    let poll = t!(Poll::new());
    t!(poll.register(
        &server,
        Token(0),
        Ready::readable() | Ready::writable(),
        PollOpt::edge()
    ));

    let mut events = Events::with_capacity(128);
    t!(poll.poll(&mut events, Some(Duration::new(0, 0))));
    let e = events.iter().collect::<Vec<_>>();
    debug!("events {:?}", e);
    assert_eq!(e.len(), 0);

    let client = client(&name);
    t!(poll.register(
        &client,
        Token(1),
        Ready::readable() | Ready::writable(),
        PollOpt::edge()
    ));
    t!(server.connect());
    loop {
        t!(poll.poll(&mut events, None));
        let e = events.iter().collect::<Vec<_>>();
        debug!("events {:?}", e);
        if let Some(event) = e.iter().find(|e| e.token() == Token(0)) {
            if event.readiness().is_writable() {
                break;
            }
        }
    }
}

#[test]
fn write_then_drop() {
    drop(env_logger::init());

    let (mut server, mut client) = pipe();
    let poll = t!(Poll::new());
    t!(poll.register(
        &server,
        Token(0),
        Ready::readable() | Ready::writable(),
        PollOpt::edge()
    ));
    t!(poll.register(
        &client,
        Token(1),
        Ready::readable() | Ready::writable(),
        PollOpt::edge()
    ));
    assert_eq!(t!(client.write(b"1234")), 4);
    drop(client);

    let mut events = Events::with_capacity(128);

    loop {
        t!(poll.poll(&mut events, None));
        let events = events.iter().collect::<Vec<_>>();
        debug!("events {:?}", events);
        if let Some(event) = events.iter().find(|e| e.token() == Token(0)) {
            if event.readiness().is_readable() {
                break;
            }
        }
    }

    let mut buf = [0; 10];
    assert_eq!(t!(server.read(&mut buf)), 4);
    assert_eq!(&buf[..4], b"1234");
}

#[test]
fn connect_twice() {
    drop(env_logger::init());

    let (mut server, name) = server();
    let c1 = client(&name);
    let poll = t!(Poll::new());
    t!(poll.register(
        &server,
        Token(0),
        Ready::readable() | Ready::writable(),
        PollOpt::edge()
    ));
    t!(poll.register(
        &c1,
        Token(1),
        Ready::readable() | Ready::writable(),
        PollOpt::edge()
    ));
    drop(c1);

    let mut events = Events::with_capacity(128);

    loop {
        t!(poll.poll(&mut events, None));
        let events = events.iter().collect::<Vec<_>>();
        debug!("events {:?}", events);
        if let Some(event) = events.iter().find(|e| e.token() == Token(0)) {
            if event.readiness().is_readable() {
                break;
            }
        }
    }

    let mut buf = [0; 10];
    assert_eq!(t!(server.read(&mut buf)), 0);
    t!(server.disconnect());
    assert_eq!(
        server.connect().err().unwrap().kind(),
        io::ErrorKind::WouldBlock
    );

    let c2 = client(&name);
    t!(poll.register(
        &c2,
        Token(2),
        Ready::readable() | Ready::writable(),
        PollOpt::edge()
    ));

    loop {
        t!(poll.poll(&mut events, None));
        let events = events.iter().collect::<Vec<_>>();
        debug!("events {:?}", events);
        if let Some(event) = events.iter().find(|e| e.token() == Token(0)) {
            if event.readiness().is_writable() {
                break;
            }
        }
    }
}
