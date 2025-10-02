use dbus::arg::{RefArg, Variant};
use std::collections::HashMap;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Metadata {
    pub length: Option<i64>,
    pub art_url: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<Vec<String>>,
    pub artist: Option<Vec<String>>,
    pub composer: Option<Vec<String>>,
    pub disc_number: Option<i32>,
    pub genre: Option<Vec<String>>,
    pub title: Option<String>,
    pub track_number: Option<i32>,
    pub url: Option<String>,
}

impl Metadata {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn to_hashmap(&self) -> HashMap<String, Variant<Box<dyn RefArg + 'static>>> {
        let mut metadata = HashMap::new();

        if self.length.is_some() {
            let x = Box::new(self.length.unwrap().to_string()) as Box<dyn RefArg>;
            metadata.insert("mpris:length".to_string(), Variant(x));
        }

        if self.art_url.is_some() {
            let x = Box::new(self.art_url.clone().unwrap()) as Box<dyn RefArg>;
            metadata.insert("mpris:artUrl".to_string(), Variant(x));
        }

        if self.album.is_some() {
            let x = Box::new(self.album.clone().unwrap()) as Box<dyn RefArg>;
            metadata.insert("xesam:album".to_string(), Variant(x));
        }

        if self.album_artist.is_some() {
            let x = Box::new(self.album_artist.clone().unwrap()) as Box<dyn RefArg>;
            metadata.insert("xesam:albumArtist".to_string(), Variant(x));
        }

        if self.artist.is_some() {
            let x = Box::new(self.artist.clone().unwrap()) as Box<dyn RefArg>;
            metadata.insert("xesam:artist".to_string(), Variant(x));
        }

        if self.composer.is_some() {
            let x = Box::new(self.composer.clone().unwrap()) as Box<dyn RefArg>;
            metadata.insert("xesam:composer".to_string(), Variant(x));
        }

        if self.disc_number.is_some() {
            let x = Box::new(self.disc_number.unwrap()) as Box<dyn RefArg>;
            metadata.insert("xesam:discNumber".to_string(), Variant(x));
        }

        if self.genre.is_some() {
            let x = Box::new(self.clone().genre.unwrap()) as Box<dyn RefArg>;
            metadata.insert("xesam:genre".to_string(), Variant(x));
        }

        if self.title.is_some() {
            let x = Box::new(self.clone().title.unwrap()) as Box<dyn RefArg>;
            metadata.insert("xesam:title".to_string(), Variant(x));
        }

        if self.track_number.is_some() {
            let x = Box::new(self.track_number.unwrap()) as Box<dyn RefArg>;
            metadata.insert("xesam:trackNumber".to_string(), Variant(x));
        }

        if self.url.is_some() {
            let x = Box::new(self.url.clone().unwrap()) as Box<dyn RefArg>;
            metadata.insert("xesam:url".to_string(), Variant(x));
        }

        metadata
    }
}
