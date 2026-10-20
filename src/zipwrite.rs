//! Écriture d'une archive zip (la fiche technique en `.xlsx`, `ficheformats.rs`) : le contraire de
//! `zipread.rs`. Pas de zip64 ni de chiffrement ; chaque fichier est rangé tel quel (méthode 0, pour
//! les images PNG déjà compressées) ou compressé (méthode 8, « deflate », par flate2, qui donne aussi
//! le CRC-32). Les noms sont en UTF-8 (drapeau 11), la date de chaque fichier est fixe
//! (1er janvier 1980) : deux écritures du même contenu donnent le même fichier, octet pour octet.

use std::io::Write;

use flate2::write::DeflateEncoder;
use flate2::{Compression, Crc};

/// Ce que l'archive ne sait pas contenir (zip64 n'est pas écrit).
const LIMITE: usize = u32::MAX as usize;

/// Une archive en cours d'écriture, en mémoire.
pub struct ZipWriter {
    out: Vec<u8>,
    /// Pour le répertoire central : nom, méthode, CRC, tailles, position de l'en-tête local.
    entries: Vec<Entry>,
}

struct Entry {
    name: String,
    method: u16,
    crc: u32,
    compressed: u32,
    size: u32,
    local: u32,
}

/// Date et heure DOS du 1er janvier 1980, minuit.
const DOS_TIME: u16 = 0;
const DOS_DATE: u16 = (1 << 5) | 1;

impl Default for ZipWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl ZipWriter {
    pub fn new() -> ZipWriter {
        ZipWriter { out: Vec::new(), entries: Vec::new() }
    }

    /// Ajoute un fichier rangé tel quel.
    pub fn store(&mut self, name: &str, data: &[u8]) -> Result<(), String> {
        self.add(name, data, false)
    }

    /// Ajoute un fichier compressé (garde la version rangée telle quelle si la compression ne gagne rien).
    pub fn deflate(&mut self, name: &str, data: &[u8]) -> Result<(), String> {
        self.add(name, data, true)
    }

    fn add(&mut self, name: &str, data: &[u8], compress: bool) -> Result<(), String> {
        if name.is_empty() || name.len() > u16::MAX as usize || name.starts_with('/') || name.contains('\\') || name.contains("..") {
            return Err(format!("nom de fichier d'archive refusé : « {name} »"));
        }
        if data.len() > LIMITE || self.out.len() > LIMITE.saturating_sub(data.len()).saturating_sub(1024) {
            return Err("l'archive dépasse 4 Go (zip64 n'est pas écrit)".into());
        }
        let mut crc = Crc::new();
        crc.update(data);
        let (method, body): (u16, std::borrow::Cow<[u8]>) = if compress && !data.is_empty() {
            let mut enc = DeflateEncoder::new(Vec::new(), Compression::new(6));
            enc.write_all(data).map_err(|e| e.to_string())?;
            let packed = enc.finish().map_err(|e| e.to_string())?;
            if packed.len() < data.len() {
                (8, packed.into())
            } else {
                (0, data.into())
            }
        } else {
            (0, data.into())
        };
        let local = self.out.len() as u32;
        let o = &mut self.out;
        o.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        o.extend_from_slice(&20u16.to_le_bytes()); // version nécessaire
        o.extend_from_slice(&0x0800u16.to_le_bytes()); // noms en UTF-8
        o.extend_from_slice(&method.to_le_bytes());
        o.extend_from_slice(&DOS_TIME.to_le_bytes());
        o.extend_from_slice(&DOS_DATE.to_le_bytes());
        o.extend_from_slice(&crc.sum().to_le_bytes());
        o.extend_from_slice(&(body.len() as u32).to_le_bytes());
        o.extend_from_slice(&(data.len() as u32).to_le_bytes());
        o.extend_from_slice(&(name.len() as u16).to_le_bytes());
        o.extend_from_slice(&0u16.to_le_bytes()); // pas de champ supplémentaire
        o.extend_from_slice(name.as_bytes());
        o.extend_from_slice(&body);
        self.entries.push(Entry { name: name.to_string(), method, crc: crc.sum(), compressed: body.len() as u32, size: data.len() as u32, local });
        Ok(())
    }

    /// Écrit le répertoire central et rend l'archive.
    pub fn finish(mut self) -> Result<Vec<u8>, String> {
        if self.entries.len() > u16::MAX as usize {
            return Err("trop de fichiers pour une archive zip".into());
        }
        let start = self.out.len();
        if start > LIMITE {
            return Err("l'archive dépasse 4 Go (zip64 n'est pas écrit)".into());
        }
        for e in &self.entries {
            let o = &mut self.out;
            o.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
            o.extend_from_slice(&20u16.to_le_bytes()); // version faite par
            o.extend_from_slice(&20u16.to_le_bytes()); // version nécessaire
            o.extend_from_slice(&0x0800u16.to_le_bytes());
            o.extend_from_slice(&e.method.to_le_bytes());
            o.extend_from_slice(&DOS_TIME.to_le_bytes());
            o.extend_from_slice(&DOS_DATE.to_le_bytes());
            o.extend_from_slice(&e.crc.to_le_bytes());
            o.extend_from_slice(&e.compressed.to_le_bytes());
            o.extend_from_slice(&e.size.to_le_bytes());
            o.extend_from_slice(&(e.name.len() as u16).to_le_bytes());
            o.extend_from_slice(&0u16.to_le_bytes()); // champ supplémentaire
            o.extend_from_slice(&0u16.to_le_bytes()); // commentaire
            o.extend_from_slice(&0u16.to_le_bytes()); // disque
            o.extend_from_slice(&0u16.to_le_bytes()); // attributs internes
            o.extend_from_slice(&0u32.to_le_bytes()); // attributs externes
            o.extend_from_slice(&e.local.to_le_bytes());
            o.extend_from_slice(e.name.as_bytes());
        }
        let size = self.out.len() - start;
        let o = &mut self.out;
        o.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        o.extend_from_slice(&0u16.to_le_bytes());
        o.extend_from_slice(&0u16.to_le_bytes());
        o.extend_from_slice(&(self.entries.len() as u16).to_le_bytes());
        o.extend_from_slice(&(self.entries.len() as u16).to_le_bytes());
        o.extend_from_slice(&(size as u32).to_le_bytes());
        o.extend_from_slice(&(start as u32).to_le_bytes());
        o.extend_from_slice(&0u16.to_le_bytes());
        Ok(self.out)
    }
}
