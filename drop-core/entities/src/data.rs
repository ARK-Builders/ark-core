pub trait Data: Send + Sync {
    fn len(&self) -> u64;
    fn read(&self) -> Option<u8>;
}
