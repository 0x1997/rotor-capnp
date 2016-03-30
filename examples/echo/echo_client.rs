extern crate capnp;
extern crate rotor;
extern crate rotor_capnp;

mod messages_capnp {
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/messages_capnp.rs"));
}

use std::env::{self, Args};
use std::time::Duration;

use rotor::{Config as LoopConfig, Loop, Scope};
use rotor::mio::tcp::TcpStream;
use rotor_capnp::{Action, CapnpStream, ConnectionState, Endpoint, Error, MessageReader,
                  MessageBuilder, MessageWriter};

use messages_capnp::{request, response};

struct Metrics {
    requests: usize,
}

impl Metrics {
    fn new() -> Metrics {
        Metrics { requests: 0 }
    }
}

struct EchoClient(Args);

impl EchoClient {
    fn send_request(mut self,
                    mut output: MessageWriter,
                    scope: &mut Scope<Metrics>)
                    -> Action<Self> {
        if let Some(content) = self.0.next() {
            scope.requests += 1;
            println!("[client] sending request: {}", content);
            let mut builder = MessageBuilder::new_default();
            {
                let mut request = builder.init_root::<request::Builder>();
                request.set_client(7);
                request.set_content(&content);
            }
            output.write(&builder);
            Action::Recv(self)
        } else {
            println!("[client] closing connection");
            Action::Close
        }
    }
}

impl Endpoint for EchoClient {
    type Context = Metrics;
    type Socket = TcpStream;
    type Seed = Args;

    fn create(seed: Self::Seed,
              _sock: &mut Self::Socket,
              _scope: &mut Scope<Self::Context>)
              -> Action<Self> {
        Action::Flush(EchoClient(seed))
    }

    fn message_received(self,
                        message: &MessageReader,
                        output: MessageWriter,
                        scope: &mut Scope<Self::Context>)
                        -> Action<Self> {
        let response = message.get_root::<response::Reader>().unwrap();
        let content = response.get_content().unwrap();
        println!("[client] received response: {}", content);
        self.send_request(output, scope)
    }

    fn message_flushed(self,
                       output: MessageWriter,
                       scope: &mut Scope<Self::Context>)
                       -> Action<Self> {
        self.send_request(output, scope)
    }

    fn idle_timeout(&self, _scope: &mut Scope<Self::Context>) -> Duration {
        Duration::from_secs(10)
    }

    fn recv_timeout(&self, _scope: &mut Scope<Self::Context>) -> Duration {
        Duration::from_secs(10)
    }

    fn send_timeout(&self, _scope: &mut Scope<Self::Context>) -> Duration {
        Duration::from_secs(10)
    }

    fn timeout(self,
               state: ConnectionState,
               _output: MessageWriter,
               _scope: &mut Scope<Self::Context>)
               -> Action<Self> {
        match state {
            ConnectionState::Idle => {
                println!("[client] closing idle connection")
            }
            _ => println!("[client] timed out while \"{:?}\"", state),
        };
        Action::Close
    }

    fn wakeup(&self, _scope: &mut Scope<Self::Context>) -> Action<Self> {
        unreachable!()
    }

    // Connection will be closed after this
    fn exception(self, err: Error, _scope: &mut Scope<Self::Context>) {
        println!("[client] {}, closing connection", err);
    }
}

fn main() {
    let mut payloads = env::args();
    payloads.next().unwrap();
    let loop_creator = Loop::new(&LoopConfig::new()).unwrap();
    let mut loop_inst = loop_creator.instantiate(Metrics::new());
    let socket = TcpStream::connect(&"127.0.0.1:3055".parse().unwrap()).unwrap();

    loop_inst.add_machine_with(|scope| {
                 CapnpStream::<EchoClient>::new(socket, payloads, scope)
             })
             .unwrap();
    loop_inst.run().unwrap();
}
