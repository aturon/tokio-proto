//! Utilities for building protocol clients
//!
//! Provides a channel that handles details of providing a `Service` client.
//! Usually, this module does not have to be used directly. Instead it is used
//! by `pipeline` and `multiplex` in the `connect` fns.
//!
//! However, some protocols require implementing the dispatch layer directly,
//! in which case using client channel is helpful.

// Allow warnings in order to prevent the compiler from outputting an error
// that seems to be fixed on nightly.
#![allow(warnings)]

use error::Error;
use streaming::Message;
use tokio_service::Service;
use futures::{Future, Async, Poll, Stream, AsyncSink, Sink};
use futures::sync::spsc;
use futures::sync::oneshot;
use std::io;
use std::cell::RefCell;

/// Client `Service` for pipeline or multiplex protocols
pub struct ClientProxy<R, S, E> {
    tx: RefCell<spsc::Sender<Envelope<R, S, E>, io::Error>>,
}

/// Response future returned from a client
pub struct Response<T, E> {
    inner: oneshot::Receiver<Result<T, E>>,
}

/// Message used to dispatch requests to the task managing the client
/// connection.
type Envelope<R, S, E> = (R, oneshot::Sender<Result<S, E>>);

/// A client / receiver pair
pub type Pair<R, S, E> = (ClientProxy<R, S, E>, Receiver<R, S, E>);

/// Receive requests submitted to the client
pub type Receiver<R, S, E> = spsc::Receiver<Envelope<R, S, E>, io::Error>;

/// Return a client handle and a handle used to receive requests on
pub fn pair<R, S, E>() -> Pair<R, S, E> {
    // Create a stream
    let (tx, rx) = spsc::channel();

    // Use the sender handle to create a `Client` handle
    let client = ClientProxy { tx: RefCell::new(tx) };

    // Return the pair
    (client, rx)
}

impl<R, S, E> Service for ClientProxy<R, S, E> {
    type Request = R;
    type Response = S;
    type Error = E;
    type Future = Response<S, E>;

    fn call(&self, request: R) -> Self::Future {
        let (tx, rx) = oneshot::channel();

        // TODO: handle error
        match self.tx.borrow_mut().start_send(Ok((request, tx))) {
            Ok(AsyncSink::Ready) => {}
            Ok(AsyncSink::NotReady(_)) => {
                panic!("not ready to send a new request")
            }
            Err(_) => panic!("receiving end of client is gone"),
        }

        Response { inner: rx }
    }
}

impl<T, E> Future for Response<T, E>
    where E: From<io::Error>,
{
    type Item = T;
    type Error = E;

    fn poll(&mut self) -> Poll<T, E> {
        match self.inner.poll() {
            Ok(Async::Ready(Ok(v))) => Ok(Async::Ready(v)),
            Ok(Async::Ready(Err(e))) => Err(e),
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(_) => {
                let e = io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe");
                Err(e.into())
            }
        }
    }
}
