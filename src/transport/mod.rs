use std::io;
use tokio_core::io::Io;
use futures::{Stream, Sink};

pub trait Transport: Stream + Sink<SinkError = <Self as Stream>::Error> {
    fn tick(&mut self) {}
}

/*
impl<M1, M2, B1, B2, E> Transport for Box<Transport<In = M1, Out = M2, BodyIn = B1, BodyOut = B2, Error = E>>
    where E: From<Error<E>> + 'static,
          M1: 'static,
          M2: 'static,
          B1: 'static,
          B2: 'static,
{
    type In = M1;
    type BodyIn = B1;
    type Out = M2;
    type BodyOut = B2;
    type Error = E;

    fn poll_read(&mut self) -> Async<()> {
        (**self).poll_read()
    }

    fn read(&mut self) -> Poll<Frame<M2, B2, E>, io::Error> {
        (**self).read()
    }

    fn poll_write(&mut self) -> Async<()> {
        (**self).poll_write()
    }

    fn write(&mut self, req: Frame<M1, B1, E>) -> Poll<(), io::Error> {
        (**self).write(req)
    }

    fn flush(&mut self) -> Poll<(), io::Error> {
        (**self).flush()
    }
}

impl<M1, M2, B1, B2, E> Transport for Box<Transport<In = M1, Out = M2, BodyIn = B1, BodyOut = B2, Error = E> + Send>
    where E: From<Error<E>> + 'static,
          M1: 'static,
          M2: 'static,
          B1: 'static,
          B2: 'static,
{
    type In = M1;
    type BodyIn = B1;
    type Out = M2;
    type BodyOut = B2;
    type Error = E;

    fn poll_read(&mut self) -> Async<()> {
        (**self).poll_read()
    }

    fn read(&mut self) -> Poll<Frame<M2, B2, E>, io::Error> {
        (**self).read()
    }

    fn poll_write(&mut self) -> Async<()> {
        (**self).poll_write()
    }

    fn write(&mut self, req: Frame<M1, B1, E>) -> Poll<(), io::Error> {
        (**self).write(req)
    }

    fn flush(&mut self) -> Poll<(), io::Error> {
        (**self).flush()
    }
}

pub struct CodecTransport<C: Codec> {
    framed: Box<Stream<Item = C::In, Error = io::Error> +
                Sink<SinkItem = C::Out, SinkError = io::Error> +
                Send>
}

impl<C: Codec> CodecTransport<C> {
    fn bind<I: Io>(codec: C, io: I) -> CodecTransport<C> {
        CodecTransport {
            framed: Box::new(io.framed(codec))
        }
    }
}

impl<C: Codec> Stream for CodecTransport<C> {
    type Item = C::In;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<C::In>, io::Error> {
        self.framed.poll()
    }
}

impl<C: Codec> Sink for CodecTransport<C> {
    type SinkItem = C::Out;
    type SinkError = io::Error;

    fn start_send(&mut self, item: C::Out) -> StartSend<C::Out, io::Error> {
        self.framed.start_send(item)
    }

    fn poll_complete(&mut self) -> Poll<(), io::Error> {
        self.framed.poll_complete()
    }
}
*/
