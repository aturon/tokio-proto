use super::{Frame, RequestId, StreamingMultiplex};
use super::advanced::{Multiplex, MultiplexMessage};

use BindClient;
use error::Error;
use streaming::{Body, Message};
use transport::Transport;
use util::client_proxy::{self, ClientProxy, Receiver};
use futures::{Future, Complete, Poll, Async};
use futures::stream::Stream;
use tokio_core::reactor::Handle;
use tokio_core::io::Io;
use std::io;
use std::collections::HashMap;

pub trait Client<T: Io> {
    type Request;
    type RequestBody;

    type Response;
    type ResponseBody;

    type Error: From<io::Error>;

    type Transport: Transport<Item = Frame<Self::Response, Self::ResponseBody, Self::Error>,
                              SinkItem = Frame<Self::Request, Self::RequestBody, Self::Error>,
                              Error = io::Error>;
    fn bind_transport(&self, io: T) -> Self::Transport;
}

impl<P, T, B> BindClient<StreamingMultiplex<B>, T> for P where
    P: Client<T>,
    T: Io,
    B: Stream<Item = P::RequestBody, Error = P::Error>,
{
    type ServiceRequest = Message<P::Request, B>;
    type ServiceResponse = Message<P::Response, Body<P::ResponseBody, P::Error>>;
    type ServiceError = P::Error;

    type BindClient = ClientProxy<Self::ServiceRequest, Self::ServiceResponse, Self::ServiceError>;

    fn bind_client(&self, handle: &Handle, io: T) -> Self::BindClient {
        let (client, rx) = client_proxy::pair();

        let dispatch = Dispatch {
            transport: self.bind_transport(io),
            requests: rx,
            in_flight: HashMap::new(),
            next_request_id: 0,
        };

        let task = Multiplex::new(dispatch) .map_err(|e| {
            // TODO: where to punt this error to?
            debug!("multiplex task failed with error; err={:?}", e);
        });

        // Spawn the task
        handle.spawn(task);

        // Return the client
        client
    }
}

struct Dispatch<P, T, B> where
    P: Client<T> + BindClient<StreamingMultiplex<B>, T>,
    T: Io,
    B: Stream<Item = P::RequestBody, Error = P::Error>,
{
    transport: P::Transport,
    requests: Receiver<P::ServiceRequest, P::ServiceResponse, P::ServiceError>,
    in_flight: HashMap<RequestId, Complete<Result<P::ServiceResponse, P::ServiceError>>>,
    next_request_id: u64,
}

impl<P, T, B> super::advanced::Dispatch for Dispatch<P, T, B> where
    P: Client<T>,
    T: Io,
    B: Stream<Item = P::RequestBody, Error = P::Error>,
{
    type In = P::Request;
    type BodyIn = P::RequestBody;
    type Out = P::Response;
    type BodyOut = P::ResponseBody;
    type Error = P::Error;
    type Stream = B;
        type Transport = P::Transport;

    fn transport(&mut self) -> &mut Self::Transport {
        &mut self.transport
    }

    fn dispatch(&mut self, message: MultiplexMessage<Self::Out, Body<Self::BodyOut, Self::Error>, Self::Error>) -> io::Result<()> {
        let MultiplexMessage { id, message, solo } = message;

        assert!(!solo);

        if let Some(complete) = self.in_flight.remove(&id) {
            complete.complete(message);
        } else {
            return Err(io::Error::new(io::ErrorKind::Other, "request / response mismatch"));
        }

        Ok(())
    }

    fn poll(&mut self) -> Poll<Option<MultiplexMessage<Self::In, B, Self::Error>>, io::Error> {
        trace!("Dispatch::poll");
        // Try to get a new request frame
        match self.requests.poll() {
            Ok(Async::Ready(Some((request, complete)))) => {
                trace!("   --> received request");

                let request_id = self.next_request_id;
                self.next_request_id += 1;

                trace!("   --> assigning request-id={:?}", request_id);

                // Track complete handle
                self.in_flight.insert(request_id, complete);

                Ok(Async::Ready(Some(MultiplexMessage::new(request_id, request))))

            }
            Ok(Async::Ready(None)) => {
                trace!("   --> client dropped");
                Ok(Async::Ready(None))
            }
            Err(e) => {
                trace!("   --> error");
                // An error on receive can only happen when the other half
                // disconnected. In this case, the client needs to be
                // shutdown
                panic!("unimplemented error handling: {:?}", e);
            }
            Ok(Async::NotReady) => {
                trace!("   --> not ready");
                Ok(Async::NotReady)
            }
        }
    }

    fn poll_ready(&self) -> Async<()> {
        // Not capping the client yet
        Async::Ready(())
    }

    fn cancel(&mut self, _request_id: RequestId) -> io::Result<()> {
        // TODO: implement
        Ok(())
    }
}

impl<P, T, B> Drop for Dispatch<P, T, B> where
    P: Client<T> + BindClient<StreamingMultiplex<B>, T>,
    T: Io,
    B: Stream<Item = P::RequestBody, Error = P::Error>,
{
    fn drop(&mut self) {
        if !self.in_flight.is_empty() {
            warn!("multiplex client dropping with in-flight exchanges");
        }

        // Complete any pending requests with an error
        for (_, complete) in self.in_flight.drain() {
            let err = Error::Io(broken_pipe());
            complete.complete(Err(err.into()));
        }
    }
}

fn broken_pipe() -> io::Error {
    io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe")
}
