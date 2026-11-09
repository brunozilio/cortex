use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use superfik_browser::{Frame, PageEvent, Recorder};
use superfik_core::page::Profile;
use superfik_core::{OrgId, UserId};

use super::*;

#[derive(Default)]
struct Fake {
    recorder: Recorder,
    frame: Option<Frame>,
}

#[derive(Clone, Default)]
struct Shared(Rc<RefCell<Fake>>);

impl BrowserHost for Shared {
    fn send(&mut self, call: Call) {
        self.0.borrow_mut().recorder.send(call);
    }
    fn pump(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> Vec<(BrowserId, PageEvent)> {
        self.0.borrow_mut().recorder.pump(device, queue)
    }
    fn frame(&self, _id: BrowserId) -> Option<Frame> {
        self.0.borrow().frame.clone()
    }
    fn is_active(&self) -> bool {
        self.0.borrow().recorder.is_active()
    }
}

fn painted(gpu: &Gpu, w: u32, h: u32, rgba: [u8; 4], generation: u64) -> Frame {
    let size = wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 };
    let format = wgpu::TextureFormat::Bgra8Unorm;
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let bgra = [rgba[2], rgba[1], rgba[0], rgba[3]].repeat((w * h) as usize);
    gpu.queue.write_texture(
        texture.as_image_copy(),
        &bgra,
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 4), rows_per_image: None },
        size,
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    Frame { view, size: (w, h), format, generation }
}

fn outbox() -> (Outbox, crossbeam_channel::Receiver<ServerEvent>) {
    let (tx, rx) = crossbeam_channel::unbounded();
    (Outbox::new(tx, Arc::new(|| {})), rx)
}

fn open(id: BrowserId) -> PageCall {
    PageCall::Open {
        id,
        url: "https://a.test".into(),
        profile: Profile::Persistent("c".into()),
        size: (64, 32),
        scale: 1.0,
    }
}

#[test]
fn a_page_streams_its_frames_and_reports_to_who_opened_it_only() {
    let Some(gpu) = Gpu::headless() else { return };
    let host = Shared::default();
    let (service, mut pages) = pages(Box::new(host.clone()), Some(gpu));
    let gpu = pages.gpu.as_ref().expect("a GPU");
    host.0.borrow_mut().frame = Some(painted(gpu, 64, 32, [200, 30, 10, 255], 1));
    let ana = Principal::member(UserId::new(), OrgId::new());
    let bia = Principal::member(UserId::new(), OrgId::new());
    let (to_ana, ana_hears) = outbox();
    let (to_bia, bia_hears) = outbox();
    let id = BrowserId::new();
    let t0 = Instant::now();

    service.call(&ana, open(id), to_ana.clone());
    service.call(&bia, PageCall::Navigate(id, "https://evil.test".into()), to_bia);
    pages.step(t0);
    let calls = host.0.borrow().recorder.calls.clone();
    assert!(
        matches!(&calls[..], [Call::Open { profile: Profile::Persistent(name), .. }] if name.ends_with("-c") && name != "c"),
        "{calls:?}"
    );
    let first: Vec<ServerEvent> = ana_hears.try_iter().collect();
    let [ServerEvent::PageFrame(frame)] = &first[..] else { panic!("{first:?}") };
    assert!(frame.whole && frame.size == (64, 32));
    let pixel = superfik_frames::decode(&frame.tiles[0]).expect("decodes").rgba;
    assert!(pixel[0] > 180 && pixel[2] < 40, "red, in RGBA order: {:?}", &pixel[..4]);
    assert!(bia_hears.try_iter().next().is_none());

    host.0.borrow_mut().recorder.events.push((id, PageEvent::Title("A".into())));
    pages.step(t0 + FRAME_GAP);
    let heard: Vec<ServerEvent> = ana_hears.try_iter().collect();
    assert!(matches!(&heard[..], [ServerEvent::Page { report: PageReport::Title(t), .. }] if t == "A"), "{heard:?}");

    service.call(&ana, PageCall::Refresh(id), to_ana.clone());
    pages.step(t0 + FRAME_GAP * 2);
    assert!(matches!(ana_hears.try_iter().next(), Some(ServerEvent::PageFrame(f)) if f.whole));

    service.call(&ana, PageCall::Close(id), to_ana);
    pages.step(t0 + FRAME_GAP * 3);
    assert!(!pages.is_active());
}

#[test]
fn without_a_gpu_pages_fail_to_open() {
    let (service, mut pages) = pages(Box::new(Recorder::default()), None);
    let (to_ana, ana_hears) = outbox();
    let id = BrowserId::new();
    service.call(&Principal::LocalOwner, open(id), to_ana);
    pages.step(Instant::now());
    assert!(matches!(ana_hears.try_iter().next(), Some(ServerEvent::Page { report: PageReport::Failed { .. }, .. })));
}
