use BindClient;
use super::{RpcPipeline, LiftTransport};

use streaming::{self, Message, Body};
use tokio_core::io::Io;
use tokio_core::reactor::Handle;
use tokio_service::Service;
use transport::Transport;
use futures::{stream, Future, Stream, Sink, Poll};
use util::client_proxy::{Response, ClientProxy};
use std::io;

pub trait Client<T: Io> {
    type Request;
    type Response;
    type Error: From<io::Error>;

    type Transport: Transport<Item = Self::Response,
                              SinkItem = Self::Request,
                              Error = Self::Error>;

    fn bind_transport(&self, io: T) -> Self::Transport;
}

impl<T: Io, P: Client<T>> BindClient<RpcPipeline, T> for P {
    type ServiceRequest = P::Request;
    type ServiceResponse = P::Response;
    type ServiceError = P::Error;

    type BindClient = LowerService<ClientProxy<Message<P::Request, stream::Empty<(), P::Error>>,
                                               Message<P::Response, Body<(), P::Error>>,
                                               P::Error>>;

    fn bind_client(&self, handle: &Handle, io: T) -> Self::BindClient {
        Box::new(LowerService(LiftProto(self).bind_client(handle, io)))
    }
}

struct LiftProto<'a, P: 'a>(&'a P);

impl<'a, T, P> streaming::pipeline::Client<T> for LiftProto<'a, P> where
    T: Io, P: Client<T>
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

struct LowerService<S>(S);

impl<Req, Resp, E, S> Service for LowerService<S> where
    S: Service<Request = Message<Req, stream::Empty<(), E>>,
               Response = Message<Resp, Body<(), E>>,
               Error = E>,
{
    type Request = Req;
    type Response = Resp;
    type Error = E;
    type Future = LowerFuture<S::Future>;

    fn call(&self, req: Req) -> Self::Future {
        LowerFuture(Box::new(self.0.call(Message::WithoutBody(req))))
    }
}

struct LowerFuture<F>(F);

impl<Resp, B, F> Future for LowerFuture<F>
    where F: Future<Item = Message<Resp, B>>
{
    type Item = Resp;
    type Error = F::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match try_ready!(self.0.poll()) {
            Message::WithoutBody(msg) => Ok(msg.into()),
            Message::WithBody(..) => panic!("bodies not supported"),
        }
    }
}

/*
mod bug {
    use streaming::{Message, Body};
    use futures::{Future, stream};
    use tokio_service::Service;

    trait MyService {
        type Request;
        type Response;
        type Error;
        type Future: Future<Item = Self::Response, Error = Self::Error>;
        fn call(&self, req: Self::Request) -> Self::Future;
    }

    struct LowerService<S>(S);

    // This works for `MyService` but not for the real `Service` (which has an
    // identical definition)

    impl<Req, Resp, E, S> MyService for LowerService<S> where
        S: Service<Request = Message<Req, stream::Empty<(), E>>,
                   Response = Message<Resp, Body<(), E>>,
                   Error = E>,
    {
        type Request = Req;
        type Response = Resp;
        type Error = E;
        type Future = LowerFuture<Body<(), E>, Resp, E>;

        fn call(&self, req: Req) -> Self::Future {
            LowerFuture(Box::new(self.0.call(Message::WithoutBody(req))))
        }
    }

    struct LowerFuture<Resp, B, E>(Box<Future<Item = Message<Resp, B>, Error = E>>);

    impl<Resp, B, E> Future for LowerFuture<B, Resp, E> {
        type Item = Resp;
        type Error = E;
    }

    // Trying to lift MyService to Service doesn't work either
    struct Bug<T>(T);
    impl<T: MyService> Service for Bug<T> {
        type Request = T::Request;
        type Response = T::Response;
        type Error = T::Error;
        type Future = T::Future;

        fn call(&self, req: Self::Request) -> Self::Future {
            self.0.call(req)
        }
    }
}
*/
