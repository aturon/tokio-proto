use std::io;

use streaming::multiplex::Frame;
use transport::Transport;
use futures::{Stream, Sink, StartSend, Poll, AsyncSink};

mod client;
pub use self::client::Client;

mod server;
pub use self::server::Server;

/// Identifies a request / response thread
pub type RequestId = u64;

pub struct RpcMultiplex;

// Lifts an implementation of RPC-style transport to streaming-style transport
struct LiftTransport<T>(T);

// This is just a `sink.with` to transform a multiplex frame to our own frame,
// but we can name it.
impl<F, U> Sink for LiftTransport<F>
    where F: Sink<SinkItem = (RequestId, U)>,
{
    type SinkItem = Frame<U, (), F::SinkError>;
    type SinkError = io::Error;

    fn start_send(&mut self, request: Self::SinkItem)
                  -> StartSend<Self::SinkItem, Self::SinkError> {
        if let Frame::Message { message, id, body, solo } = request {
            if !body && !solo {
                match try!(self.inner.start_send((id, message))) {
                    AsyncSink::Ready => return Ok(AsyncSink::Ready),
                    AsyncSink::NotReady((id, msg)) => {
                        let msg = Frame::Message {
                            message: msg,
                            id: id,
                            body: false,
                            solo: false,
                        };
                        return Ok(AsyncSink::NotReady(msg))
                    }
                }
            }
        }
        Err(io::Error::new(io::ErrorKind::Other, "no support for streaming"))
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        self.inner.poll_complete()
    }
}

// This is just a `stream.map` to transform our frames into multiplex frames
// but we can name it.
impl<F, T> Stream for LiftTransport<F>
    where F: Stream<Item = (RequestId, T)>,
{
    type Item = Frame<T, (), F::Error>;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        let (id, msg) = match try_ready!(self.inner.poll()) {
            Some(msg) => msg,
            None => return Ok(None.into()),
        };
        Ok(Some(Frame::Message {
            message: msg,
            body: false,
            solo: false,
            id: id,
        }).into())
    }
}

impl<T, U, V> Transport for LiftTransport<T> where
    T: Transport<Item = (RequestId, U),
                 SinkItem = (RequestId, V)>
{}
