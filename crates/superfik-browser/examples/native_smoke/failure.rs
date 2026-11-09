//! A failed host must finish page loads and agent requests, never leave them waiting.

use super::*;

pub fn check() -> Result<(), Error> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default()))?;
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default()))?;
    let root = tempfile::tempdir()?;
    let mut other = std::process::Command::new("/bin/sleep").arg("60").spawn()?;
    std::os::unix::fs::symlink(format!("fixture-{}", other.id()), root.path().join("SingletonLock"))?;
    let mut host = superfik_browser::system(root.path().into());
    other.kill()?;
    other.wait()?;
    check_host(host.as_mut(), &device, &queue, "profile owned by another process")?;

    let file = root.path().join("not-a-directory");
    std::fs::write(&file, b"fixture")?;
    let mut host = superfik_browser::cef::CefHost::new(file);
    host.warm_up();
    check_host(&mut host, &device, &queue, "engine startup failure")?;
    eprintln!("Native browser failure acceptance passed");
    Ok(())
}

fn check_host(host: &mut dyn BrowserHost, device: &wgpu::Device, queue: &wgpu::Queue, why: &str) -> Result<(), Error> {
    let id = BrowserId::new();
    let url = "https://fixture.invalid/".to_owned();
    let request = BrowseId("unavailable-host".into());
    host.send(Call::Open { id, url: url.clone(), profile: Profile::Ephemeral, size: (32, 32), scale: 1.0 });
    host.send(Call::Agent { id, request: request.clone(), op: BrowserOp::Evaluate { script: "1".into() } });
    let events = host.pump(device, queue);
    if !events.iter().any(|(page, event)| {
        *page == id && matches!(event, PageEvent::Failed { url: failed, error } if failed == &url && !error.is_empty())
    }) || !events.contains(&(id, PageEvent::Loading(false)))
        || !events.iter().any(|(page, event)| {
            *page == id && matches!(event, PageEvent::Answered { request: answered, answer: Err(error) } if answered == &request && !error.is_empty())
        })
    {
        return Err(format!("{why}: did not finish page and agent calls: {events:?}").into());
    }
    if host.is_active() || !host.pump(device, queue).is_empty() {
        return Err(format!("{why}: failed host kept pumping or repeated answers").into());
    }
    Ok(())
}
