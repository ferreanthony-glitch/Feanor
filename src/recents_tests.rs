//! Projets récents : en tête, sans doublon, dix au plus ; le nom montré.

use std::path::{Path, PathBuf};

use crate::recents::{label, remember, MAX};

#[test]
fn the_last_project_goes_first_without_duplicates() {
    let mut list: Vec<PathBuf> = Vec::new();
    remember(&mut list, Path::new(r"C:\Films\Bus\Bus.pelure"));
    remember(&mut list, Path::new(r"C:\Films\Car\Car.pelure"));
    remember(&mut list, Path::new(r"c:\films\bus\BUS.pelure"));
    assert_eq!(list.len(), 2, "le même projet, la casse ignorée");
    assert_eq!(list[0], PathBuf::from(r"c:\films\bus\BUS.pelure"));
    for i in 0..20 {
        remember(&mut list, &PathBuf::from(format!(r"C:\P{i}\P{i}.pelure")));
    }
    assert_eq!(list.len(), MAX);
    assert_eq!(list[0], PathBuf::from(r"C:\P19\P19.pelure"));
}

#[test]
fn a_recent_project_shows_its_name_and_place() {
    assert_eq!(label(Path::new(r"C:\Films\Bus\Bus.pelure")), r"Bus  —  C:\Films");
}