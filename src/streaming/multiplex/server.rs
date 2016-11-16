use super::{Frame, RequestId, StreamingMultiplex};
use super::advanced::{Multiplex, MultiplexMessage};

use BindServer;
use streaming::{Message, Body};
use transport::Transport;
use tokio_service::Service;
use tokio_core::io::Io;
use futures::{BoxFuture, Future, Poll, Async};
use futures::stream::Stream;
use std::io;

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

impl<P, T, B> BindServer<super::StreamingMultiplex<B>, T> for P where
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
            in_flight: vec![],
        };

        // Create the pipeline dispatcher
        Box::new(Multiplex::new(dispatch))
    }
}

struct Dispatch<S, T, P> where T: Io, P: Server<T>, S: Service {
    // The service handling the connection
    service: S,
    transport: P::Transport,
    in_flight: Vec<(RequestId, InFlight<S::Future>)>,
}

enum InFlight<F: Future> {
    Active(F),
    Done(Result<F::Item, F::Error>),
}

/// The total number of requests that can be in flight at once.
const MAX_IN_FLIGHT_REQUESTS: usize = 32;

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

    fn transport(&mut self) -> &mut T {
        &mut self.transport
    }

    fn poll(&mut self) -> Poll<Option<MultiplexMessage<Self::In, B, Self::Error>>, io::Error> {
        trace!("Dispatch::poll");

        let mut idx = None;

        for (i, &mut (request_id, ref mut slot)) in self.in_flight.iter_mut().enumerate() {
            trace!("   --> poll; request_id={:?}", request_id);
            if slot.poll() && idx.is_none() {
                idx = Some(i);
            }
        }

        if let Some(idx) = idx {
            let (request_id, message) = self.in_flight.remove(idx);
            let message = MultiplexMessage {
                id: request_id,
                message: message.unwrap_done(),
                solo: false,
            };

            Ok(Async::Ready(Some(message)))
        } else {
            Ok(Async::NotReady)
        }
    }

    fn dispatch(&mut self, message: MultiplexMessage<Self::Out, Body<Self::BodyOut, Self::Error>, Self::Error>) -> io::Result<()> {
        assert!(self.poll_ready().is_ready());

        let MultiplexMessage { id, message, solo } = message;

        assert!(!solo);

        if let Ok(request) = message {
            let response = self.service.call(request);
            self.in_flight.push((id, InFlight::Active(response)));
        }

        // TODO: Should the error be handled differently?

        Ok(())
    }

    fn poll_ready(&self) -> Async<()> {
        if self.in_flight.len() < MAX_IN_FLIGHT_REQUESTS {
            Async::Ready(())
        } else {
            Async::NotReady
        }
    }

    fn cancel(&mut self, _request_id: RequestId) -> io::Result<()> {
        // TODO: implement
        Ok(())
    }
}

/*
 *
 * ===== InFlight =====
 *
 */

impl<F> InFlight<F>
    where F: Future,
{
    // Returns true if done
    fn poll(&mut self) -> bool {
        let res = match *self {
            InFlight::Active(ref mut f) => {
                trace!("   --> polling future");
                match f.poll() {
                    Ok(Async::Ready(e)) => Ok(e),
                    Err(e) => Err(e),
                    Ok(Async::NotReady) => return false,
                }
            }
            _ => return true,
        };

        *self = InFlight::Done(res);
        true
    }

    fn unwrap_done(self) -> Result<F::Item, F::Error> {
        match self {
            InFlight::Done(res) => res,
            _ => panic!("future is not ready"),
        }
    }
}
