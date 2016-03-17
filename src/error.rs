use rotor_stream;

use serialization;

quick_error! {
    #[derive(Debug)]
    pub enum Error {
        Serialization(err: serialization::Error) {
            cause(err)
            description(err.description())
        }
        Stream(err: rotor_stream::Exception) {
            cause(err)
            description(err.description())
            display("{}", err)
        }
    }
}
