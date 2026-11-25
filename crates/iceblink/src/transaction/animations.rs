//! Transaction commands on animations.

use crate::animation::Animation;
use crate::id::AnimationId;
use crate::transaction::{Command, Transaction};

impl Transaction {
    /// Starts an animation, replacing any animation of the same target.
    pub fn animate(&mut self, animation: Animation) -> AnimationId {
        let id = self.mint();
        self.commands
            .push(Command::Animate(id, Box::new(animation)));
        id
    }

    /// Cancels an animation, so its target returns to the value the client last wrote.
    pub fn cancel_animation(&mut self, id: AnimationId) {
        self.commands.push(Command::CancelAnimation(id));
    }
}
