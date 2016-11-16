use std::io;
use std::marker;

use BindServer;
use super::{RequestId, RpcMultiplex, LiftTransport};

use streaming::{self, Message, Body};
use tokio_core::io::Io;
use tokio_core::reactor::Handle;
use tokio_service::Service;
use transport::Transport;
use futures::{stream, Future, BoxFuture, Stream, Sink, Poll};

pub trait Server<T: Io> {
    type Request;
    type Response;
    type Error: From<io::Error>;

    type Transport: Transport<Item = (RequestId, Self::Request),
                              SinkItem = (RequestId, Self::Response),
                              Error = Self::Error>;
    fn bind_transport(&self, io: T) -> Self::Transport;
}

impl<T: Io, P: Server<T>> BindServer<RpcMultiplex, T> for P {
    type ServiceRequest = P::Request;
    type ServiceResponse = P::Response;
    type ServiceError = P::Error;

    fn bind_server<S>(&self, io: T, service: S) -> BoxFuture<(), P::Error> where
        S: Service<Request = Self::ServiceRequest,
                   Response = Self::ServiceResponse,
                   Error = Self::ServiceError>
    {
        LiftProto(self).bind_server(io, LiftService(service))
    }
}

struct LiftProto<'a, P: 'a>(&'a P);

impl<'a, T, P> streaming::multiplex::Server<T> for LiftProto<'a, P> where
    T: Io, P: Server<T>
{
    type Request = P::Request;
    type RequestBody = ();

    type Response = P::Response;
    type ResponseBody = ();

    type Error = P::Error;

    type Transport = LiftTransport<P::Transport>;

    fn bind_transport(&self, io: T) -> Self::Transport {
        LiftTransport(self.0.bind_transport(io))
    }
}

struct LiftService<S>(S);

impl<S: Service> Service for LiftService<S> {
    type Request = streaming::Message<S::Request, streaming::Body<(), io::Error>>;
    type Response = streaming::Message<S::Response, stream::Empty<(), S::Error>>;
    type Error = S::Error;
    type Future = LiftFuture<S::Future, stream::Empty<(), S::Error>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        match req {
            Message::WithoutBody(msg) => {
                LiftFuture(self.0.call(msg), marker::PhantomData)
            }
            Message::WithBody(..) => panic!("bodies not supported"),
        }
    }
}

struct LiftFuture<F, T>(F, marker::PhantomData<fn() -> T>);

impl<F: Future, T> Future for LiftFuture<F, T> {
    type Item = Message<F::Item, T>;
    type Error = F::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let item = try_ready!(self.0.poll());
        Ok(Message::WithoutBody(item).into())
    }
}
