//! A GPU buffer that grows to fit whatever is written into it.

#[derive(Debug)]
pub(crate) struct GrowableBuffer {
    buffer: wgpu::Buffer,
    usage: wgpu::BufferUsages,
    label: &'static str,
}

impl GrowableBuffer {
    pub fn new(device: &wgpu::Device, label: &'static str, usage: wgpu::BufferUsages) -> Self {
        let usage = usage | wgpu::BufferUsages::COPY_DST;
        Self { buffer: create(device, label, usage, 4096), usage, label }
    }

    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    pub fn write(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, data: &[u8]) -> bool {
        let needed = (data.len() as u64).max(16);
        let mut recreated = false;
        if needed > self.buffer.size() {
            self.buffer = create(device, self.label, self.usage, needed.next_power_of_two());
            recreated = true;
        }
        if !data.is_empty() {
            queue.write_buffer(&self.buffer, 0, data);
        }
        recreated
    }
}

fn create(device: &wgpu::Device, label: &str, usage: wgpu::BufferUsages, size: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage, mapped_at_creation: false })
}
