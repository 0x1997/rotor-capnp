use std::time::Duration;

use rotor::Scope;
use rotor_stream::StreamSocket;

use error::Error;
use serialization::{MessageReader, MessageWriter, ReaderOptions};

pub enum Action<E: Endpoint> {
    /// Wait for arrival of new messages until timeout reached.
    Idle(E),
    /// Receive new message
    Recv(E),
    /// Flush the write buffer
    Flush(E),
    /// Sleep until timed out
    Sleep(E, Duration),
    /// Close the connection immediately, pending data in the buffers will be discarded
    Close,
}

#[derive(Debug)]
pub enum ConnectionState {
    Idle,
    Receiving,
    Sending,
    Sleeping,
}

pub trait Endpoint: Sized {
    type Context;
    type Socket: StreamSocket;
    type Seed;

    fn create(seed: Self::Seed,
              sock: &mut Self::Socket,
              scope: &mut Scope<Self::Context>)
              -> Action<Self>;

    fn message_received(self,
                        message: &MessageReader,
                        output: MessageWriter,
                        scope: &mut Scope<Self::Context>)
                        -> Action<Self>;

    fn message_flushed(self,
                       output: MessageWriter,
                       scope: &mut Scope<Self::Context>)
                       -> Action<Self>;

    fn reader_options(&self, _scope: &mut Scope<Self::Context>) -> ReaderOptions {
        ReaderOptions::new()
    }

    /// 120 seconds by default.
    fn idle_timeout(&self, _scope: &mut Scope<Self::Context>) -> Duration {
        Duration::from_secs(120)
    }

    fn recv_timeout(&self, scope: &mut Scope<Self::Context>) -> Duration;
    fn send_timeout(&self, scope: &mut Scope<Self::Context>) -> Duration;

    fn timeout(self,
               state: ConnectionState,
               output: MessageWriter,
               scope: &mut Scope<Self::Context>)
               -> Action<Self>;

    fn wakeup(&self, scope: &mut Scope<Self::Context>) -> Action<Self>;

    /// Connection will be closed after this
    fn exception(self, err: Error, scope: &mut Scope<Self::Context>);
}
