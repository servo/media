#[macro_export]
macro_rules! make_message_handler(
    ($node:ident) => (
        fn message(&mut self, msg: ::audio::node::AudioNodeMessage, sample_rate: f32) {
            match msg {
                ::audio::node::AudioNodeMessage::$node(m) => self.handle_message(m, sample_rate),
                ::audio::node::AudioNodeMessage::GetInputCount(tx) => tx.send(self.input_count()).unwrap(),
                ::audio::node::AudioNodeMessage::GetOutputCount(tx) => tx.send(self.output_count()).unwrap(),
                ::audio::node::AudioNodeMessage::GetChannelCount(tx) => tx.send(self.channel_count()).unwrap(),
                _ => (),
            }
        });
    );

#[macro_export]
macro_rules! make_state_change(
    ($fn_name:ident, $state:ident, $render_msg:ident) => (
        pub fn $fn_name(&self) -> StateChangeResult {
            self.state.set(ProcessingState::$state);
            let (tx, rx) = mpsc::channel();
            let _ = self.sender.send(AudioRenderThreadMsg::$render_msg(tx));
            rx.recv().unwrap()
        });
    );

#[macro_export]
macro_rules! make_render_thread_state_change(
    ($fn_name:ident, $state:ident, $sink_method:ident) => (
        fn $fn_name(&mut self) -> StateChangeResult {
            if self.state == ProcessingState::$state {
                return Ok(());
            }
            self.state = ProcessingState::$state;
            self.sink.$sink_method()
        }
    );
);
