use BindClient;
use super::{RequestId, RpcMultiplex, LiftTransport};

use std::io;

use streaming::{self, Message, Body};
use tokio_core::io::Io;
use tokio_core::reactor::Handle;
use tokio_service::Service;
use util::client_proxy::ClientProxy;
use transport::Transport;
use futures::{stream, Future, Stream, Sink, Poll};

pub trait Client<T: Io> {
    type Request;
    type Response;
    type Error: From<io::Error>;

    type Transport: Transport<Item = (RequestId, Self::Response),
                              SinkItem = (RequestId, Self::Request),
                              Error = Self::Error>;
    fn bind_transport(&self, io: T) -> Self::Transport;
}

impl<T: Io, P: Client<T>> BindClient<RpcMultiplex, T> for P {
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

impl<'a, T, P> streaming::multiplex::Client<T> for LiftProto<'a, P> where
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
    S: Service<Request = streaming::Message<Req, stream::Empty<(), E>>,
               Response = streaming::Message<Resp, streaming::Body<(), E>>>
{
    type Request = Req;
    type Response = Resp;
    type Error = E;
    type Future = LowerFuture<S::Future>;

    fn call(&self, req: Req) -> Self::Future {
        LowerFuture(self.0.call(Message::WithoutBody(req)))
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
            Message::WithBOdy(..) => panic!("bodies not supported"),
        }
    }
}
