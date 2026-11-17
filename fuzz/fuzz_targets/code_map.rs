#![no_main]

use libfuzzer_sys::fuzz_target;

const PATHS: [&str; 10] = [
    "src/a.rs",
    "src/a.ts",
    "src/a.tsx",
    "src/a.js",
    "src/a.mjs",
    "src/a.py",
    "src/a.pyi",
    "src/a.go",
    "a.cts",
    "a.jsx",
];

fuzz_target!(|data: &[u8]| {
    let Some((&pick, source)) = data.split_first() else { return };
    let path = PATHS[usize::from(pick) % PATHS.len()];
    let Ok(source) = std::str::from_utf8(source) else { return };
    assert!(code_map::extract::supported(path));
    let Some(found) = code_map::extract(path, source) else { return };
    assert_eq!(found.path, path);
    let lines = u32::try_from(source.split('\n').count()).unwrap_or(u32::MAX);
    for symbol in &found.symbols {
        assert!(1 <= symbol.line && symbol.line <= symbol.end_line, "{symbol:?}");
        assert!(symbol.end_line <= lines, "{symbol:?} ends past the last of {lines} lines");
    }
    for call in &found.calls {
        assert!(1 <= call.line && call.line <= lines, "{call:?} past the last of {lines} lines");
    }
});
