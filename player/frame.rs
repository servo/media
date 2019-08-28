use std::sync::Arc;

#[derive(Clone)]
pub enum FrameData {
    Raw(Arc<Vec<u8>>),
    Texture(u32),
    OESTexture(u32),
}

pub trait Buffer: Send + Sync {
    fn to_vec(&self) -> Result<FrameData, ()>;
}

#[derive(Clone)]
pub struct Frame {
    width: i32,
    height: i32,
    data: FrameData,
    buffer: Arc<dyn Buffer>,
}

impl Frame {
    pub fn new(width: i32, height: i32, buffer: Arc<dyn Buffer>) -> Result<Self, ()> {
        let data = buffer.to_vec()?;

        Ok(Frame {
            width,
            height,
            data,
            buffer,
        })
    }

    pub fn get_width(&self) -> i32 {
        self.width
    }

    pub fn get_height(&self) -> i32 {
        self.height
    }

    pub fn get_data(&self) -> Arc<Vec<u8>> {
        match self.data {
            FrameData::Raw(ref data) => data.clone(),
            _ => unreachable!("invalid raw data request for texture frame"),
        }
    }

    pub fn get_texture_id(&self) -> u32 {
        match self.data {
            FrameData::Texture(data) | FrameData::OESTexture(data) => data,
            _ => unreachable!("invalid texture id request for raw data frame"),
        }
    }

    pub fn is_gl_texture(&self) -> bool {
        match self.data {
            FrameData::Texture(_) | FrameData::OESTexture(_) => true,
            _ => false,
        }
    }

    pub fn is_external_oes(&self) -> bool {
        match self.data {
            FrameData::OESTexture(_) => true,
            _ => false,
        }
    }
}

pub trait FrameRenderer: Send + 'static {
    fn render(&mut self, frame: Frame);
}
