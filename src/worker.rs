//! yt-dlp alt süreçleri: indirme ve oynatma listesi çözümleme.
//! Her iş kendi iş parçacığında çalışır; arayüze yalnızca kanal üzerinden mesaj gönderir.

use std::io::Read;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{self, Sender};
use std::time::{Duration, Instant};

use crate::settings::Bicim;
use crate::win::Is;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const ETIKET: &str = "[YTI]";

/// Çalışma ortamı: bulunan araçların yolları.
pub struct Ortam {
    pub ytdlp: Option<PathBuf>,
    pub ffmpeg: Option<PathBuf>,
    pub node: Option<PathBuf>,
}

impl Ortam {
    pub fn bul() -> Ortam {
        let exe_dizini = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf));
        let ytdlp = exe_dizini
            .map(|d| d.join("yt-dlp.exe"))
            .filter(|p| p.is_file())
            .or_else(|| path_ara("yt-dlp.exe"));
        Ortam { ytdlp, ffmpeg: path_ara("ffmpeg.exe"), node: path_ara("node.exe") }
    }
}

fn path_ara(ad: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|d| d.join(ad)).find(|p| p.is_file())
}

/// Bir indirmenin o anki aşaması; metni arayüz seçili dile göre üretir.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Asama {
    Baslatiliyor,
    BilgiAliniyor,
    Indiriliyor,
    Birlestiriliyor,
    Mp3,
    Mp4,
    Tasiniyor,
    Isleniyor,
    IptalEdiliyor,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Hata {
    Baslatilamadi(String),
    Kod(i32),
    Beklenmedik,
    ListeBos,
    /// yt-dlp'nin kendi "ERROR:" metni (çevrilmez)
    YtDlp(String),
}

pub enum Olay {
    Baslik(String),
    Ilerleme { oran: f32, hiz: Option<f64>, kalan: Option<u64> },
    Asama(Asama),
    Dosya(PathBuf),
    Bitti(Result<(), Hata>),
    Iptal,
    Liste(Vec<(String, String)>),
}

pub struct Mesaj {
    pub id: u64,
    pub olay: Olay,
}

#[derive(Clone)]
pub struct Istek {
    pub url: String,
    pub bicim: Bicim,
    pub video_yukseklik: Option<u32>,
    pub ses_kalite: &'static str,
    pub cikti: PathBuf,
}

struct Gonderici {
    id: u64,
    tx: Sender<Mesaj>,
    ctx: eframe::egui::Context,
}

impl Gonderici {
    fn gonder(&self, olay: Olay) {
        let _ = self.tx.send(Mesaj { id: self.id, olay });
        self.ctx.request_repaint();
    }
}

fn temel_komut(ortam: &Ortam) -> Command {
    let mut c = Command::new(ortam.ytdlp.as_ref().expect("yt-dlp yolu"));
    c.creation_flags(CREATE_NO_WINDOW)
        .env("PYTHONIOENCODING", "utf-8")
        .env("PYTHONUTF8", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .args(["--color", "never", "--newline"]);
    if let Some(node) = &ortam.node {
        // YouTube artık bir JavaScript çalışma ortamı istiyor; varsayılan (deno) kurulu değil.
        c.arg("--js-runtimes").arg(format!("node:{}", node.display()));
    }
    c
}

/// Süreci başlatır, stdout ve stderr'i satır satır `satir` geri çağrısına verir,
/// çıkış kodunu döndürür.
fn calistir(mut komut: Command, is: &Is, mut satir: impl FnMut(&str)) -> Result<Option<i32>, Hata> {
    if is.iptal_edildi() {
        return Ok(None);
    }
    let mut cocuk = komut.spawn().map_err(|e| Hata::Baslatilamadi(e.to_string()))?;
    is.ata(&cocuk);

    let (tx, rx) = mpsc::channel::<String>();
    let mut okuyucular = Vec::new();
    if let Some(o) = cocuk.stdout.take() {
        let tx = tx.clone();
        okuyucular.push(std::thread::spawn(move || satir_oku(o, tx)));
    }
    if let Some(o) = cocuk.stderr.take() {
        let tx = tx.clone();
        okuyucular.push(std::thread::spawn(move || satir_oku(o, tx)));
    }
    drop(tx);
    for s in rx {
        satir(&s);
    }
    for o in okuyucular {
        let _ = o.join();
    }
    let durum = cocuk.wait().map_err(|e| Hata::Baslatilamadi(e.to_string()))?;
    Ok(durum.code())
}

/// Ham baytları \n veya \r ile bölerek satırlara ayırır (ilerleme çubuğu \r kullanabilir).
fn satir_oku(mut r: impl Read, tx: Sender<String>) {
    let mut tampon = [0u8; 8192];
    let mut satir: Vec<u8> = Vec::with_capacity(512);
    loop {
        match r.read(&mut tampon) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                for &b in &tampon[..n] {
                    if b == b'\n' || b == b'\r' {
                        if !satir.is_empty() {
                            let _ = tx.send(String::from_utf8_lossy(&satir).into_owned());
                            satir.clear();
                        }
                    } else {
                        satir.push(b);
                    }
                }
            }
        }
    }
    if !satir.is_empty() {
        let _ = tx.send(String::from_utf8_lossy(&satir).into_owned());
    }
}

fn sayi(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

fn hata_metni(satir: &str) -> Option<String> {
    let s = satir.strip_prefix("ERROR:")?.trim();
    Some(s.to_string())
}

/// Ara dosyalar için geçici klasör: çıktı klasörüyle aynı sürücüdeyse %LOCALAPPDATA%
/// altında (indirilenler klasörü kirlenmesin), değilse çıktı klasörünün içinde
/// (son taşıma kopyalamaya dönüşmesin).
fn gecici_klasor(cikti: &Path, id: u64) -> PathBuf {
    let surucu = |p: &Path| p.to_string_lossy().chars().next().map(|c| c.to_ascii_uppercase());
    if let Some(yerel) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
        && surucu(&yerel) == surucu(cikti)
    {
        return yerel.join("YtIndirici").join("gecici").join(format!("{}-{id}", std::process::id()));
    }
    cikti.join(".yt-indirici-gecici").join(format!("{}-{id}", std::process::id()))
}

fn gecici_temizle(d: &Path) {
    for _ in 0..10 {
        if !d.exists() || std::fs::remove_dir_all(d).is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    if let Some(ust) = d.parent() {
        let _ = std::fs::remove_dir(ust); // boşsa siler
    }
}

pub fn indir(
    id: u64,
    istek: Istek,
    ortam: Arc<Ortam>,
    is: Arc<Is>,
    tx: Sender<Mesaj>,
    ctx: eframe::egui::Context,
) {
    let g = Gonderici { id, tx, ctx };
    let gecici = gecici_klasor(&istek.cikti, id);
    let _ = std::fs::create_dir_all(&istek.cikti);

    let mut c = temel_komut(&ortam);
    c.args(["--no-playlist", "--progress", "--no-simulate"])
        .arg("--progress-template")
        .arg(format!(
            "download:{ETIKET}P|%(progress.status)s|%(progress.downloaded_bytes)s|%(progress.total_bytes)s|%(progress.total_bytes_estimate)s|%(progress.speed)s|%(progress.eta)s"
        ))
        .arg("--progress-template")
        .arg(format!("postprocess:{ETIKET}PP|%(progress.postprocessor)s|%(progress.status)s"))
        .arg("--print")
        .arg(format!("before_dl:{ETIKET}B|%(format_id)s|%(title)s"))
        .arg("--print")
        .arg(format!("after_move:{ETIKET}F|%(filepath)s"))
        .arg("-P")
        .arg(&istek.cikti)
        .arg("-P")
        .arg(format!("temp:{}", gecici.display()))
        .args(["-o", "%(title)s.%(ext)s"]);

    match istek.bicim {
        Bicim::Mp4 => {
            let cozunurluk = match istek.video_yukseklik {
                Some(h) => format!("res:{h}"),
                None => "res".to_string(),
            };
            // Önce çözünürlük, sonra kare hızı; aynı çözünürlükte kurgu programlarıyla en uyumlu
            // H.264 + AAC tercih edilir. 1080p üstünde YouTube yalnızca VP9/AV1 verir.
            c.args(["-f", "bv*+ba/b", "-S"])
                .arg(format!("{cozunurluk},fps,vcodec:h264,proto,acodec:aac"))
                .args(["--merge-output-format", "mp4", "--remux-video", "mp4"]);
        }
        Bicim::Mp3 => {
            c.args(["-f", "ba/b", "-x", "--audio-format", "mp3", "--audio-quality", istek.ses_kalite]);
        }
    }
    c.arg("--").arg(&istek.url);

    // İlerleme durumu: mp4'te video ve ses ayrı parçalar halinde iner.
    let mut parca_sayisi: u32 = 1;
    let mut biten_parca: u32 = 0;
    let mut parca_bitti = false;
    let mut son_oran = 0.0f64;
    let mut son_hata: Option<String> = None;
    let mut son_gonderim = Instant::now() - Duration::from_secs(1);

    g.gonder(Olay::Asama(Asama::BilgiAliniyor));
    let sonuc = calistir(c, &is, |s| {
        if let Some(h) = hata_metni(s) {
            son_hata = Some(h);
            return;
        }
        let Some(i) = s.find(ETIKET) else { return };
        let govde = &s[i + ETIKET.len()..];
        let mut p = govde.splitn(2, '|');
        let tur = p.next().unwrap_or("");
        let geri = p.next().unwrap_or("");
        match tur {
            "B" => {
                let mut q = geri.splitn(2, '|');
                let bicim_id = q.next().unwrap_or("");
                let baslik = q.next().unwrap_or("").trim();
                parca_sayisi = bicim_id.split('+').count().max(1) as u32;
                biten_parca = 0;
                parca_bitti = false;
                son_oran = 0.0;
                if !baslik.is_empty() && baslik != "NA" {
                    g.gonder(Olay::Baslik(baslik.to_string()));
                }
                g.gonder(Olay::Asama(Asama::Indiriliyor));
            }
            "P" => {
                let f: Vec<&str> = geri.split('|').collect();
                if f.len() < 6 {
                    return;
                }
                let durum = f[0];
                let inen = sayi(f[1]).unwrap_or(0.0);
                let toplam = sayi(f[2]).or_else(|| sayi(f[3])).unwrap_or(0.0);
                let oran = if toplam > 0.0 { (inen / toplam).clamp(0.0, 1.0) } else { 0.0 };
                if durum == "finished" {
                    if !parca_bitti {
                        biten_parca = (biten_parca + 1).min(parca_sayisi);
                        parca_bitti = true;
                    }
                    son_oran = 0.0;
                } else {
                    if parca_bitti {
                        parca_bitti = false;
                    } else if oran + 0.3 < son_oran {
                        // "finished" satırı gelmeden yeni parçaya geçildi.
                        biten_parca = (biten_parca + 1).min(parca_sayisi.saturating_sub(1));
                    }
                    son_oran = oran;
                }
                let anlik = if durum == "finished" { 0.0 } else { oran };
                let genel = ((biten_parca as f64 + anlik) / parca_sayisi as f64).clamp(0.0, 1.0);
                if durum == "finished" || son_gonderim.elapsed() >= Duration::from_millis(100) {
                    son_gonderim = Instant::now();
                    g.gonder(Olay::Ilerleme {
                        oran: genel as f32,
                        hiz: sayi(f[4]),
                        kalan: sayi(f[5]).map(|v| v as u64),
                    });
                }
            }
            "PP" => {
                let mut q = geri.split('|');
                let ad = q.next().unwrap_or("");
                let durum = q.next().unwrap_or("");
                if durum == "started" || durum == "processing" {
                    let asama = match ad {
                        "Merger" => Asama::Birlestiriliyor,
                        "ExtractAudio" => Asama::Mp3,
                        "VideoRemuxer" => Asama::Mp4,
                        "MoveFiles" => Asama::Tasiniyor,
                        _ => Asama::Isleniyor,
                    };
                    g.gonder(Olay::Asama(asama));
                }
            }
            "F" => {
                let yol = geri.trim();
                if !yol.is_empty() && yol != "NA" {
                    g.gonder(Olay::Dosya(PathBuf::from(yol)));
                }
            }
            _ => {}
        }
    });

    gecici_temizle(&gecici);

    if is.iptal_edildi() {
        g.gonder(Olay::Iptal);
        return;
    }
    let son = match sonuc {
        Ok(Some(0)) => Ok(()),
        Ok(kod) => Err(match (son_hata, kod) {
            (Some(h), _) => Hata::YtDlp(h),
            (None, Some(k)) => Hata::Kod(k),
            (None, None) => Hata::Beklenmedik,
        }),
        Err(e) => Err(e),
    };
    g.gonder(Olay::Bitti(son));
}

/// Oynatma listesi / kanal bağlantısını tek tek video bağlantılarına açar.
pub fn listeyi_coz(
    id: u64,
    url: String,
    ortam: Arc<Ortam>,
    is: Arc<Is>,
    tx: Sender<Mesaj>,
    ctx: eframe::egui::Context,
) {
    let g = Gonderici { id, tx, ctx };
    let mut c = temel_komut(&ortam);
    c.args(["--flat-playlist", "--yes-playlist", "--ignore-errors", "--print"])
        .arg(format!("{ETIKET}E|%(webpage_url,url)s|%(title)s"))
        .arg("--")
        .arg(&url);

    let mut ogeler: Vec<(String, String)> = Vec::new();
    let mut son_hata: Option<String> = None;
    let sonuc = calistir(c, &is, |s| {
        if let Some(h) = hata_metni(s) {
            son_hata = Some(h);
            return;
        }
        let Some(i) = s.find(ETIKET) else { return };
        let Some(geri) = s[i + ETIKET.len()..].strip_prefix("E|") else { return };
        let mut q = geri.splitn(2, '|');
        let u = q.next().unwrap_or("").trim();
        let b = q.next().unwrap_or("").trim();
        if u.is_empty() || u == "NA" {
            return;
        }
        let b = if b.is_empty() || b == "NA" { u } else { b };
        ogeler.push((u.to_string(), b.to_string()));
    });

    if is.iptal_edildi() {
        g.gonder(Olay::Iptal);
        return;
    }
    match sonuc {
        _ if !ogeler.is_empty() => g.gonder(Olay::Liste(ogeler)),
        Err(e) => g.gonder(Olay::Bitti(Err(e))),
        Ok(_) => g.gonder(Olay::Bitti(Err(son_hata.map(Hata::YtDlp).unwrap_or(Hata::ListeBos)))),
    }
}

/// Bağlantı bir liste/kanal mı? (Tek video bağlantıları doğrudan indirilir,
/// böylece fazladan bir yt-dlp çalıştırması beklenmez.)
pub fn liste_mi(url: &str, tum_liste: bool) -> bool {
    let u = url.to_ascii_lowercase();
    let tek_video = u.contains("watch?") && u.contains("v=") || u.contains("youtu.be/") || u.contains("/shorts/");
    if u.contains("list=") {
        return !tek_video || tum_liste;
    }
    if tek_video {
        return false;
    }
    u.contains("/playlist")
        || u.contains("youtube.com/@")
        || u.contains("youtube.com/channel/")
        || u.contains("youtube.com/c/")
        || u.contains("youtube.com/user/")
}
