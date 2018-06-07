#[macro_export]
macro_rules! make_message_handler(
    ($node:ident) => (
        fn message(&mut self, msg: ::audio::node::AudioNodeMessage, sample_rate: f32) {
            match msg {
                ::audio::node::AudioNodeMessage::$node(m) => self.handle_message(m, sample_rate),
                _ => (),
            }
        }
    );
);
