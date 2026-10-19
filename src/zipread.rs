//! Lecture d'une archive zip (paquets de brosses Krita `.bundle`, brosses Procreate) : le
//! répertoire central à la fin du fichier, puis chaque fichier, rangé tel quel (méthode 0) ou
//! compressé (méthode 8, « deflate », par flate2). Pas de zip64 ni de chiffrement.

use std::io::Read;

/// Un fichier de l'archive.
#[derive(Clone, Debug)]
struct Entry {
    name: String,
    method: u16,
    compressed: usize,
    size: usize,
    local: usize,
}

/// Une archive zip ouverte (sur ses octets).
pub struct Zip<'a> {
    data: &'a [u8],
    entries: Vec<Entry>,
}

fn u16le(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(i..i + 2)?.try_into().ok()?))
}

fn u32le(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?))
}

impl<'a> Zip<'a> {
    pub fn open(data: &'a [u8]) -> Result<Zip<'a>, String> {
        // La fin du répertoire central : dans les derniers 64 Ko (après un commentaire éventuel).
        let lo = data.len().saturating_sub(22 + 65_535);
        let eocd = (lo..data.len().saturating_sub(21)).rev().find(|&i| u32le(data, i) == Some(0x0605_4b50)).ok_or("ce n'est pas une archive zip")?;
        let count = u16le(data, eocd + 10).ok_or("zip tronqué")? as usize;
        let mut p = u32le(data, eocd + 16).ok_or("zip tronqué")? as usize;
        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            if u32le(data, p) != Some(0x0201_4b50) {
                return Err("répertoire du zip illisible".into());
            }
            let method = u16le(data, p + 10).ok_or("zip tronqué")?;
            let compressed = u32le(data, p + 20).ok_or("zip tronqué")? as usize;
            let size = u32le(data, p + 24).ok_or("zip tronqué")? as usize;
            let (n, x, c) = (u16le(data, p + 28).ok_or("zip tronqué")? as usize, u16le(data, p + 30).ok_or("zip tronqué")? as usize, u16le(data, p + 32).ok_or("zip tronqué")? as usize);
            let local = u32le(data, p + 42).ok_or("zip tronqué")? as usize;
            let name = String::from_utf8_lossy(data.get(p + 46..p + 46 + n).ok_or("zip tronqué")?).into_owned();
            entries.push(Entry { name, method, compressed, size, local });
            p += 46 + n + x + c;
        }
        Ok(Zip { data, entries })
    }

    /// Les noms des fichiers (chemins dans l'archive, séparés par `/`).
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|e| e.name.as_str())
    }

    /// Le contenu du fichier `name` (aucun : absent ou illisible).
    pub fn read(&self, name: &str) -> Option<Vec<u8>> {
        let e = self.entries.iter().find(|e| e.name == name)?;
        if u32le(self.data, e.local) != Some(0x0403_4b50) {
            return None;
        }
        let n = u16le(self.data, e.local + 26)? as usize;
        let x = u16le(self.data, e.local + 28)? as usize;
        let start = e.local + 30 + n + x;
        let raw = self.data.get(start..start + e.compressed)?;
        match e.method {
            0 => Some(raw.to_vec()),
            8 => {
                // (Un fichier abîmé ou piégé qui annonce des gigaoctets n'épuise pas la mémoire.)
                const LIMIT: u64 = 256 * 1024 * 1024;
                let mut out = Vec::with_capacity(e.size.min(LIMIT as usize));
                flate2::read::DeflateDecoder::new(raw).take(LIMIT).read_to_end(&mut out).ok()?;
                Some(out)
            }
            _ => None,
        }
    }

    /// Le premier fichier dont le chemin finit par `/name` (ou vaut `name`).
    pub fn find(&self, name: &str) -> Option<Vec<u8>> {
        let suffix = format!("/{name}");
        let full = self.entries.iter().find(|e| e.name == name || e.name.ends_with(&suffix))?.name.clone();
        self.read(&full)
    }
}
