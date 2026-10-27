//! The build id of network games includes a hash of everything that decides how the game plays
//! (LAN round 3, #2: two builds with different rules reported the same id when nobody raised a
//! version number, and drifted apart in a game).
use std::path::{Path, PathBuf};

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs" || x == "toml") {
            out.push(p);
        }
    }
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut list = Vec::new();
    for d in [
        "crates/rr-core/src",
        "crates/rr-rules/src",
        "crates/rr-world/src",
        "crates/rr-sim/src",
        "crates/rr-ai/src",
        // (and the network's own messages: QA round 7, #4, a new message slipped past)
        "crates/rr-net/src",
        "data/rules",
    ] {
        let dir = root.join(d);
        println!("cargo:rerun-if-changed={}", dir.display());
        files(&dir, &mut list);
    }
    // the app sets up and arms the computer players of a network game (levels, truce, peace,
    // invader): its setup files count too (net round 5, #5)
    for f in ["crates/rr-app/src/app.rs", "crates/rr-app/src/main.rs", "crates/rr-app/src/netui.rs"] {
        let p = root.join(f);
        println!("cargo:rerun-if-changed={}", p.display());
        list.push(p);
    }
    let lock = root.join("Cargo.lock");
    println!("cargo:rerun-if-changed={}", lock.display());
    list.push(lock);
    list.sort();
    // FNV-1a over the paths (relative) and the contents
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |bytes: &[u8]| {
        for b in bytes {
            h = (h ^ *b as u64).wrapping_mul(0x100_0000_01b3);
        }
    };
    for p in &list {
        eat(p.strip_prefix(&root).unwrap_or(p).to_string_lossy().as_bytes());
        eat(&std::fs::read(p).unwrap_or_default());
    }
    println!("cargo:rustc-env=RR_SOURCE_HASH={h:016x}");
}
