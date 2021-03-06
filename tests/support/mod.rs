#![allow(dead_code)]

use std::fmt;
use std::sync::Arc;
use std::thread;

use futures::{Future, IntoFuture, Async, Poll};
use futures::future::FutureResult;
use futures::stream::Stream;
use futures::executor::{self, Unpark};
use futures::task;

pub mod local_executor;

pub fn f_ok(a: i32) -> FutureResult<i32, u32> { Ok(a).into_future() }
pub fn f_err(a: u32) -> FutureResult<i32, u32> { Err(a).into_future() }
pub fn r_ok(a: i32) -> Result<i32, u32> { Ok(a) }
pub fn r_err(a: u32) -> Result<i32, u32> { Err(a) }

pub fn assert_done<T, F>(f: F, result: Result<T::Item, T::Error>)
    where T: Future,
          T::Item: Eq + fmt::Debug,
          T::Error: Eq + fmt::Debug,
          F: FnOnce() -> T,
{
    assert_eq!(f().wait(), result);
}

pub fn assert_empty<T: Future, F: FnMut() -> T>(mut f: F) {
    assert!(executor::spawn(f()).poll_future(unpark_panic()).ok().unwrap().is_not_ready());
}

pub fn sassert_done<S: Stream>(s: &mut S) {
    match executor::spawn(s).poll_stream(unpark_panic()) {
        Ok(Async::Ready(None)) => {}
        Ok(Async::Ready(Some(_))) => panic!("stream had more elements"),
        Ok(Async::NotReady) => panic!("stream wasn't ready"),
        Err(_) => panic!("stream had an error"),
    }
}

pub fn sassert_empty<S: Stream>(s: &mut S) {
    match executor::spawn(s).poll_stream(unpark_noop()) {
        Ok(Async::Ready(None)) => panic!("stream is at its end"),
        Ok(Async::Ready(Some(_))) => panic!("stream had more elements"),
        Ok(Async::NotReady) => {}
        Err(_) => panic!("stream had an error"),
    }
}

pub fn sassert_next<S: Stream>(s: &mut S, item: S::Item)
    where S::Item: Eq + fmt::Debug
{
    match executor::spawn(s).poll_stream(unpark_panic()) {
        Ok(Async::Ready(None)) => panic!("stream is at its end"),
        Ok(Async::Ready(Some(e))) => assert_eq!(e, item),
        Ok(Async::NotReady) => panic!("stream wasn't ready"),
        Err(_) => panic!("stream had an error"),
    }
}

pub fn sassert_err<S: Stream>(s: &mut S, err: S::Error)
    where S::Error: Eq + fmt::Debug
{
    match executor::spawn(s).poll_stream(unpark_panic()) {
        Ok(Async::Ready(None)) => panic!("stream is at its end"),
        Ok(Async::Ready(Some(_))) => panic!("stream had more elements"),
        Ok(Async::NotReady) => panic!("stream wasn't ready"),
        Err(e) => assert_eq!(e, err),
    }
}

pub fn unpark_panic() -> Arc<Unpark> {
    struct Foo;

    impl Unpark for Foo {
        fn unpark(&self) {
            panic!("should not be unparked");
        }
    }

    Arc::new(Foo)
}

pub fn unpark_noop() -> Arc<Unpark> {
    struct Foo;

    impl Unpark for Foo {
        fn unpark(&self) {}
    }

    Arc::new(Foo)
}

pub trait ForgetExt {
    fn forget(self);
}

impl<F> ForgetExt for F
    where F: Future + Sized + Send + 'static,
          F::Item: Send,
          F::Error: Send
{
    fn forget(self) {
        thread::spawn(|| self.wait());
    }
}

pub struct DelayFuture<F>(F,bool);

impl<F: Future> Future for DelayFuture<F> {
    type Item = F::Item;
    type Error = F::Error;

    fn poll(&mut self) -> Poll<F::Item,F::Error> {
        if self.1 {
            self.0.poll()
        } else {
            self.1 = true;
            task::park().unpark();
            Ok(Async::NotReady)
        }
    }
}

/// Introduces one Ok(Async::NotReady) before polling the given future
pub fn delay_future<F>(f: F) -> DelayFuture<F::Future>
    where F: IntoFuture,
{
    DelayFuture(f.into_future(), false)
}

