//! Arayüz: tek pencere, bağlantı girişi, biçim/kalite, dil, çıktı klasörü ve indirme kuyruğu.

use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};

use eframe::egui::{self, Color32, RichText};

use crate::i18n::{DILLER, Dil, Metinler};
use crate::settings::{Ayarlar, Bicim, SES_KALITE, VIDEO_KALITE};
use crate::win::{self, Is};
use crate::worker::{self, Asama, Hata, Istek, Mesaj, Olay, Ortam};

const AYNI_ANDA: usize = 2;

#[derive(Clone, PartialEq)]
enum Durum {
    Bekliyor,
    Cozumleniyor,
    Calisiyor(Asama),
    Tamamlandi,
    Iptal,
    Hata(Hata),
}

impl Durum {
    fn aktif(&self) -> bool {
        matches!(self, Durum::Calisiyor(_) | Durum::Cozumleniyor)
    }
    fn bitmis(&self) -> bool {
        matches!(self, Durum::Tamamlandi | Durum::Iptal | Durum::Hata(_))
    }
}

struct Oge {
    id: u64,
    /// Oynatma listesi/kanal bağlantısı: çözümlenince tek tek öğelere açılır.
    liste: bool,
    baslik: String,
    istek: Istek,
    /// VIDEO_KALITE ya da SES_KALITE içindeki sıra (istek.bicim'e göre)
    kalite: usize,
    durum: Durum,
    oran: f32,
    hiz: Option<f64>,
    kalan: Option<u64>,
    dosya: Option<PathBuf>,
    is: Option<Arc<Is>>,
}

pub struct Uygulama {
    ayarlar: Ayarlar,
    kayitli: Ayarlar,
    sistem_dili: Dil,
    url_girdi: String,
    ogeler: Vec<Oge>,
    sonraki_id: u64,
    ortam: Arc<Ortam>,
    tx: Sender<Mesaj>,
    rx: Receiver<Mesaj>,
    kapatma_sorusu: bool,
    kapatmaya_izin: bool,
    ilk_kare: bool,
}

/// Çince/Japonca/Korece karakterler için sistem yazı tiplerini yedek olarak ekler.
/// egui'nin gömülü yazı tiplerinde CJK yok; büyük bir yazı tipini exe'ye gömmek yerine
/// Windows'ta hazır bulunanlar kullanılır.
fn cjk_yazi_tiplerini_yukle(ctx: &egui::Context) {
    use egui::epaint::text::{FontInsert, FontPriority, InsertFontFamily};
    let fonts = std::env::var_os("WINDIR")
        .map(|w| PathBuf::from(w).join("Fonts"))
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows\Fonts"));
    for (ad, dosya) in [("msyh", "msyh.ttc"), ("malgun", "malgun.ttf")] {
        if let Ok(bayt) = std::fs::read(fonts.join(dosya)) {
            let aileler = [egui::FontFamily::Proportional, egui::FontFamily::Monospace]
                .into_iter()
                .map(|family| InsertFontFamily { family, priority: FontPriority::Lowest })
                .collect();
            ctx.add_font(FontInsert::new(ad, egui::FontData::from_owned(bayt), aileler));
        }
    }
    ctx.request_repaint();
}

fn kalite_etiketi(m: &Metinler, bicim: Bicim, kalite: usize) -> &'static str {
    match bicim {
        Bicim::Mp4 if kalite == 0 => m.en_iyi,
        Bicim::Mp4 => VIDEO_KALITE[kalite].0,
        Bicim::Mp3 => SES_KALITE[kalite].0,
    }
}

impl Uygulama {
    pub fn new(ctx: &egui::Context, ayarlar: Ayarlar) -> Self {
        let sistem_dili = Dil::sistemden();
        if ayarlar.dil.unwrap_or(sistem_dili) == Dil::Zh {
            // Arayüz Çinceyse ilk karede kutucuk görünmesin.
            cjk_yazi_tiplerini_yukle(ctx);
        } else {
            // Diğer dillerde yalnızca CJK video başlıkları için gerekir: açılışı yavaşlatmasın.
            let ctx = ctx.clone();
            std::thread::spawn(move || cjk_yazi_tiplerini_yukle(&ctx));
        }
        let (tx, rx) = mpsc::channel();
        Self {
            kayitli: ayarlar.clone(),
            ayarlar,
            sistem_dili,
            url_girdi: String::new(),
            ogeler: Vec::new(),
            sonraki_id: 1,
            ortam: Arc::new(Ortam::bul()),
            tx,
            rx,
            kapatma_sorusu: false,
            kapatmaya_izin: false,
            ilk_kare: true,
        }
    }

    fn dil(&self) -> Dil {
        self.ayarlar.dil.unwrap_or(self.sistem_dili)
    }

    fn m(&self) -> &'static Metinler {
        self.dil().m()
    }

    /// Komut satırından gelen bağlantıları kuyruğa ekler (`yt-indirici.exe <url>... [--mp3|--mp4]`).
    /// Biçim seçeneği yalnızca bu bağlantılara uygulanır, kayıtlı ayarı değiştirmez.
    pub fn komut_satiri(&mut self, ctx: &egui::Context, argumanlar: &[String]) {
        let asil = self.ayarlar.bicim;
        for a in argumanlar {
            match a.as_str() {
                "--mp3" => self.ayarlar.bicim = Bicim::Mp3,
                "--mp4" => self.ayarlar.bicim = Bicim::Mp4,
                _ => {}
            }
        }
        let urller: Vec<&str> = argumanlar.iter().filter(|a| !a.starts_with("--")).map(String::as_str).collect();
        if !urller.is_empty() {
            self.url_girdi = urller.join(" ");
            self.ekle(ctx);
        }
        self.ayarlar.bicim = asil;
    }

    fn istek_olustur(&self, url: String) -> (Istek, usize) {
        let a = &self.ayarlar;
        let kalite = match a.bicim {
            Bicim::Mp4 => a.video_kalite,
            Bicim::Mp3 => a.ses_kalite,
        };
        (
            Istek {
                url,
                bicim: a.bicim,
                video_yukseklik: VIDEO_KALITE[a.video_kalite].1,
                ses_kalite: SES_KALITE[a.ses_kalite].1,
                cikti: a.cikti_klasoru.clone(),
            },
            kalite,
        )
    }

    fn yeni_oge(&mut self, istek: Istek, kalite: usize, baslik: String, liste: bool) -> Oge {
        let id = self.sonraki_id;
        self.sonraki_id += 1;
        Oge {
            id,
            liste,
            baslik,
            istek,
            kalite,
            durum: if liste { Durum::Cozumleniyor } else { Durum::Bekliyor },
            oran: 0.0,
            hiz: None,
            kalan: None,
            dosya: None,
            is: None,
        }
    }

    /// Girdi kutusundaki bağlantıları (boşluk/satırla ayrılmış olabilir) kuyruğa ekler.
    fn ekle(&mut self, ctx: &egui::Context) {
        let girdi = std::mem::take(&mut self.url_girdi);
        for url in girdi.split_whitespace().map(str::to_string) {
            let liste = worker::liste_mi(&url, self.ayarlar.tum_liste);
            let (istek, kalite) = self.istek_olustur(url.clone());
            let mut oge = self.yeni_oge(istek, kalite, url.clone(), liste);
            if liste {
                let is = Arc::new(Is::yeni());
                oge.is = Some(is.clone());
                let (id, ortam, tx, ctx) = (oge.id, self.ortam.clone(), self.tx.clone(), ctx.clone());
                std::thread::spawn(move || worker::listeyi_coz(id, url, ortam, is, tx, ctx));
            }
            self.ogeler.push(oge);
        }
    }

    fn baslat(&mut self, i: usize, ctx: &egui::Context) {
        let is = Arc::new(Is::yeni());
        let oge = &mut self.ogeler[i];
        oge.durum = Durum::Calisiyor(Asama::Baslatiliyor);
        oge.oran = 0.0;
        oge.hiz = None;
        oge.kalan = None;
        oge.is = Some(is.clone());
        let (id, istek, ortam, tx, ctx) =
            (oge.id, oge.istek.clone(), self.ortam.clone(), self.tx.clone(), ctx.clone());
        std::thread::spawn(move || worker::indir(id, istek, ortam, is, tx, ctx));
    }

    fn zamanla(&mut self, ctx: &egui::Context) {
        if self.ortam.ytdlp.is_none() {
            return;
        }
        let mut aktif = self.ogeler.iter().filter(|o| !o.liste && o.durum.aktif()).count();
        while aktif < AYNI_ANDA {
            let Some(i) = self.ogeler.iter().position(|o| !o.liste && o.durum == Durum::Bekliyor) else {
                break;
            };
            self.baslat(i, ctx);
            aktif += 1;
        }
    }

    fn mesajlari_isle(&mut self) {
        while let Ok(m) = self.rx.try_recv() {
            let Some(i) = self.ogeler.iter().position(|o| o.id == m.id) else { continue };
            match m.olay {
                Olay::Liste(girdiler) => {
                    let ana = self.ogeler.remove(i);
                    let yeni: Vec<Oge> = girdiler
                        .into_iter()
                        .map(|(url, baslik)| {
                            let mut istek = ana.istek.clone();
                            istek.url = url;
                            self.yeni_oge(istek, ana.kalite, baslik, false)
                        })
                        .collect();
                    self.ogeler.splice(i..i, yeni);
                }
                olay => {
                    let o = &mut self.ogeler[i];
                    let iptal_ediliyor = o.durum == Durum::Calisiyor(Asama::IptalEdiliyor);
                    match olay {
                        Olay::Baslik(b) => o.baslik = b,
                        Olay::Ilerleme { oran, hiz, kalan } if !iptal_ediliyor => {
                            o.oran = oran;
                            o.hiz = hiz;
                            o.kalan = kalan;
                            o.durum = Durum::Calisiyor(Asama::Indiriliyor);
                        }
                        Olay::Asama(a) if !iptal_ediliyor => {
                            if a != Asama::Indiriliyor {
                                o.hiz = None;
                                o.kalan = None;
                            }
                            o.durum = Durum::Calisiyor(a);
                        }
                        Olay::Dosya(p) => o.dosya = Some(p),
                        Olay::Bitti(Ok(())) => {
                            o.durum = Durum::Tamamlandi;
                            o.oran = 1.0;
                            o.is = None;
                        }
                        Olay::Bitti(Err(e)) => {
                            o.durum = Durum::Hata(e);
                            o.is = None;
                        }
                        Olay::Iptal => {
                            o.durum = Durum::Iptal;
                            o.is = None;
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn iptal(&mut self, i: usize) {
        let o = &mut self.ogeler[i];
        match &o.durum {
            Durum::Bekliyor => o.durum = Durum::Iptal,
            d if d.aktif() => {
                if let Some(is) = &o.is {
                    is.iptal();
                }
                o.durum = Durum::Calisiyor(Asama::IptalEdiliyor);
            }
            _ => {}
        }
    }

    fn ust_bolum(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let m = self.m();
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let genislik = (ui.available_width() - 90.0).max(120.0);
            let yanit = ui.add(
                egui::TextEdit::singleline(&mut self.url_girdi)
                    .hint_text(m.ipucu)
                    .desired_width(genislik),
            );
            if self.ilk_kare {
                yanit.request_focus();
            }
            let enter = yanit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let bos = self.url_girdi.trim().is_empty();
            let tik = ui.add_enabled(!bos, egui::Button::new(m.ekle).min_size(egui::vec2(80.0, 0.0))).clicked();
            if (enter || tik) && !bos {
                self.ekle(&ctx);
                yanit.request_focus();
            }
        });

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(m.bicim);
            ui.radio_value(&mut self.ayarlar.bicim, Bicim::Mp4, m.mp4_video);
            ui.radio_value(&mut self.ayarlar.bicim, Bicim::Mp3, m.mp3_ses);
            ui.add_space(16.0);
            ui.label(m.kalite);
            match self.ayarlar.bicim {
                Bicim::Mp4 => {
                    egui::ComboBox::from_id_salt("video_kalite")
                        .selected_text(kalite_etiketi(m, Bicim::Mp4, self.ayarlar.video_kalite))
                        .show_ui(ui, |ui| {
                            for i in 0..VIDEO_KALITE.len() {
                                ui.selectable_value(&mut self.ayarlar.video_kalite, i, kalite_etiketi(m, Bicim::Mp4, i));
                            }
                        });
                }
                Bicim::Mp3 => {
                    egui::ComboBox::from_id_salt("ses_kalite")
                        .selected_text(SES_KALITE[self.ayarlar.ses_kalite].0)
                        .show_ui(ui, |ui| {
                            for (i, (ad, _)) in SES_KALITE.iter().enumerate() {
                                ui.selectable_value(&mut self.ayarlar.ses_kalite, i, *ad);
                            }
                        });
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut secili = self.dil();
                egui::ComboBox::from_id_salt("dil").selected_text(secili.ad()).show_ui(ui, |ui| {
                    for d in DILLER {
                        ui.selectable_value(&mut secili, *d, d.ad());
                    }
                });
                ui.label(m.dil);
                if secili != self.dil() {
                    self.ayarlar.dil = Some(secili);
                    if secili == Dil::Zh {
                        cjk_yazi_tiplerini_yukle(&ctx);
                    }
                    ctx.send_viewport_cmd(egui::ViewportCommand::Title(secili.m().baslik.to_string()));
                }
            });
        });
        ui.checkbox(&mut self.ayarlar.tum_liste, m.tum_liste);

        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.label(m.klasor);
            if ui.button(m.degistir).clicked()
                && let Some(d) = rfd::FileDialog::new()
                    .set_title(m.klasor_sec)
                    .set_directory(&self.ayarlar.cikti_klasoru)
                    .pick_folder()
            {
                self.ayarlar.cikti_klasoru = d;
            }
            if ui.button(m.ac).clicked() {
                let _ = std::fs::create_dir_all(&self.ayarlar.cikti_klasoru);
                gezgin_ac(&self.ayarlar.cikti_klasoru, false);
            }
            ui.add(egui::Label::new(self.ayarlar.cikti_klasoru.display().to_string()).truncate());
        });

        let kirmizi = Color32::from_rgb(220, 70, 60);
        if self.ortam.ytdlp.is_none() {
            ui.colored_label(kirmizi, m.ytdlp_yok);
        }
        if self.ortam.ffmpeg.is_none() {
            ui.colored_label(kirmizi, m.ffmpeg_yok);
        }
    }

    fn kuyruk(&mut self, ui: &mut egui::Ui) {
        let m = self.m();
        let toplam = self.ogeler.len();
        let biten = self.ogeler.iter().filter(|o| o.durum == Durum::Tamamlandi).count();
        ui.horizontal(|ui| {
            ui.heading(m.kuyruk);
            if toplam > 0 {
                ui.label(RichText::new(m.sayac(biten, toplam)).weak());
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let temizlenecek = self.ogeler.iter().any(|o| o.durum.bitmis());
                if ui.add_enabled(temizlenecek, egui::Button::new(m.bitenleri_temizle)).clicked() {
                    self.ogeler.retain(|o| !o.durum.bitmis());
                }
            });
        });
        ui.add_space(2.0);

        if self.ogeler.is_empty() {
            ui.add_space(24.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(m.kuyruk_bos).weak());
            });
            return;
        }

        let mut iptal_et = None;
        let mut kaldir = None;
        let mut tekrar = None;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for (i, o) in self.ogeler.iter().enumerate() {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let baslik = if o.liste { format!("{}{}", m.liste_onek, o.baslik) } else { o.baslik.clone() };
                    ui.add(egui::Label::new(RichText::new(baslik).strong()).truncate())
                        .on_hover_text(&o.istek.url);
                    ui.horizontal(|ui| {
                        let dugme_alani = 260.0;
                        let pb = egui::ProgressBar::new(o.oran)
                            .desired_width((ui.available_width() - dugme_alani).max(80.0))
                            .text(m.yuzde(o.oran));
                        let pb = match o.durum {
                            Durum::Tamamlandi => pb.fill(Color32::from_rgb(60, 150, 80)),
                            Durum::Hata(_) => pb.fill(Color32::from_rgb(170, 60, 50)),
                            _ => pb,
                        };
                        ui.add(pb);
                        if !o.durum.bitmis() && ui.button(m.iptal).clicked() {
                            iptal_et = Some(i);
                        }
                        if matches!(o.durum, Durum::Hata(_) | Durum::Iptal)
                            && !o.liste
                            && ui.button(m.tekrar_dene).clicked()
                        {
                            tekrar = Some(i);
                        }
                        if o.durum == Durum::Tamamlandi
                            && let Some(f) = &o.dosya
                        {
                            if ui.button(m.ac).clicked() {
                                dosya_ac(f);
                            }
                            if ui.button(m.klasorde_goster).clicked() {
                                gezgin_ac(f, true);
                            }
                        }
                        if o.durum.bitmis() && ui.button(m.kaldir).clicked() {
                            kaldir = Some(i);
                        }
                    });
                    let bicim = match o.istek.bicim {
                        Bicim::Mp4 => "MP4",
                        Bicim::Mp3 => "MP3",
                    };
                    let mut bilgi = format!("{bicim} · {} — ", kalite_etiketi(m, o.istek.bicim, o.kalite));
                    let mut hata_metni = None;
                    match &o.durum {
                        Durum::Bekliyor => bilgi.push_str(m.sirada),
                        Durum::Cozumleniyor => bilgi.push_str(m.liste_cozumleniyor),
                        Durum::Calisiyor(a) => {
                            bilgi.push_str(m.asama(*a));
                            if let Some(h) = o.hiz {
                                bilgi.push_str(&format!(" · {}", m.hiz(h)));
                            }
                            if let Some(k) = o.kalan {
                                bilgi.push_str(&format!(" · {}", m.kalan(k)));
                            }
                        }
                        Durum::Tamamlandi => bilgi.push_str(m.tamamlandi),
                        Durum::Iptal => bilgi.push_str(m.iptal_edildi),
                        Durum::Hata(e) => {
                            let metin = m.hata(e);
                            bilgi.push_str(m.hata_onek);
                            bilgi.push_str(&metin);
                            hata_metni = Some(metin);
                        }
                    }
                    let mut metin = RichText::new(bilgi).small();
                    if hata_metni.is_some() {
                        metin = metin.color(Color32::from_rgb(220, 70, 60));
                    }
                    let y = ui.add(egui::Label::new(metin).truncate());
                    if let Some(h) = hata_metni {
                        y.on_hover_text(h);
                    }
                });
            }
        });

        if let Some(i) = iptal_et {
            self.iptal(i);
        }
        if let Some(i) = tekrar {
            let o = &mut self.ogeler[i];
            o.durum = Durum::Bekliyor;
            o.oran = 0.0;
        }
        if let Some(i) = kaldir {
            self.ogeler.remove(i);
        }
    }

    fn kapatma(&mut self, ctx: &egui::Context) {
        let m = self.m();
        let aktif = self.ogeler.iter().any(|o| o.durum.aktif() || o.durum == Durum::Bekliyor);
        if ctx.input(|i| i.viewport().close_requested()) && aktif && !self.kapatmaya_izin {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.kapatma_sorusu = true;
        }
        if !self.kapatma_sorusu {
            return;
        }
        let modal = egui::Modal::new(egui::Id::unique("kapatma")).show(ctx, |ui| {
            ui.set_width(320.0);
            ui.heading(m.cikilsin_mi);
            ui.label(m.cikis_aciklama);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(m.evet_cik).clicked() {
                    self.kapatmaya_izin = true;
                    self.kapatma_sorusu = false;
                    for o in &self.ogeler {
                        if let Some(is) = &o.is {
                            is.iptal();
                        }
                    }
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                if ui.button(m.vazgec).clicked() {
                    self.kapatma_sorusu = false;
                }
            });
        });
        if modal.should_close() {
            self.kapatma_sorusu = false;
        }
    }
}

impl eframe::App for Uygulama {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.ilk_kare
            && let (Some(ms), Some(yol)) = (win::surec_yasi_ms(), std::env::var_os("YTI_OLCUM"))
        {
            let _ = std::fs::write(PathBuf::from(yol), format!("{ms:.1}"));
        }

        self.mesajlari_isle();
        self.zamanla(&ctx);

        egui::CentralPanel::default().show(ui, |ui| {
            self.ust_bolum(ui);
            ui.separator();
            self.kuyruk(ui);
        });

        self.kapatma(&ctx);

        if self.ayarlar != self.kayitli {
            self.ayarlar.kaydet();
            self.kayitli = self.ayarlar.clone();
        }
        self.ilk_kare = false;
    }
}

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn gezgin_ac(yol: &std::path::Path, sec: bool) {
    let mut c = std::process::Command::new("explorer.exe");
    if sec {
        c.raw_arg(format!("/select,\"{}\"", yol.display()));
    } else {
        c.arg(yol);
    }
    let _ = c.spawn();
}

fn dosya_ac(yol: &std::path::Path) {
    // "start" boş başlık parametresi ister; yol tırnak içinde.
    let _ = std::process::Command::new("cmd")
        .creation_flags(CREATE_NO_WINDOW)
        .raw_arg(format!("/C start \"\" \"{}\"", yol.display()))
        .spawn();
}
