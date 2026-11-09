//! Window surface and device setup.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum GpuError {
    #[error("could not create a GPU surface: {0}")]
    Surface(#[from] wgpu::CreateSurfaceError),
    #[error("no compatible GPU adapter: {0}")]
    Adapter(#[from] wgpu::RequestAdapterError),
    #[error("could not open the GPU device: {0}")]
    Device(#[from] wgpu::RequestDeviceError),
    #[error("the surface does not support any usable format")]
    Format,
}

pub type OpenDevice = fn(&wgpu::Adapter, &wgpu::DeviceDescriptor<'_>) -> Option<(wgpu::Device, wgpu::Queue)>;

#[derive(Debug)]
pub struct GpuContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: Option<wgpu::Surface<'static>>,
    pub config: wgpu::SurfaceConfiguration,
}

impl GpuContext {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new<W>(window: W, width: u32, height: u32) -> Result<Self, GpuError>
    where
        W: Into<wgpu::SurfaceTarget<'static>> + wgpu::wgt::WgpuHasDisplayHandle + Clone,
    {
        Self::opening(window, width, height, None)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn opening<W>(window: W, width: u32, height: u32, open: Option<OpenDevice>) -> Result<Self, GpuError>
    where
        W: Into<wgpu::SurfaceTarget<'static>> + wgpu::wgt::WgpuHasDisplayHandle + Clone,
    {
        let descriptor = wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(window.clone()));
        pollster_block(Self::with_instance(descriptor, window, width, height, open))
    }

    pub async fn new_async(
        window: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
    ) -> Result<Self, GpuError> {
        Self::with_instance(
            wgpu::InstanceDescriptor::new_without_display_handle_from_env(),
            window,
            width,
            height,
            None,
        )
        .await
    }

    async fn with_instance(
        descriptor: wgpu::InstanceDescriptor,
        window: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
        open: Option<OpenDevice>,
    ) -> Result<Self, GpuError> {
        let instance = wgpu::Instance::new(descriptor);
        let surface = instance.create_surface(window)?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            })
            .await?;
        let descriptor = wgpu::DeviceDescriptor { label: Some("superfik"), ..Default::default() };
        let (device, queue) = match open.and_then(|open| open(&adapter, &descriptor)) {
            Some(opened) => opened,
            None => adapter.request_device(&descriptor).await?,
        };
        let mut config = surface.get_default_config(&adapter, width.max(1), height.max(1)).ok_or(GpuError::Format)?;
        let caps = surface.get_capabilities(&adapter);
        config.format = caps.formats.iter().copied().find(|f| !f.is_srgb()).ok_or(GpuError::Format)?;
        config.present_mode = wgpu::PresentMode::AutoVsync;
        config.desired_maximum_frame_latency = 1;
        surface.configure(&device, &config);
        Ok(Self { device, queue, surface: Some(surface), config })
    }

    pub fn headless(width: u32, height: u32) -> Result<Self, GpuError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster_block(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))?;
        let (device, queue) = pollster_block(
            adapter.request_device(&wgpu::DeviceDescriptor { label: Some("superfik headless"), ..Default::default() }),
        )?;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Rgba8Unorm,
            color_space: Default::default(),
            width: width.max(1),
            height: height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 1,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: Vec::new(),
        };
        Ok(Self { device, queue, surface: None, config })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        if let Some(surface) = &self.surface {
            surface.configure(&self.device, &self.config);
        }
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.config.format
    }
}

fn pollster_block<F: std::future::Future>(future: F) -> F::Output {
    pollster::block_on(future)
}
