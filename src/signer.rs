#[derive(Debug, Clone)]
pub struct Signer<'a> {
    key: &'a str,
}

impl<'a> Signer<'a> {
    pub fn new(key: &'a str) -> Signer {
        Signer { key }
    }

    pub fn sign<P, Q>(&self, path: P, sig_path: Q) -> Result<(), anyhow::Error>
    where
        P: AsRef<std::path::Path>,
        Q: AsRef<std::path::Path>,
    {
        let mut ctx = gpgme::Context::from_protocol(gpgme::Protocol::OpenPgp)?;
        let key = ctx.get_secret_key(self.key)?;
        ctx.add_signer(&key)?;
        let reader = std::fs::File::open(path)?;
        let mut input = gpgme::Data::from_reader(reader)?;
        let writer = std::fs::File::create(sig_path)?;
        let mut output = gpgme::Data::from_writer(writer)?;
        ctx.sign(gpgme::SignMode::Detached, &mut input, &mut output)?;
        Ok(())
    }
}
