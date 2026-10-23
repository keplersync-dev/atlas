//! File hashes Atlas needs, read in a streaming way (resource packs can be large).
//!
//! - sha1: Modrinth lookups and the download cache key
//! - sha256: Atlas blob key and manifest hashing
//! - murmur2: CurseForge's fingerprint — MurmurHash2 (32-bit, seed 1) over the file
//!   with every tab, newline, carriage return and space byte removed.

use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::Path;

use sha1::{Digest, Sha1};
use sha2::Sha256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hashes {
    pub sha1: String,
    pub sha256: String,
    pub murmur2: u32,
}

const CHUNK: usize = 64 * 1024;
/// Files up to this size (nearly every mod) are read into memory once. Bigger ones are
/// streamed twice, since murmur2 needs its input's length before the first byte.
pub const IN_MEMORY: u64 = 64 * 1024 * 1024;

fn is_ignored(b: u8) -> bool {
    matches!(b, b'\t' | b'\n' | b'\r' | b' ')
}

/// `is_ignored` as a table, for the in-memory path's tight loop.
const KEEP: [bool; 256] = {
    let mut t = [true; 256];
    t[b'\t' as usize] = false;
    t[b'\n' as usize] = false;
    t[b'\r' as usize] = false;
    t[b' ' as usize] = false;
    t
};

/// Hash a file. Blocking: call it from a blocking thread.
pub fn hash_file(path: &Path) -> io::Result<Hashes> {
    hash_file_with(path, IN_MEMORY)
}

fn hash_file_with(path: &Path, in_memory: u64) -> io::Result<Hashes> {
    if std::fs::metadata(path)?.len() <= in_memory {
        return Ok(hash_bytes(&std::fs::read(path)?));
    }
    // Pass 1: sha1 + sha256, and how many bytes murmur2 will see (it needs the length first).
    let mut sha1 = Sha1::new();
    let mut sha256 = Sha256::new();
    let mut kept = 0u64;
    for_each_chunk(path, |chunk| {
        sha1.update(chunk);
        sha256.update(chunk);
        kept += chunk.iter().filter(|b| !is_ignored(**b)).count() as u64;
    })?;

    // Pass 2: murmur2 over the kept bytes.
    let mut murmur = Murmur2::new(kept as u32);
    for_each_chunk(path, |chunk| {
        for &b in chunk {
            if !is_ignored(b) {
                murmur.push(b);
            }
        }
    })?;

    Ok(Hashes {
        sha1: hex::encode(sha1.finalize()),
        sha256: hex::encode(sha256.finalize()),
        murmur2: murmur.finish(),
    })
}

/// All three hashes of a file read into memory.
pub fn hash_bytes(data: &[u8]) -> Hashes {
    // Copy every byte, but only move past the kept ones: no branch per byte.
    let mut kept = vec![0u8; data.len()];
    let mut n = 0;
    for &b in data {
        kept[n] = b;
        n += usize::from(KEEP[b as usize]);
    }
    kept.truncate(n);
    Hashes {
        sha1: hex::encode(Sha1::digest(data)),
        sha256: hex::encode(Sha256::digest(data)),
        murmur2: murmur2(&kept),
    }
}

/// MurmurHash2 (seed 1) over bytes already stripped of whitespace, four at a time.
fn murmur2(kept: &[u8]) -> u32 {
    let mut m = Murmur2::new(kept.len() as u32);
    let mut words = kept.chunks_exact(4);
    for w in &mut words {
        m.word(u32::from_le_bytes([w[0], w[1], w[2], w[3]]));
    }
    words.remainder().iter().for_each(|&b| m.push(b));
    m.finish()
}

fn for_each_chunk(path: &Path, mut f: impl FnMut(&[u8])) -> io::Result<()> {
    let mut reader = BufReader::with_capacity(CHUNK, File::open(path)?);
    let mut buf = vec![0u8; CHUNK];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            return Ok(());
        }
        f(&buf[..n]);
    }
}

/// MurmurHash2 fed one byte at a time, seed 1 (as CurseForge uses it).
struct Murmur2 {
    h: u32,
    block: [u8; 4],
    filled: usize,
}

const M: u32 = 0x5bd1_e995;

impl Murmur2 {
    fn new(len: u32) -> Self {
        Self {
            h: 1 ^ len,
            block: [0; 4],
            filled: 0,
        }
    }

    fn push(&mut self, b: u8) {
        self.block[self.filled] = b;
        self.filled += 1;
        if self.filled == 4 {
            self.word(u32::from_le_bytes(self.block));
            self.filled = 0;
        }
    }

    /// Mix in four bytes at once (only between whole blocks).
    fn word(&mut self, mut k: u32) {
        k = k.wrapping_mul(M);
        k ^= k >> 24;
        k = k.wrapping_mul(M);
        self.h = self.h.wrapping_mul(M) ^ k;
    }

    fn finish(mut self) -> u32 {
        let tail = &self.block[..self.filled];
        if !tail.is_empty() {
            if tail.len() >= 3 {
                self.h ^= (tail[2] as u32) << 16;
            }
            if tail.len() >= 2 {
                self.h ^= (tail[1] as u32) << 8;
            }
            self.h ^= tail[0] as u32;
            self.h = self.h.wrapping_mul(M);
        }
        let mut h = self.h;
        h ^= h >> 13;
        h = h.wrapping_mul(M);
        h ^= h >> 15;
        h
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn murmur(bytes: &[u8]) -> u32 {
        let kept: Vec<u8> = bytes.iter().copied().filter(|b| !is_ignored(*b)).collect();
        let mut m = Murmur2::new(kept.len() as u32);
        kept.iter().for_each(|b| m.push(*b));
        m.finish()
    }

    fn file_with(bytes: &[u8]) -> (std::path::PathBuf, Hashes) {
        let p = std::env::temp_dir().join(format!("atlas-hash-{}", crate::util::ids::new_id()));
        std::fs::write(&p, bytes).unwrap();
        let h = hash_file(&p).unwrap();
        std::fs::remove_file(&p).ok();
        (p, h)
    }

    #[test]
    fn sha_hashes_match_known_values() {
        let (_, h) = file_with(b"abc");
        assert_eq!(h.sha1, "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(
            h.sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn read_once_and_streamed_give_the_same_hashes() {
        // Whitespace spread through it, and every tail length for the word-at-a-time path.
        for len in [0, 1, 2, 3, 4, 5, 200_003] {
            let data: Vec<u8> = (0..len as u32)
                .map(|i| if i % 7 == 0 { b'\n' } else { (i % 251) as u8 })
                .collect();
            let p = std::env::temp_dir().join(format!("atlas-hash-{}", crate::util::ids::new_id()));
            std::fs::write(&p, &data).unwrap();
            let once = hash_file_with(&p, u64::MAX).unwrap();
            let streamed = hash_file_with(&p, 0).unwrap();
            std::fs::remove_file(&p).ok();
            assert_eq!(once, streamed, "{len} bytes");
            assert_eq!(once.murmur2, murmur(&data));
        }
    }

    #[test]
    fn murmur2_matches_reference_values() {
        // From an independent (Python) MurmurHash2, seed 1, whitespace removed.
        assert_eq!(murmur(b""), 0x5bd1_5e36);
        assert_eq!(murmur(b"abc"), 0x60a4_fcc1);
        assert_eq!(murmur(b"abcd"), 0xc93f_7a16);
        assert_eq!(murmur(b"abcdefg"), 0x0afa_6525);
        assert_eq!(murmur(b"hello world\n"), 0xa85c_bded);
        assert_eq!(
            murmur(b"abc"),
            murmur(b"a b\tc\r\n"),
            "whitespace is ignored"
        );
        assert_ne!(murmur(b"abc"), murmur(b"abd"));
        // Every tail length (0–3 leftover bytes) goes through a different branch.
        let lens: Vec<u32> = (4..8).map(|n| murmur(&b"abcdefg"[..n])).collect();
        let unique: std::collections::HashSet<_> = lens.iter().collect();
        assert_eq!(unique.len(), 4);
    }

    #[test]
    fn file_hash_matches_in_memory_murmur_across_chunks() {
        // Bigger than one chunk, with whitespace spread through it.
        let data: Vec<u8> = (0..200_000u32)
            .map(|i| if i % 7 == 0 { b' ' } else { (i % 251) as u8 })
            .collect();
        let (_, h) = file_with(&data);
        assert_eq!(h.murmur2, murmur(&data));
    }
}

#[cfg(test)]
mod real {
    /// Atlas's fingerprints against the ones CurseForge saved for the files in this PC's
    /// CurseForge instances (`installedAddons` in minecraftinstance.json). Reads only.
    /// `cargo test --release --lib real_fingerprints -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_fingerprints() {
        use crate::dto::LauncherKind;
        let (mut same, mut differ) = (0, 0);
        for root in crate::launchers::detect(LauncherKind::Curseforge) {
            let Ok(dirs) = std::fs::read_dir(root.join("Instances")) else {
                continue;
            };
            for dir in dirs.flatten().map(|d| d.path()) {
                let Ok(text) = std::fs::read_to_string(dir.join("minecraftinstance.json")) else {
                    continue;
                };
                let json: serde_json::Value = serde_json::from_str(&text).unwrap();
                for addon in json["installedAddons"].as_array().into_iter().flatten() {
                    let file = &addon["installedFile"];
                    let (Some(name), Some(expected)) = (
                        file["fileNameOnDisk"].as_str(),
                        // CurseForge sometimes records 0: it didn't work one out.
                        file["packageFingerprint"].as_u64().filter(|&f| f != 0),
                    ) else {
                        continue;
                    };
                    let Some(path) = ["mods", "resourcepacks", "shaderpacks"]
                        .iter()
                        .map(|d| dir.join(d).join(name))
                        .find(|p| p.is_file())
                    else {
                        continue;
                    };
                    let h = super::hash_file(&path).unwrap();
                    if u64::from(h.murmur2) == expected {
                        same += 1;
                    } else {
                        differ += 1;
                        println!("differs: {}", path.display());
                    }
                }
            }
        }
        println!("{same} fingerprints match CurseForge's, {differ} differ");
        assert_eq!(differ, 0);
    }
}
