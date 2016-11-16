use std::io;

use streaming::pipeline::Frame;
use transport::Transport;
use futures::{Stream, Sink, StartSend, Poll, AsyncSink};

mod client;
pub use self::client::Client;

mod server;
pub use self::server::Server;

pub struct RpcPipeline;

// Lifts an implementation of RPC-style transport to streaming-style transport
struct LiftTransport<T>(T);

impl<T: Sink> Sink for LiftTransport<T> {
    type SinkItem = Frame<T::SinkItem, (), T::SinkError>;
    type SinkError = io::Error;

    fn start_send(&mut self, request: Self::SinkItem)
                  -> StartSend<Self::SinkItem, Self::SinkError> {
        if let Frame::Message { message, body } = request {
            if !body {
                match try!(self.0.start_send(message)) {
                    AsyncSink::Ready => return Ok(AsyncSink::Ready),
                    AsyncSink::NotReady(msg) => {
                        let msg = Frame::Message { message: msg, body: false };
                        return Ok(AsyncSink::NotReady(msg))
                    }
                }
            }
        }
        Err(io::Error::new(io::ErrorKind::Other, "no support for streaming"))
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        self.0.poll_complete()
    }
}

impl<T: Stream> Stream for LiftTransport<T> {
    type Item = Frame<T::Item, (), T::Error>;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        let item = try_ready!(self.0.poll());
        Ok(item.map(|msg| {
            Frame::Message { message: msg, body: false }
        }).into())
    }
}

impl<T: Transport> Transport for LiftTransport<T> {}
