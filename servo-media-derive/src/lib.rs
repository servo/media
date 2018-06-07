extern crate proc_macro;
extern crate syn;
#[macro_use]
extern crate quote;

use proc_macro::TokenStream;

#[proc_macro_derive(AudioScheduledSourceNode)]
pub fn audio_scheduled_source_node(input: TokenStream) -> TokenStream {
    let s = input.to_string();
    let ast = syn::parse_derive_input(&s).unwrap();
    let gen = impl_audio_scheduled_source_node(&ast);
    gen.parse().unwrap()
}

fn impl_audio_scheduled_source_node(ast: &syn::DeriveInput) -> quote::Tokens {
    let name = &ast.ident;
    quote! {
        use audio::node::AudioScheduledSourceNode;

        impl #name {
            pub fn should_play_at(&self, tick: Tick) -> (bool, bool) {
                if self.start_at.is_none() {
                    return (false, true);
                }

                if tick < self.start_at.unwrap() {
                    (false, false)
                } else {
                    if let Some(stop_at) = self.stop_at {
                        if tick >= stop_at {
                            return (false, true);
                        }
                    }
                    (true, false)
                }
            }
        }

        impl AudioScheduledSourceNode for #name {
            fn start(&mut self, tick: Tick) -> bool {
                // We can only allow a single call to `start` and always before
                // any `stop` calls.
                if self.start_at.is_some() || self.stop_at.is_some() {
                    return false;
                }
                self.start_at = Some(tick);
                true
            }

            fn stop(&mut self, tick: Tick) -> bool {
                // We can only allow calls to `stop` after `start` is called.
                if self.start_at.is_none() {
                    return false;
                }
                // If `stop` is called again after already having been called,
                // the last invocation will be the only one applied.
                self.stop_at = Some(tick);
                true
            }
        }
    }
}
