extern crate gpgme;
extern crate std;

#[derive(Debug, Default, Clone)]
pub struct Signer {
    key: String,
}

impl Signer {
    pub fn new(key: String) -> Signer {
        return Signer { key: key };
    }

    pub fn sign<P: AsRef<std::path::Path>, Q: AsRef<std::path::Path>>(&self,
                                                                      path: P,
                                                                      sig_path: Q) {
        let mut ctx = gpgme::create_context().unwrap();
        ctx.set_protocol(gpgme::PROTOCOL_OPENPGP).unwrap();
        ctx.set_armor(true);
        let key = ctx.find_secret_key(self.key.to_owned()).unwrap();
        ctx.add_signer(&key).unwrap();
        let mut input = gpgme::Data::load(&path).unwrap();
        let writer = std::fs::File::create(sig_path).unwrap();
        let mut output = gpgme::Data::from_writer(writer).unwrap();
        ctx.sign(gpgme::ops::SIGN_MODE_DETACH, &mut input, &mut output).unwrap();
    }
}
