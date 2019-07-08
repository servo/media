/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::num::NonZeroU32;

/// An ID for clients to track instances of Players and AudioContexts belonging to the same tab and mute them simultaneously.
/// Current tuple implementation matches one of Servo's BrowsingContextId.
#[derive(PartialEq, Eq, Hash, Debug, Clone, Copy)]
pub struct ClientContextId(u32, NonZeroU32);

impl ClientContextId {
    pub fn build(a: u32, b: u32) -> ClientContextId {
        ClientContextId(a, NonZeroU32::new(b).unwrap())
    }
}

pub trait MediaInstance {
    fn get_id(&self) -> usize;
}

pub trait Muteable: Send + MediaInstance {
    fn mute(&self, val: bool) -> Result<(), ()>;
}

pub enum BackendMsg {
    /// Message to notify about a media instance shutdown.
    /// The given `usize` is the media instance ID.
    Shutdown(ClientContextId, usize),
}
