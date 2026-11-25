//! Transaction commands on images, external textures and shaders.

use crate::id::{ImageId, ShaderId, TextureId};
use crate::resource::{ImageDesc, ImageWrite, ShaderDesc, TextureDesc};
use crate::transaction::{Command, Transaction};

impl Transaction {
    /// Creates an image of transparent texels.
    pub fn create_image(&mut self, desc: ImageDesc) -> ImageId {
        let id = self.mint();
        self.commands.push(Command::CreateImage(id, desc));
        id
    }

    /// Replaces texels of an image.
    pub fn write_image(&mut self, id: ImageId, write: ImageWrite) {
        self.commands.push(Command::WriteImage(id, write));
    }

    /// Creates an external texture slot.
    pub fn create_texture(&mut self, desc: TextureDesc) -> TextureId {
        let id = self.mint();
        self.commands.push(Command::CreateTexture(id, desc));
        id
    }

    /// Reports that new content is bound to an external texture slot.
    pub fn texture_frame(&mut self, id: TextureId) {
        self.commands.push(Command::TextureFrame(id));
    }

    /// Creates an application shader.
    pub fn create_shader(&mut self, desc: ShaderDesc) -> ShaderId {
        let id = self.mint();
        self.commands.push(Command::CreateShader(id, desc));
        id
    }
}
