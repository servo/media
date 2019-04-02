use std::sync::Arc;

#[derive(Clone)]
pub enum FrameData {
    Raw(Arc<Vec<u8>>),
    Texture(u32),
}

pub trait Buffer: Send + Sync {
    fn to_vec(&self) -> Result<FrameData, ()>;
}

#[derive(Clone)]
pub struct Frame {
    width: i32,
    height: i32,
    data: FrameData,
    buffer: Arc<Buffer>,
}

impl Frame {
    pub fn new(width: i32, height: i32, buffer: Arc<Buffer>) -> Result<Self, ()> {
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
            FrameData::Texture(data) => data,
            _ => unreachable!("invalid texture id request for raw data frame"),
        }
    }
}

pub trait FrameRenderer: Send + 'static {
    fn render(&mut self, frame: Frame);
}
