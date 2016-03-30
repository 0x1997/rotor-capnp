//! mio based async stream for Cap'n Proto messages
//!
extern crate byteorder;
extern crate capnp;
extern crate rotor;
extern crate rotor_stream;
#[macro_use]
extern crate quick_error;

mod error;
mod protocol;
mod serialization;
mod stream;

pub use rotor_stream::{Accept, Persistent, Stream};

pub use error::Error;
pub use protocol::{Action, ConnectionState, Endpoint};
pub use serialization::{MessageReader, MessageBuilder, MessageWriter};
pub use stream::Capnp;

/// State machine for the Cap'n Proto message stream.
pub type CapnpStream<E> = Stream<Capnp<E>>;