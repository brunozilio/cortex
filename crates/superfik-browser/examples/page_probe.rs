//! Opens one page in CEF on Linux, without a window, and saves its first
//! settled frame as a PNG: the engine, the frame's import into wgpu and
//! the page's reports, end to end.
//!
//! `cargo run -p superfik-browser --example page_probe -- <url> <out.png>`
//! (CEF's files sit beside the example's binary after the build).

#[cfg(target_os = "linux")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::time::{Duration, Instant};

    use superfik_browser::{BrowserHost, BrowserId, Call, PageEvent, Profile};

    if let Some(code) = superfik_browser::helper::run_if_helper() {
        std::process::exit(code);
    }
    let mut args = std::env::args().skip(1);
    let url = args.next().unwrap_or_else(|| "data:text/html,<h1 style='color:teal'>Superfik on Linux</h1>".into());
    let out = args.next().unwrap_or_else(|| "page.png".into());

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))?;
    println!("adapter: {:?}", adapter.get_info());
    let descriptor = wgpu::DeviceDescriptor { label: Some("page probe"), ..Default::default() };
    let (device, queue) = match superfik_browser::open_device(&adapter, &descriptor) {
        Some(opened) => {
            println!("device: DMABUF capable");
            opened
        }
        None => pollster::block_on(adapter.request_device(&descriptor))?,
    };

    let root = tempfile::tempdir()?;
    let mut host = superfik_browser::cef::CefHost::new(root.path().to_path_buf());
    host.warm_up();
    let id = BrowserId::new();
    host.send(Call::Open { id, url, profile: Profile::Ephemeral, size: (800, 600), scale: 1.0 });

    let started = Instant::now();
    let mut loaded = false;
    let mut settled: Option<(Instant, u64)> = None;
    while started.elapsed() < Duration::from_secs(60) {
        for (_, event) in host.pump(&device, &queue) {
            match event {
                PageEvent::Loading(false) => loaded = true,
                PageEvent::Failed { url, error } => return Err(format!("{url}: {error}").into()),
                other => println!("event: {other:?}"),
            }
        }
        if let Some(frame) = host.frame(id) {
            match settled {
                Some((since, generation)) if generation == frame.generation => {
                    if loaded && since.elapsed() > Duration::from_millis(800) {
                        save(&device, &queue, &frame, &out)?;
                        println!("frame {}x{} {:?} → {out}", frame.size.0, frame.size.1, frame.format);
                        return Ok(());
                    }
                }
                _ => settled = Some((Instant::now(), frame.generation)),
            }
        }
        std::thread::sleep(Duration::from_millis(4));
    }
    Err(format!("no settled frame in a minute (loaded: {loaded})").into())
}

#[cfg(target_os = "linux")]
fn save(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    frame: &superfik_browser::Frame,
    out: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let (width, height) = frame.size;
    let row = (width * 4).div_ceil(256) * 256;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("page probe readback"),
        size: u64::from(row * height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.copy_texture_to_buffer(
        frame.view.texture().as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(height) },
        },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );
    queue.submit([encoder.finish()]);
    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::PollType::wait_indefinitely())?;
    let data = slice.get_mapped_range().map_err(|error| format!("{error:?}"))?;
    let bgra = matches!(frame.format, wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb);
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        let start = (y * row) as usize;
        for px in data[start..start + (width * 4) as usize].as_chunks::<4>().0 {
            if bgra {
                pixels.extend_from_slice(&[px[2], px[1], px[0], 255]);
            } else {
                pixels.extend_from_slice(&[px[0], px[1], px[2], 255]);
            }
        }
    }
    image::save_buffer(out, &pixels, width, height, image::ColorType::Rgba8)?;
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("page_probe runs on Linux; on macOS see the native_smoke example");
}
