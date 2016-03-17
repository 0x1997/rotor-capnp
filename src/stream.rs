use std::time::Duration;

use rotor::{Scope, Time};
use rotor_stream::{Exception, Intent, IntentBuilder, Protocol, Transport};

use error::Error;
use protocol::{Action, ConnectionState, Endpoint};
use serialization::{self, MessageWriter};

#[derive(Debug)]
enum Reading {
    SegmentCount,
    SegmentTable(usize),
    Segments(usize, Vec<(usize, usize)>),
}

#[derive(Debug)]
enum CapnpState {
    Idle,
    Reading(Reading),
    Writing,
    Sleeping,
}

pub struct Capnp<E: Endpoint> {
    fsm: E,
    state: CapnpState,
}

impl<E: Endpoint> Capnp<E> {
    fn intent(fsm: E, state: CapnpState) -> IntentBuilder<Self> {
        Intent::of(Capnp {
            fsm: fsm,
            state: state,
        })
    }

    fn from_action(action: Action<E>, scope: &mut Scope<E::Context>) -> Intent<Self> {
        match action {
            Action::Idle(fsm) => Capnp::intent_idle(fsm, scope),
            Action::Recv(fsm) => Capnp::intent_read(fsm, scope),
            Action::Flush(fsm) => Capnp::intent_flush(fsm, scope),
            Action::Sleep(fsm, timeout) => Capnp::intent_sleep(fsm, scope, timeout),
            Action::Close => Intent::done(),
        }
    }

    fn intent_idle(fsm: E, scope: &mut Scope<E::Context>) -> Intent<Self> {
        let deadline = scope.now() + fsm.idle_timeout(scope);
        Capnp::intent(fsm, CapnpState::Idle)
            .expect_bytes(1)
            .deadline(deadline)
    }

    fn intent_read(fsm: E, scope: &mut Scope<E::Context>) -> Intent<Self> {
        let deadline = scope.now() + fsm.recv_timeout(scope);
        Capnp::intent(fsm, CapnpState::Reading(Reading::SegmentCount))
            .expect_bytes(4)
            .deadline(deadline)
    }

    fn intent_continue_read(fsm: E,
                            transport: &mut Transport<E::Socket>,
                            state: Reading,
                            scope: &mut Scope<E::Context>,
                            deadline: Time)
                            -> Intent<Self> {
        use self::CapnpState::Reading;
        use self::Reading::*;
        match state {
            SegmentCount => {
                match serialization::read_segment_count(transport.input()) {
                    Ok(segment_count) => {
                        Capnp::intent(fsm, Reading(SegmentTable(segment_count)))
                            .expect_bytes(segment_count * 4)
                            .deadline(deadline)
                    }
                    Err(err) => {
                        fsm.exception(Error::Serialization(err), scope);
                        Intent::done()
                    }
                }
            }
            SegmentTable(segment_count) => {
                match serialization::read_segment_table(transport.input(),
                                                        segment_count,
                                                        fsm.reader_options(scope)) {
                    Ok((total_words, segment_slices)) => {
                        Capnp::intent(fsm, Reading(Segments(total_words, segment_slices)))
                            .expect_bytes(total_words * 8)
                            .deadline(deadline)
                    }
                    Err(err) => {
                        fsm.exception(Error::Serialization(err), scope);
                        Intent::done()
                    }
                }
            }
            Segments(total_words, segment_slices) => {
                match serialization::read_segments(transport.input(),
                                                   total_words,
                                                   segment_slices,
                                                   fsm.reader_options(scope)) {
                    Ok(message) => {
                        let action = fsm.message_received(&message,
                                                          MessageWriter(transport.output()),
                                                          scope);
                        Capnp::from_action(action, scope)
                    }
                    Err(err) => {
                        fsm.exception(Error::Serialization(err), scope);
                        Intent::done()
                    }
                }
            }
        }
    }

    fn intent_flush(fsm: E, scope: &mut Scope<E::Context>) -> Intent<Self> {
        let deadline = scope.now() + fsm.send_timeout(scope);
        Capnp::intent(fsm, CapnpState::Writing)
            .expect_flush()
            .deadline(deadline)
    }

    fn intent_sleep(fsm: E, scope: &mut Scope<E::Context>, timeout: Duration) -> Intent<Self> {
        Capnp::intent(fsm, CapnpState::Sleeping).sleep().deadline(scope.now() + timeout)
    }
}

impl<E: Endpoint> Protocol for Capnp<E> {
    type Context = E::Context;
    type Socket = E::Socket;
    type Seed = E::Seed;

    fn create(seed: Self::Seed,
              sock: &mut Self::Socket,
              scope: &mut Scope<Self::Context>)
              -> Intent<Self> {
        let action = E::create(seed, sock, scope);
        Capnp::from_action(action, scope)
    }

    fn bytes_read(self,
                  transport: &mut Transport<Self::Socket>,
                  _end: usize,
                  scope: &mut Scope<Self::Context>)
                  -> Intent<Self> {
        let state = match self.state {
            CapnpState::Idle => {
                if transport.input().len() < 4 {
                    return Capnp::intent_read(self.fsm, scope);
                } else {
                    Reading::SegmentCount
                }
            }
            CapnpState::Reading(state) => state,
            _ => unreachable!(),
        };
        let deadline = scope.now() + self.fsm.recv_timeout(scope);
        Capnp::intent_continue_read(self.fsm, transport, state, scope, deadline)
    }

    fn bytes_flushed(self,
                     transport: &mut Transport<Self::Socket>,
                     scope: &mut Scope<Self::Context>)
                     -> Intent<Self> {
        match self.state {
            CapnpState::Writing => {
                let action = self.fsm.message_flushed(MessageWriter(transport.output()), scope);
                Capnp::from_action(action, scope)
            }
            _ => unreachable!(),
        }
    }

    fn timeout(self,
               transport: &mut Transport<Self::Socket>,
               scope: &mut Scope<Self::Context>)
               -> Intent<Self> {
        let state = match self.state {
            CapnpState::Idle => ConnectionState::Idle,
            CapnpState::Reading(_) => ConnectionState::Receiving,
            CapnpState::Writing => ConnectionState::Sending,
            CapnpState::Sleeping => ConnectionState::Sleeping,
        };
        let action = self.fsm.timeout(state, MessageWriter(transport.output()), scope);
        Capnp::from_action(action, scope)
    }

    fn wakeup(self,
              _transport: &mut Transport<Self::Socket>,
              scope: &mut Scope<Self::Context>)
              -> Intent<Self> {
        let action = self.fsm.wakeup(scope);
        Capnp::from_action(action, scope)
    }

    fn exception(self,
                 _transport: &mut Transport<Self::Socket>,
                 reason: Exception,
                 scope: &mut Scope<Self::Context>)
                 -> Intent<Self> {
        match reason {
            Exception::EndOfStream => {
                if let CapnpState::Reading(_) = self.state {
                    self.fsm.exception(Error::Stream(reason), scope);
                }
                Intent::done()
            }
            Exception::LimitReached => unreachable!(),
            _ => {
                self.fsm.exception(Error::Stream(reason), scope);
                Intent::done()
            }
        }
    }
}
