extern crate gpgme;
extern crate std;

#[derive(Debug)]
pub enum Error {
    Gpgme(gpgme::Error),
    Io(std::io::Error),
}

impl From<gpgme::Error> for Error {
    fn from(e: gpgme::Error) -> Self {
        Error::Gpgme(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

#[derive(Debug, Clone)]
pub struct Signer<'a> {
    key: &'a str,
}

impl<'a> Signer<'a> {
    pub fn new(key: &'a str) -> Signer {
        Signer { key: key }
    }

    pub fn sign<P, Q>(&self, path: P, sig_path: Q) -> Result<(), Error>
        where P: AsRef<std::path::Path>,
              Q: AsRef<std::path::Path>
    {
        let mut ctx = try!(gpgme::create_context());
        try!(ctx.set_protocol(gpgme::PROTOCOL_OPENPGP));
        let key = try!(ctx.find_secret_key(self.key.to_owned()));
        try!(ctx.add_signer(&key));
        let mut input = try!(gpgme::Data::load(path.as_ref()));
        let writer = try!(std::fs::File::create(sig_path));
        match gpgme::Data::from_writer(writer) {
            Ok(mut output) => {
                try!(ctx.sign(gpgme::ops::SIGN_MODE_DETACH, &mut input, &mut output));
                Ok(())
            }
            Err(wrapped_error) => Err(Error::from(wrapped_error.error())),
        }
    }
}
