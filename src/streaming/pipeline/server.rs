use BindServer;
use futures::stream::Stream;
use futures::{BoxFuture, Future, Poll, Async};
use std::collections::VecDeque;
use std::io;
use streaming::{Message, Body};
use super::advanced::{Pipeline, PipelineMessage};
use super::{StreamingPipeline, Frame};
use tokio_core::io::Io;
use tokio_service::Service;
use transport::Transport;

// TODO:
//
// - Wait for service readiness
// - Handle request body stream cancellation

pub trait Server<T: Io> {
    type Request;
    type RequestBody;

    type Response;
    type ResponseBody;

    type Error: From<io::Error>;

    type Transport: Transport<Item = Frame<Self::Request, Self::RequestBody, Self::Error>,
                              SinkItem = Frame<Self::Response, Self::ResponseBody, Self::Error>,
                              Error = io::Error>;
    fn bind_transport(&self, io: T) -> Self::Transport;
}

impl<P, T, B> BindServer<super::StreamingPipeline<B>, T> for P where
    P: Server<T>,
    T: Io,
    B: Stream<Item = P::ResponseBody, Error = P::Error>,
{
    type ServiceRequest = Message<P::Request, Body<P::RequestBody, P::Error>>;
    type ServiceResponse = Message<P::Response, B>;
    type ServiceError = P::Error;

    fn bind_server<S>(&self, io: T, service: S) -> BoxFuture<(), P::Error> where
        S: Service<Request = Self::ServiceRequest,
                   Response = Self::ServiceResponse,
                   Error = Self::ServiceError>
    {
        let dispatch = Dispatch {
            service: service,
            transport: self.bind_transport(io),
            in_flight: VecDeque::with_capacity(32),
        };

        // Create the pipeline dispatcher
        Box::new(Pipeline::new(dispatch))
    }
}

struct Dispatch<S, T, P> where T: Io, P: Server<T>, S: Service {
    // The service handling the connection
    service: S,
    transport: P::Transport,
    in_flight: VecDeque<InFlight<S::Future>>,
}

enum InFlight<F: Future> {
    Active(F),
    Done(Result<F::Item, F::Error>),
}

impl<P, T, B, S> super::advanced::Dispatch for Dispatch<S, T, P> where
    P: Server<T>,
    T: Io,
    B: Stream<Item = P::ResponseBody, Error = P::Error>,
    S: Service<Request = Message<P::Request, Body<P::RequestBody, P::Error>>,
               Response = Message<P::Response, B>,
               Error = P::Error>,
{
    type In = P::Response;
    type BodyIn = P::ResponseBody;
    type Out = P::Request;
    type BodyOut = P::RequestBody;
    type Error = P::Error;
    type Stream = B;
    type Transport = P::Transport;

    fn transport(&mut self) -> &mut P::Transport {
        &mut self.transport
    }

    fn dispatch(&mut self,
                request: PipelineMessage<Self::Out, Body<Self::BodyOut, Self::Error>, Self::Error>)
                -> io::Result<()>
    {
        if let Ok(request) = request {
            let response = self.service.call(request);
            self.in_flight.push_back(InFlight::Active(response));
        }

        // TODO: Should the error be handled differently?

        Ok(())
    }

    fn poll(&mut self) -> Poll<Option<PipelineMessage<Self::In, Self::Stream, Self::Error>>, io::Error> {
        for slot in self.in_flight.iter_mut() {
            slot.poll();
        }

        match self.in_flight.front() {
            Some(&InFlight::Done(_)) => {}
            _ => return Ok(Async::NotReady)
        }

        match self.in_flight.pop_front() {
            Some(InFlight::Done(res)) => Ok(Async::Ready(Some(res))),
            _ => panic!(),
        }
    }

    fn has_in_flight(&self) -> bool {
        !self.in_flight.is_empty()
    }
}

impl<F: Future> InFlight<F> {
    fn poll(&mut self) {
        let res = match *self {
            InFlight::Active(ref mut f) => {
                match f.poll() {
                    Ok(Async::Ready(e)) => Ok(e),
                    Err(e) => Err(e),
                    Ok(Async::NotReady) => return,
                }
            }
            _ => return,
        };
        *self = InFlight::Done(res);
    }
}
