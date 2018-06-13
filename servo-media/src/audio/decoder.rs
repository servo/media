pub trait AudioDecoder {
    fn decode(&self, data: Vec<u8>) -> Result<(), ()>;
}
