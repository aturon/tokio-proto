//! Dispatch for multiplexed protocols
//!
//! This module contains reusable components for quickly implementing clients
//! and servers for multiplex based protocols.
//!
//! ## Multiplexing
//!
//! Multiplexing allows multiple request / response transactions to be inflight
//! concurrently while sharing the same underlying transport. This is usually
//! done by splitting the protocol into frames and assigning a request ID to
//! each frame.
//!
//! ## Considerations
//!
//! There are some difficulties with implementing back pressure in the case
//! that the wire protocol does not support a means by which backpressure can
//! be signaled to the peer.
//!
//! The problem is that, the transport is no longer read from while waiting for
//! the current frames to be processed, it is possible that the processing
//! logic is blocked waiting for another frame that is currently pending on the
//! socket.
//!
//! To deal with this, once the connection level frame buffer is filled, a
//! timeout is set. If no further frames are able to be read before the timeout
//! expires, then the connection is killed.

mod frame_buf;

mod client;
pub use self::client::Client;

mod server;
pub use self::server::Server;

mod frame;
pub use self::frame::Frame;

pub mod advanced;

/// Identifies a request / response thread
pub type RequestId = u64;

pub struct StreamingMultiplex<B>(B);
