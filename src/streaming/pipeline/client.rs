use error::Error;
use BindClient;
use streaming::{Body, Message};
use transport::Transport;
use super::{StreamingPipeline, Frame};
use super::advanced::{Pipeline, PipelineMessage};
use util::client_proxy::{self, ClientProxy, Receiver};
use futures::stream::Stream;
use futures::{Future, IntoFuture, Complete, Poll, Async};
use tokio_core::reactor::Handle;
use tokio_core::io::Io;
use std::collections::VecDeque;
use std::io;

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

impl<P, T, B> BindClient<StreamingPipeline<B>, T> for P where
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
            in_flight: VecDeque::with_capacity(32),
        };
        let task = Pipeline::new(dispatch) .map_err(|e| {
            // TODO: where to punt this error to?
            error!("pipeline error: {}", e);
        });

        // Spawn the task
        handle.spawn(task);

        // Return the client
        client
    }
}

struct Dispatch<P, T, B> where
    P: Client<T> + BindClient<StreamingPipeline<B>, T>,
    T: Io,
    B: Stream<Item = P::RequestBody, Error = P::Error>,
{
    transport: P::Transport,
    requests: Receiver<P::ServiceRequest, P::ServiceResponse, P::ServiceError>,
    in_flight: VecDeque<Complete<Result<P::ServiceResponse, P::ServiceError>>>,
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

    fn dispatch(&mut self,
                response: PipelineMessage<Self::Out, Body<Self::BodyOut, Self::Error>, Self::Error>)
                -> io::Result<()>
    {
        if let Some(complete) = self.in_flight.pop_front() {
            complete.complete(response);
        } else {
            return Err(io::Error::new(io::ErrorKind::Other, "request / response mismatch"));
        }

        Ok(())
    }

    fn poll(&mut self) -> Poll<Option<PipelineMessage<Self::In, Self::Stream, Self::Error>>, io::Error> {
        trace!("Dispatch::poll");
        // Try to get a new request frame
        match self.requests.poll() {
            Ok(Async::Ready(Some((request, complete)))) => {
                trace!("   --> received request");

                // Track complete handle
                self.in_flight.push_back(complete);

                Ok(Async::Ready(Some(Ok(request))))

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

    fn has_in_flight(&self) -> bool {
        !self.in_flight.is_empty()
    }
}

impl<P, T, B> Drop for Dispatch<P, T, B> where
    P: Client<T> + BindClient<StreamingPipeline<B>, T>,
    T: Io,
    B: Stream<Item = P::RequestBody, Error = P::Error>,
{
    fn drop(&mut self) {
        // Complete any pending requests with an error
        while let Some(complete) = self.in_flight.pop_front() {
            let err = Error::Io(broken_pipe());
            complete.complete(Err(err.into()));
        }
    }
}

fn broken_pipe() -> io::Error {
    io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe")
}
