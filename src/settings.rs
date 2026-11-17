//! Ayarlar: %APPDATA%\YtIndirici\ayarlar.json içinde tek dosya.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::i18n::Dil;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Bicim {
    Mp4,
    Mp3,
}

/// Video kalite seçenekleri: (etiket, en fazla yükseklik; None = en iyi).
/// İlk öğenin etiketi arayüzde seçili dile göre çevrilir.
pub const VIDEO_KALITE: &[(&str, Option<u32>)] = &[
    ("En iyi", None),
    ("2160p (4K)", Some(2160)),
    ("1440p", Some(1440)),
    ("1080p", Some(1080)),
    ("720p", Some(720)),
    ("480p", Some(480)),
    ("360p", Some(360)),
];

/// MP3 kalite seçenekleri: (etiket, yt-dlp --audio-quality değeri)
pub const SES_KALITE: &[(&str, &str)] = &[
    ("320 kbps", "320K"),
    ("256 kbps", "256K"),
    ("192 kbps", "192K"),
    ("128 kbps", "128K"),
];

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct Ayarlar {
    pub cikti_klasoru: PathBuf,
    pub bicim: Bicim,
    pub video_kalite: usize,
    pub ses_kalite: usize,
    pub tum_liste: bool,
    /// None: Windows arayüz dili kullanılır.
    pub dil: Option<Dil>,
}

impl Default for Ayarlar {
    fn default() -> Self {
        Self {
            cikti_klasoru: varsayilan_klasor(),
            bicim: Bicim::Mp4,
            video_kalite: 3, // 1080p
            ses_kalite: 0,   // 320 kbps
            tum_liste: false,
            dil: None,
        }
    }
}

fn varsayilan_klasor() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .map(|p| PathBuf::from(p).join("Downloads"))
        .unwrap_or_else(|| PathBuf::from(r"C:\Users\Windows\Downloads"))
}

fn dosya_yolu() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|p| PathBuf::from(p).join("YtIndirici").join("ayarlar.json"))
}

impl Ayarlar {
    pub fn yukle() -> Self {
        let mut a: Ayarlar = dosya_yolu()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        a.video_kalite = a.video_kalite.min(VIDEO_KALITE.len() - 1);
        a.ses_kalite = a.ses_kalite.min(SES_KALITE.len() - 1);
        if a.cikti_klasoru.as_os_str().is_empty() {
            a.cikti_klasoru = varsayilan_klasor();
        }
        a
    }

    /// Geçici dosyaya yazıp yerine taşır; yarım yazılmış ayar dosyası kalmaz.
    pub fn kaydet(&self) {
        let Some(yol) = dosya_yolu() else { return };
        if let Some(d) = yol.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let Ok(json) = serde_json::to_vec_pretty(self) else { return };
        let gecici = yol.with_extension("json.tmp");
        if std::fs::write(&gecici, json).is_ok() {
            let _ = std::fs::rename(&gecici, &yol);
        }
    }
}
