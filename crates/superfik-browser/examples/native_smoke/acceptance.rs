use super::Error;
use serde_json::{Value, json};
use superfik_browser::Profile;

pub(super) fn check() -> Result<(), Error> {
    let temporary = tempfile::tempdir()?;
    let root = temporary.path().canonicalize()?;
    let exe = std::env::current_exe()?;
    let execute = |phase| -> Result<(), Error> {
        let status = std::process::Command::new(&exe)
            .arg(phase)
            .arg(&root)
            .args(["-ApplePersistenceIgnoreState", "YES", "-NSQuitAlwaysKeepsWindows", "NO"])
            .status()?;
        if !status.success() {
            return Err(format!("{phase} failed: {status}").into());
        }
        Ok(())
    };
    execute("seed")?;
    if std::env::var_os("SUPERFIK_EXTENDED_PROFILE_SMOKE").is_some() {
        use superfik_core::browser_profiles::{BrowserProfile, GpuIdentity, ProfileBook, Screen, WebRtc};
        let mut book = ProfileBook::default();
        for (name, cpu, memory) in [("a", 4, 4.), ("b", 2, 2.)] {
            let mut profile = BrowserProfile::new(name.into());
            profile.settings.enabled = true;
            profile.settings.hardware_concurrency = Some(cpu);
            profile.settings.device_memory = Some(memory);
            profile.settings.seed = if name == "a" { 1001 } else { 2002 };
            profile.settings.canvas = true;
            profile.settings.audio = true;
            profile.settings.fonts = vec!["Times New Roman".into()];
            profile.settings.webrtc = WebRtc::Disabled;
            profile.settings.screen = Some(if name == "a" {
                Screen { width: 1280, height: 720, scale: 1., depth: 24 }
            } else {
                Screen { width: 1920, height: 1080, scale: 2., depth: 24 }
            });
            profile.settings.gpu = Some(GpuIdentity {
                vendor: "Superfik Laboratory".into(),
                renderer: format!("Superfik Lab {name}"),
                ..GpuIdentity::default()
            });
            let id = profile.id;
            book.save(profile)?;
            book.assign(name.into(), Some(id))?;
        }
        book.assign("real".into(), None)?;
        std::fs::write(root.join("superfik-profiles.json"), serde_json::to_vec(&book)?)?;
        let real = Profile::Persistent("real".into()).dir(&root).ok_or("missing real profile")?;
        std::fs::write(
            real.join("superfik-identity.json"),
            json!({"schema":1,"enabled":true,"hardware_concurrency":1,"device_memory":1}).to_string(),
        )?;
    }
    for (name, cpu, memory) in [("a", 4, 4), ("b", 2, 2)] {
        let directory = Profile::Persistent(name.into()).dir(&root).ok_or("missing profile")?;
        std::fs::write(
            directory.join("superfik-identity.json"),
            json!({
                "schema":1,"enabled":true,"hardware_concurrency":cpu,"device_memory":memory
            })
            .to_string(),
        )?;
    }
    execute("check")?;
    if std::env::var_os("SUPERFIK_EXTENDED_PROFILE_SMOKE").is_some() {
        let first = std::fs::read(root.join("check.json"))?;
        execute("check")?;
        if serde_json::from_slice::<Value>(&first)?
            != serde_json::from_slice::<Value>(&std::fs::read(root.join("check.json"))?)?
        {
            return Err("Native profiles changed across process restarts".into());
        }
    }
    let baseline: Value = serde_json::from_slice(&std::fs::read(root.join("seed.json"))?)?;
    let actual: Value = serde_json::from_slice(&std::fs::read(root.join("check.json"))?)?;
    for name in ["a", "b", "off", "private", "real"] {
        for context in ["page", "frame", "worker"] {
            if baseline[name][context] != baseline["off"]["page"] {
                return Err(format!("inconsistent baseline: {name}/{context}").into());
            }
        }
    }
    for (name, cpu, memory) in [("a", 4, 4), ("b", 2, 2)] {
        for context in ["page", "frame", "worker"] {
            if actual[name][context] != json!({"cpu":cpu,"memory":memory}) {
                return Err(format!("{name}/{context}: unexpected values {}", actual[name][context]).into());
            }
        }
        if actual[name]["hints"].as_object().is_none_or(|hints| {
            hints.is_empty()
                || hints.values().any(|value| value.as_str().and_then(|v| v.parse::<u32>().ok()) != Some(memory))
        }) {
            return Err(format!("{name}: inconsistent client hints {}", actual[name]["hints"]).into());
        }
    }
    for name in ["off", "private", "real"] {
        if actual[name] != baseline[name] {
            return Err(format!("default identity changed: {name}").into());
        }
    }
    if std::env::var_os("SUPERFIK_EXTENDED_PROFILE_SMOKE").is_some() {
        for name in ["a", "b"] {
            if actual[name]["gpu"] != json!(format!("Superfik Lab {name}")) {
                return Err(format!("{name}: native GPU profile was not applied: {}", actual[name]["gpu"]).into());
            }
            for context in ["lab", "frameLab", "workerLab"] {
                if baseline[name][context]["menlo"] != json!(true) {
                    return Err(format!(
                        "Font acceptance fixture could not load its baseline local font: {name}/{context}"
                    )
                    .into());
                }
                let observed = &actual[name][context];
                if observed["gpu"] != json!(format!("Superfik Lab {name}")) || observed["menlo"] != json!(false) {
                    return Err(format!("{name}/{context}: inconsistent native GPU/font profile: {observed}").into());
                }
                if !observed["webgpu"].is_null() && observed["webgpu"]["vendor"] != json!("Superfik Laboratory") {
                    return Err(format!("{name}/{context}: WebGPU identity differs from WebGL").into());
                }
                for key in ["pixels", "encoded", "glPixels", "packPixels", "floatPixels"] {
                    if observed[key].is_null()
                        || observed[key] == baseline[name][context][key]
                        || observed[key] != actual[name]["lab"][key]
                    {
                        return Err(format!(
                            "{name}/{context}: missing/inconsistent canvas policy for {key}: {observed}"
                        )
                        .into());
                    }
                }
                if context != "workerLab" {
                    for key in ["audio", "dataUrl"] {
                        if observed[key].is_null() || observed[key] == baseline[name][context][key] {
                            return Err(format!("{name}/{context}: {key} not overridden").into());
                        }
                    }
                    if observed["audio"] != observed["audioCopy"] || observed["rtc"] != json!(false) {
                        return Err(format!("{name}/{context}: audio/WebRTC policy inconsistent: {observed}").into());
                    }
                    let display = if name == "a" { json!([1280, 720, 1, 24]) } else { json!([1920, 1080, 2, 24]) };
                    if observed["display"] != display {
                        return Err(format!("{name}/{context}: screen mismatch: {observed}").into());
                    }
                }
            }
        }
    }
    println!("Native identity acceptance passed: {actual}");
    Ok(())
}
