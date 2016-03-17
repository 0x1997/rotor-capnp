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

pub type CapnpStream<E> = Stream<Capnp<E>>;