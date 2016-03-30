use std::time::Duration;

use rotor::Scope;
use rotor_stream::StreamSocket;

use error::Error;
use serialization::{MessageReader, MessageWriter, ReaderOptions};

/// Wrapper of the new state of `Endpoint` and the next action.
pub enum Action<E: Endpoint> {
    /// Wait for arrival of a new message until the timeout expires.
    Idle(E),
    /// Receive new message until the timeout expires.
    Recv(E),
    /// Flush the write buffer until the timeout expires.
    Flush(E),
    /// Sleep until the specified the timeout expires.
    Sleep(E, Duration),
    /// Close the connection immediately, pending data in the buffers will be discarded.
    Close,
}

/// State of the underlying connection
#[derive(Debug)]
pub enum ConnectionState {
    Idle,
    Receiving,
    Sending,
    Sleeping,
}

/// A handler for receiving and sending Cap'n Proto messages.
///
/// Currently this is used by both client side and server side of the connection.
/// Client specific abstractions might be added in the future.
pub trait Endpoint: Sized {
    /// Context shared between transitions of the state machine. 
    type Context;
    /// Type of the underlying socket.
    type Socket: StreamSocket;
    /// Seed for initializing the state machine.
    type Seed;

    /// A new connection has been established.
    fn create(seed: Self::Seed,
              sock: &mut Self::Socket,
              scope: &mut Scope<Self::Context>)
              -> Action<Self>;

    /// A new message has been received.
    fn message_received(self,
                        message: &MessageReader,
                        output: MessageWriter,
                        scope: &mut Scope<Self::Context>)
                        -> Action<Self>;

    /// All outgoing messages have been flushed.
    fn message_flushed(self,
                       output: MessageWriter,
                       scope: &mut Scope<Self::Context>)
                       -> Action<Self>;

    /// Options for the Cap'n Proto message reader.
    fn reader_options(&self, _scope: &mut Scope<Self::Context>) -> ReaderOptions {
        ReaderOptions::new()
    }

    /// Timeout for an idle connection. By default it's 120 seconds.
    fn idle_timeout(&self, _scope: &mut Scope<Self::Context>) -> Duration {
        Duration::from_secs(120)
    }

    /// Timeout for reading a message.
    fn recv_timeout(&self, scope: &mut Scope<Self::Context>) -> Duration;
    
    /// Timeout for sending a message.
    fn send_timeout(&self, scope: &mut Scope<Self::Context>) -> Duration;

    /// Timeout expired during the `state`.
    fn timeout(self,
               state: ConnectionState,
               output: MessageWriter,
               scope: &mut Scope<Self::Context>)
               -> Action<Self>;

    /// The state machine has been woken up.
    fn wakeup(&self, scope: &mut Scope<Self::Context>) -> Action<Self>;

    /// Connection will be closed after this.
    fn exception(self, err: Error, scope: &mut Scope<Self::Context>);
}
