extern crate capnp;
extern crate rotor;
extern crate rotor_capnp;

mod messages_capnp {
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/messages_capnp.rs"));
}

use std::time::Duration;

use rotor::{Config as LoopConfig, Loop, Scope};
use rotor::mio::tcp::{TcpListener, TcpStream};
use rotor_capnp::{Accept, Action, CapnpStream, ConnectionState, Endpoint, Error, MessageReader,
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

struct EchoServer(usize);

impl Endpoint for EchoServer {
    type Context = Metrics;
    type Socket = TcpStream;
    type Seed = ();

    fn create(_seed: Self::Seed,
              sock: &mut Self::Socket,
              _scope: &mut Scope<Self::Context>)
              -> Action<Self> {
        println!("[server] new connection from {}", sock.peer_addr().unwrap());
        Action::Idle(EchoServer(0))
    }

    fn message_received(self,
                        message: &MessageReader,
                        mut output: MessageWriter,
                        scope: &mut Scope<Self::Context>)
                        -> Action<Self> {
        scope.requests += 1;
        let request_id = self.0 + 1;
        let request = message.get_root::<request::Reader>().unwrap();
        let client = request.get_client();
        let content = request.get_content().unwrap();
        println!("[server] request {} from client {}: {}",
                 request_id,
                 client,
                 content);
        let mut builder = MessageBuilder::new_default();
        {
            let mut response = builder.init_root::<response::Builder>();
            response.set_content(content);
        }
        output.write(&builder);
        Action::Flush(EchoServer(request_id))
    }

    fn message_flushed(self,
                       _output: MessageWriter,
                       _scope: &mut Scope<Self::Context>)
                       -> Action<Self> {
        Action::Idle(self)
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
                println!("[server] closing idle connection after {} request(s)",
                         self.0)
            }
            _ => println!("[server] timed out while \"{:?}\"", state),
        };
        Action::Close
    }

    fn wakeup(&self, _scope: &mut Scope<Self::Context>) -> Action<Self> {
        unreachable!()
    }

    // Connection will be closed after this
    fn exception(self, err: Error, _scope: &mut Scope<Self::Context>) {
        println!("[server] {}, closing connection", err);
    }
}

fn main() {
    let loop_creator = Loop::new(&LoopConfig::new()).unwrap();
    let mut loop_inst = loop_creator.instantiate(Metrics::new());
    let socket = TcpListener::bind(&"127.0.0.1:3055".parse().unwrap()).unwrap();

    loop_inst.add_machine_with(|scope| {
                 Accept::<CapnpStream<EchoServer>, TcpListener>::new(socket, (), scope)
             })
             .unwrap();
    loop_inst.run().unwrap();
}
