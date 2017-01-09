#[derive(Debug, Default, Clone)]
pub struct Signer {
    key: String,
}

impl Signer {
    pub fn new(key: String) -> Signer {
        return Signer { key: key };
    }
}
