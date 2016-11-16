//! A collection of components for rapid protocol development

#![deny(warnings, missing_docs)]

extern crate bytes;
extern crate slab;
extern crate take;
extern crate rand;
extern crate smallvec;
extern crate tokio_core;
extern crate tokio_service;

#[macro_use]
extern crate futures;

#[macro_use]
extern crate log;

mod rpc;
mod streaming;
mod transport;
mod util;
mod error;

use tokio_core::io::Io;
use tokio_core::reactor::Handle;
use futures::BoxFuture;
use tokio_service::Service;

////////////////////////////////////////////////////////////////////////////////

pub trait BindServer<Kind, T: Io> {
    type ServiceRequest;
    type ServiceResponse;
    type ServiceError;

    fn bind_server<S>(&self, handle: &Handle, io: T, service: S)
                      -> BoxFuture<(), Self::ServiceError>
        where S: Service<Request = Self::ServiceRequest,
                         Response = Self::ServiceResponse,
                         Error = Self::ServiceError>;
}

pub trait BindClient<Kind, T: Io> {
    type ServiceRequest;
    type ServiceResponse;
    type ServiceError;

    type BindClient: Service<Request = Self::ServiceRequest,
                             Response = Self::ServiceResponse,
                             Error = Self::ServiceError>;
    fn bind_client(&self, handle: &Handle, io: T) -> Self::BindClient;
}

pub struct Client<P> {
    proto: P,
}

pub struct Server<P> {
    proto: P,
}

//pub fn server<P>();
/*
impl<P> Server<P> {
    pub fn serve<Kind, S>(&self, service: S) where
        S: NewService,
        P: BindServe<Kind, TcpStream, S::Instance>,

}
*/
