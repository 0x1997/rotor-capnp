use rotor_stream;

use serialization;

quick_error! {
    /// Error type for the underlying state machine
    #[derive(Debug)]
    pub enum Error {
        /// Cap'n Proto (de)serialization error.
        /// See `capnp::Error` for details.
        Serialization(err: serialization::Error) {
            cause(err)
            description(err.description())
        }
        /// Error reading from or writing to the connection.
        /// See `rotor_stream::Exception` for details.
        Stream(err: rotor_stream::Exception) {
            cause(err)
            description(err.description())
            display("{}", err)
        }
    }
}
