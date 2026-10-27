//! Smoke test: play each sound effect once and the first music track for a few seconds.
fn main() {
    let mut a = rr_audio::Audio::new().expect("audio device");
    for k in ["chop", "treefall", "hammer", "pick", "sword", "fanfare"] {
        a.play(k, 0.8);
        std::thread::sleep(std::time::Duration::from_millis(400));
    }
    println!("sfx ok; assets at {:?}", rr_audio::assets_dir());
}
