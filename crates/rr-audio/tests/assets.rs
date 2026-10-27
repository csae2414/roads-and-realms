#[test]
fn all_audio_assets_exist_and_are_ogg() {
    let dir = rr_audio::assets_dir().join("audio");
    let prov = std::fs::read_to_string(rr_audio::assets_dir().join("PROVENANCE.md")).unwrap();
    let mut n = 0;
    for sub in ["music", "sfx", "ambient"] {
        for e in std::fs::read_dir(dir.join(sub)).unwrap() {
            let p = e.unwrap().path();
            let bytes = std::fs::read(&p).unwrap();
            assert_eq!(&bytes[..4], b"OggS", "{} is not Ogg", p.display());
            // every file has its source: by name, or as a numbered group like `hammer0-2.ogg`
            let stem = p.file_stem().unwrap().to_string_lossy().to_string();
            let base = stem.trim_end_matches(|c: char| c.is_ascii_digit());
            assert!(
                prov.contains(&format!("`{stem}.ogg`")) || prov.contains(&format!("`{base}")),
                "{stem} not in PROVENANCE.md"
            );
            n += 1;
        }
    }
    assert!(n >= 30, "{n}");
}
