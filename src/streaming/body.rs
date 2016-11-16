use futures::{Async, Poll};
use futures::stream::{self, Stream, Receiver, Sender};
use std::fmt;

/// Body stream
pub struct Body<T, E> {
    inner: Option<Receiver<T, E>>,
}

impl<T, E> Body<T, E> {
    /// Return an empty body stream
    pub fn empty() -> Body<T, E> {
        Body { inner: None }
    }

    /// Return a body stream with an associated sender half
    pub fn pair() -> (Sender<T, E>, Body<T, E>) {
        let (tx, rx) = stream::channel();
        let rx = Body { inner: Some(rx) };
        (tx, rx)
    }
}

impl<T, E> Stream for Body<T, E> {
    type Item = T;
    type Error = E;

    fn poll(&mut self) -> Poll<Option<T>, E> {
        match self.inner {
            Some(ref mut s) => s.poll(),
            None => Ok(Async::Ready(None)),
        }
    }
}

impl<T, E> From<Receiver<T, E>> for Body<T, E> {
    fn from(src: Receiver<T, E>) -> Body<T, E> {
        Body { inner: Some(src) }
    }
}

impl<T, E> fmt::Debug for Body<T, E> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "Body {{ [stream of values] }}")
    }
}
