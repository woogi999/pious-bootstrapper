use std::path::{Path, PathBuf};

fn main() {
    embed_ffmpeg();
    tauri_build::build()
}

/// FFmpeg goes inside pious.exe, compressed, so recording works without a
/// download or a separate file (`core::recorder` unpacks it on first use).
/// It's taken from PIOUS_FFMPEG, else `installer\vendor\ffmpeg.exe` (the
/// release scripts put it there before building). Without one, the build
/// still works and Pious falls back to downloading FFmpeg.
fn embed_ffmpeg() {
    println!("cargo:rerun-if-env-changed=PIOUS_FFMPEG");
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let source = std::env::var_os("PIOUS_FFMPEG")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.parent().unwrap_or(&manifest).join("installer").join("vendor").join("ffmpeg.exe"));
    println!("cargo:rerun-if-changed={}", source.display());

    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let packed = out.join("ffmpeg.exe.zst");
    let digest = out.join("ffmpeg.sha256");
    if !source.is_file() {
        println!("cargo:warning=No FFmpeg at {} to build in; recording will download it.", source.display());
        std::fs::write(&packed, b"").unwrap();
        std::fs::write(&digest, "").unwrap();
        return;
    }
    let raw = std::fs::read(&source).expect("read FFmpeg");
    let hash = sha256_hex(&raw);
    // Already packed from this same file (OUT_DIR survives rebuilds).
    if std::fs::read_to_string(&digest).ok().as_deref() == Some(hash.as_str()) && packed.metadata().is_ok_and(|m| m.len() > 0) {
        return;
    }
    let mut encoder = zstd::stream::Encoder::new(Vec::new(), 19).expect("zstd");
    encoder.long_distance_matching(true).ok();
    encoder.window_log(27).ok();
    encoder.multithread(std::thread::available_parallelism().map_or(1, |n| n.get() as u32)).ok();
    std::io::copy(&mut raw.as_slice(), &mut encoder).expect("compress FFmpeg");
    let bytes = encoder.finish().expect("compress FFmpeg");
    write_atomic(&packed, &bytes);
    write_atomic(&digest, hash.as_bytes());
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

fn write_atomic(path: &Path, bytes: &[u8]) {
    let partial = path.with_extension("part");
    std::fs::write(&partial, bytes).unwrap();
    std::fs::rename(&partial, path).unwrap();
}
