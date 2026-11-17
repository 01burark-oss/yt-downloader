//! Arayüz dilleri. Her dil için tek bir metin tablosu; yt-dlp'nin kendi hata metinleri çevrilmez.

use serde::{Deserialize, Serialize};

use crate::worker::{Asama, Hata};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dil {
    Tr,
    En,
    De,
    Es,
    Ru,
    Zh,
}

pub const DILLER: &[Dil] = &[Dil::Tr, Dil::En, Dil::De, Dil::Es, Dil::Ru, Dil::Zh];

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetUserDefaultUILanguage() -> u16;
}

impl Dil {
    pub fn ad(self) -> &'static str {
        match self {
            Dil::Tr => "Türkçe",
            Dil::En => "English",
            Dil::De => "Deutsch",
            Dil::Es => "Español",
            Dil::Ru => "Русский",
            Dil::Zh => "简体中文",
        }
    }

    /// Windows arayüz dilinden varsayılan dil; desteklenmiyorsa İngilizce.
    pub fn sistemden() -> Dil {
        let birincil = unsafe { GetUserDefaultUILanguage() } & 0x3ff;
        match birincil {
            0x1f => Dil::Tr,
            0x07 => Dil::De,
            0x0a => Dil::Es,
            0x19 => Dil::Ru,
            0x04 => Dil::Zh,
            _ => Dil::En,
        }
    }

    pub fn m(self) -> &'static Metinler {
        match self {
            Dil::Tr => &TR,
            Dil::En => &EN,
            Dil::De => &DE,
            Dil::Es => &ES,
            Dil::Ru => &RU,
            Dil::Zh => &ZH,
        }
    }
}

/// Tek kopya kilidi mevcut pencereyi bu başlıklardan biriyle arar.
pub fn tum_basliklar() -> Vec<&'static str> {
    DILLER.iter().map(|d| d.m().baslik).collect()
}

pub struct Metinler {
    pub baslik: &'static str,
    pub ipucu: &'static str,
    pub ekle: &'static str,
    pub bicim: &'static str,
    pub mp4_video: &'static str,
    pub mp3_ses: &'static str,
    pub kalite: &'static str,
    pub en_iyi: &'static str,
    pub tum_liste: &'static str,
    pub klasor: &'static str,
    pub degistir: &'static str,
    pub ac: &'static str,
    pub klasor_sec: &'static str,
    pub dil: &'static str,
    pub ytdlp_yok: &'static str,
    pub ffmpeg_yok: &'static str,
    pub kuyruk: &'static str,
    /// `{b}` biten, `{t}` toplam
    pub sayac: &'static str,
    pub bitenleri_temizle: &'static str,
    pub kuyruk_bos: &'static str,
    pub iptal: &'static str,
    pub tekrar_dene: &'static str,
    pub klasorde_goster: &'static str,
    pub kaldir: &'static str,
    pub sirada: &'static str,
    pub liste_cozumleniyor: &'static str,
    pub liste_onek: &'static str,
    pub tamamlandi: &'static str,
    pub iptal_edildi: &'static str,
    pub hata_onek: &'static str,
    pub baslatiliyor: &'static str,
    pub iptal_ediliyor: &'static str,
    pub bilgi_aliniyor: &'static str,
    pub indiriliyor: &'static str,
    pub birlestiriliyor: &'static str,
    pub mp3_donusturuluyor: &'static str,
    pub mp4_aktariliyor: &'static str,
    pub tasiniyor: &'static str,
    pub isleniyor: &'static str,
    /// `{t}` kalan süre
    pub kalan: &'static str,
    pub mb_sn: &'static str,
    pub kb_sn: &'static str,
    pub cikilsin_mi: &'static str,
    pub cikis_aciklama: &'static str,
    pub evet_cik: &'static str,
    pub vazgec: &'static str,
    pub ytdlp_baslatilamadi: &'static str,
    /// `{k}` çıkış kodu
    pub ytdlp_kod: &'static str,
    pub ytdlp_beklenmedik: &'static str,
    pub liste_bos: &'static str,
    pub ondalik: char,
    /// Türkçede yüzde işareti sayıdan önce gelir (%45).
    pub yuzde_once: bool,
}

impl Metinler {
    pub fn asama(&self, a: Asama) -> &'static str {
        match a {
            Asama::Baslatiliyor => self.baslatiliyor,
            Asama::BilgiAliniyor => self.bilgi_aliniyor,
            Asama::Indiriliyor => self.indiriliyor,
            Asama::Birlestiriliyor => self.birlestiriliyor,
            Asama::Mp3 => self.mp3_donusturuluyor,
            Asama::Mp4 => self.mp4_aktariliyor,
            Asama::Tasiniyor => self.tasiniyor,
            Asama::Isleniyor => self.isleniyor,
            Asama::IptalEdiliyor => self.iptal_ediliyor,
        }
    }

    pub fn hata(&self, h: &Hata) -> String {
        match h {
            Hata::Baslatilamadi(e) => format!("{}{e}", self.ytdlp_baslatilamadi),
            Hata::Kod(k) => self.ytdlp_kod.replace("{k}", &k.to_string()),
            Hata::Beklenmedik => self.ytdlp_beklenmedik.to_string(),
            Hata::ListeBos => self.liste_bos.to_string(),
            Hata::YtDlp(e) => e.clone(),
        }
    }

    pub fn sayac(&self, biten: usize, toplam: usize) -> String {
        self.sayac.replace("{b}", &biten.to_string()).replace("{t}", &toplam.to_string())
    }

    pub fn yuzde(&self, oran: f32) -> String {
        let n = (oran * 100.0).round() as u32;
        if self.yuzde_once { format!("%{n}") } else { format!("{n}%") }
    }

    pub fn hiz(&self, b: f64) -> String {
        let (d, birim) = if b >= 1024.0 * 1024.0 {
            (b / (1024.0 * 1024.0), self.mb_sn)
        } else {
            (b / 1024.0, self.kb_sn)
        };
        format!("{d:.1} {birim}").replace('.', &self.ondalik.to_string())
    }

    pub fn kalan(&self, s: u64) -> String {
        let t = if s >= 3600 {
            format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
        } else {
            format!("{}:{:02}", s / 60, s % 60)
        };
        self.kalan.replace("{t}", &t)
    }
}

static TR: Metinler = Metinler {
    baslik: "YT İndirici",
    ipucu: "Video ya da oynatma listesi bağlantısını yapıştırın (Ctrl+V)",
    ekle: "Ekle",
    bicim: "Biçim:",
    mp4_video: "MP4 video",
    mp3_ses: "MP3 ses",
    kalite: "Kalite:",
    en_iyi: "En iyi",
    tum_liste: "Video bağlantısında oynatma listesi de varsa tüm listeyi indir",
    klasor: "Klasör:",
    degistir: "Değiştir…",
    ac: "Aç",
    klasor_sec: "Çıktı klasörünü seçin",
    dil: "Dil:",
    ytdlp_yok: "yt-dlp.exe bulunamadı: programın yanına koyun ya da PATH'e ekleyin.",
    ffmpeg_yok: "ffmpeg PATH'te bulunamadı: birleştirme ve MP3 dönüştürme çalışmaz.",
    kuyruk: "İndirme kuyruğu",
    sayac: "{b}/{t} tamamlandı",
    bitenleri_temizle: "Bitenleri temizle",
    kuyruk_bos: "Kuyruk boş. Yukarıya bir bağlantı yapıştırıp Ekle'ye basın.",
    iptal: "İptal",
    tekrar_dene: "Tekrar dene",
    klasorde_goster: "Klasörde göster",
    kaldir: "Kaldır",
    sirada: "Sırada bekliyor",
    liste_cozumleniyor: "Liste çözümleniyor…",
    liste_onek: "Liste: ",
    tamamlandi: "Tamamlandı",
    iptal_edildi: "İptal edildi",
    hata_onek: "Hata: ",
    baslatiliyor: "Başlatılıyor…",
    iptal_ediliyor: "İptal ediliyor…",
    bilgi_aliniyor: "Bilgi alınıyor…",
    indiriliyor: "İndiriliyor",
    birlestiriliyor: "Birleştiriliyor (ffmpeg)…",
    mp3_donusturuluyor: "MP3'e dönüştürülüyor (ffmpeg)…",
    mp4_aktariliyor: "MP4'e aktarılıyor (ffmpeg)…",
    tasiniyor: "Dosya taşınıyor…",
    isleniyor: "İşleniyor…",
    kalan: "kalan {t}",
    mb_sn: "MB/sn",
    kb_sn: "KB/sn",
    cikilsin_mi: "Çıkılsın mı?",
    cikis_aciklama: "Süren ya da sırada bekleyen indirmeler iptal edilecek.",
    evet_cik: "Evet, çık",
    vazgec: "Vazgeç",
    ytdlp_baslatilamadi: "yt-dlp başlatılamadı: ",
    ytdlp_kod: "yt-dlp {k} koduyla çıktı",
    ytdlp_beklenmedik: "yt-dlp beklenmedik biçimde sonlandı",
    liste_bos: "Listede indirilebilir video bulunamadı",
    ondalik: ',',
    yuzde_once: true,
};

static EN: Metinler = Metinler {
    baslik: "YT Downloader",
    ipucu: "Paste a video or playlist link (Ctrl+V)",
    ekle: "Add",
    bicim: "Format:",
    mp4_video: "MP4 video",
    mp3_ses: "MP3 audio",
    kalite: "Quality:",
    en_iyi: "Best",
    tum_liste: "If a video link also contains a playlist, download the whole playlist",
    klasor: "Folder:",
    degistir: "Change…",
    ac: "Open",
    klasor_sec: "Choose output folder",
    dil: "Language:",
    ytdlp_yok: "yt-dlp.exe not found: put it next to the program or add it to PATH.",
    ffmpeg_yok: "ffmpeg not found in PATH: merging and MP3 conversion will not work.",
    kuyruk: "Download queue",
    sayac: "{b}/{t} done",
    bitenleri_temizle: "Clear finished",
    kuyruk_bos: "The queue is empty. Paste a link above and press Add.",
    iptal: "Cancel",
    tekrar_dene: "Retry",
    klasorde_goster: "Show in folder",
    kaldir: "Remove",
    sirada: "Waiting in queue",
    liste_cozumleniyor: "Reading playlist…",
    liste_onek: "Playlist: ",
    tamamlandi: "Completed",
    iptal_edildi: "Cancelled",
    hata_onek: "Error: ",
    baslatiliyor: "Starting…",
    iptal_ediliyor: "Cancelling…",
    bilgi_aliniyor: "Fetching info…",
    indiriliyor: "Downloading",
    birlestiriliyor: "Merging (ffmpeg)…",
    mp3_donusturuluyor: "Converting to MP3 (ffmpeg)…",
    mp4_aktariliyor: "Remuxing to MP4 (ffmpeg)…",
    tasiniyor: "Moving file…",
    isleniyor: "Processing…",
    kalan: "{t} left",
    mb_sn: "MB/s",
    kb_sn: "KB/s",
    cikilsin_mi: "Quit?",
    cikis_aciklama: "Running and queued downloads will be cancelled.",
    evet_cik: "Yes, quit",
    vazgec: "Keep open",
    ytdlp_baslatilamadi: "Could not start yt-dlp: ",
    ytdlp_kod: "yt-dlp exited with code {k}",
    ytdlp_beklenmedik: "yt-dlp terminated unexpectedly",
    liste_bos: "No downloadable videos found in the playlist",
    ondalik: '.',
    yuzde_once: false,
};

static DE: Metinler = Metinler {
    baslik: "YT Downloader",
    ipucu: "Video- oder Playlist-Link einfügen (Strg+V)",
    ekle: "Hinzufügen",
    bicim: "Format:",
    mp4_video: "MP4-Video",
    mp3_ses: "MP3-Audio",
    kalite: "Qualität:",
    en_iyi: "Beste",
    tum_liste: "Wenn ein Videolink auch eine Playlist enthält, die ganze Playlist herunterladen",
    klasor: "Ordner:",
    degistir: "Ändern…",
    ac: "Öffnen",
    klasor_sec: "Zielordner wählen",
    dil: "Sprache:",
    ytdlp_yok: "yt-dlp.exe nicht gefunden: neben das Programm legen oder zum PATH hinzufügen.",
    ffmpeg_yok: "ffmpeg nicht im PATH gefunden: Zusammenführen und MP3-Umwandlung funktionieren nicht.",
    kuyruk: "Download-Warteschlange",
    sayac: "{b}/{t} fertig",
    bitenleri_temizle: "Fertige entfernen",
    kuyruk_bos: "Die Warteschlange ist leer. Oben einen Link einfügen und auf Hinzufügen klicken.",
    iptal: "Abbrechen",
    tekrar_dene: "Erneut versuchen",
    klasorde_goster: "Im Ordner zeigen",
    kaldir: "Entfernen",
    sirada: "In der Warteschlange",
    liste_cozumleniyor: "Playlist wird gelesen…",
    liste_onek: "Playlist: ",
    tamamlandi: "Fertig",
    iptal_edildi: "Abgebrochen",
    hata_onek: "Fehler: ",
    baslatiliyor: "Wird gestartet…",
    iptal_ediliyor: "Wird abgebrochen…",
    bilgi_aliniyor: "Infos werden abgerufen…",
    indiriliyor: "Wird heruntergeladen",
    birlestiriliyor: "Wird zusammengeführt (ffmpeg)…",
    mp3_donusturuluyor: "Wird in MP3 umgewandelt (ffmpeg)…",
    mp4_aktariliyor: "Wird nach MP4 umgepackt (ffmpeg)…",
    tasiniyor: "Datei wird verschoben…",
    isleniyor: "Wird verarbeitet…",
    kalan: "noch {t}",
    mb_sn: "MB/s",
    kb_sn: "KB/s",
    cikilsin_mi: "Beenden?",
    cikis_aciklama: "Laufende und wartende Downloads werden abgebrochen.",
    evet_cik: "Ja, beenden",
    vazgec: "Offen lassen",
    ytdlp_baslatilamadi: "yt-dlp konnte nicht gestartet werden: ",
    ytdlp_kod: "yt-dlp wurde mit Code {k} beendet",
    ytdlp_beklenmedik: "yt-dlp wurde unerwartet beendet",
    liste_bos: "Keine herunterladbaren Videos in der Playlist gefunden",
    ondalik: ',',
    yuzde_once: false,
};

static ES: Metinler = Metinler {
    baslik: "Descargador YT",
    ipucu: "Pega un enlace de vídeo o de lista de reproducción (Ctrl+V)",
    ekle: "Añadir",
    bicim: "Formato:",
    mp4_video: "Vídeo MP4",
    mp3_ses: "Audio MP3",
    kalite: "Calidad:",
    en_iyi: "La mejor",
    tum_liste: "Si un enlace de vídeo también incluye una lista, descargar la lista completa",
    klasor: "Carpeta:",
    degistir: "Cambiar…",
    ac: "Abrir",
    klasor_sec: "Elige la carpeta de destino",
    dil: "Idioma:",
    ytdlp_yok: "No se encontró yt-dlp.exe: colócalo junto al programa o añádelo al PATH.",
    ffmpeg_yok: "No se encontró ffmpeg en el PATH: la unión y la conversión a MP3 no funcionarán.",
    kuyruk: "Cola de descargas",
    sayac: "{b}/{t} completadas",
    bitenleri_temizle: "Quitar terminadas",
    kuyruk_bos: "La cola está vacía. Pega un enlace arriba y pulsa Añadir.",
    iptal: "Cancelar",
    tekrar_dene: "Reintentar",
    klasorde_goster: "Mostrar en carpeta",
    kaldir: "Quitar",
    sirada: "En espera",
    liste_cozumleniyor: "Leyendo la lista…",
    liste_onek: "Lista: ",
    tamamlandi: "Completada",
    iptal_edildi: "Cancelada",
    hata_onek: "Error: ",
    baslatiliyor: "Iniciando…",
    iptal_ediliyor: "Cancelando…",
    bilgi_aliniyor: "Obteniendo información…",
    indiriliyor: "Descargando",
    birlestiriliyor: "Uniendo (ffmpeg)…",
    mp3_donusturuluyor: "Convirtiendo a MP3 (ffmpeg)…",
    mp4_aktariliyor: "Pasando a MP4 (ffmpeg)…",
    tasiniyor: "Moviendo el archivo…",
    isleniyor: "Procesando…",
    kalan: "quedan {t}",
    mb_sn: "MB/s",
    kb_sn: "KB/s",
    cikilsin_mi: "¿Salir?",
    cikis_aciklama: "Se cancelarán las descargas en curso y en espera.",
    evet_cik: "Sí, salir",
    vazgec: "Seguir aquí",
    ytdlp_baslatilamadi: "No se pudo iniciar yt-dlp: ",
    ytdlp_kod: "yt-dlp terminó con el código {k}",
    ytdlp_beklenmedik: "yt-dlp terminó de forma inesperada",
    liste_bos: "No se encontraron vídeos descargables en la lista",
    ondalik: ',',
    yuzde_once: false,
};

static RU: Metinler = Metinler {
    baslik: "YT Загрузчик",
    ipucu: "Вставьте ссылку на видео или плейлист (Ctrl+V)",
    ekle: "Добавить",
    bicim: "Формат:",
    mp4_video: "Видео MP4",
    mp3_ses: "Аудио MP3",
    kalite: "Качество:",
    en_iyi: "Лучшее",
    tum_liste: "Если ссылка на видео содержит плейлист, скачать весь плейлист",
    klasor: "Папка:",
    degistir: "Изменить…",
    ac: "Открыть",
    klasor_sec: "Выберите папку для сохранения",
    dil: "Язык:",
    ytdlp_yok: "yt-dlp.exe не найден: положите его рядом с программой или добавьте в PATH.",
    ffmpeg_yok: "ffmpeg не найден в PATH: объединение и конвертация в MP3 работать не будут.",
    kuyruk: "Очередь загрузок",
    sayac: "Готово: {b} из {t}",
    bitenleri_temizle: "Убрать завершённые",
    kuyruk_bos: "Очередь пуста. Вставьте ссылку выше и нажмите «Добавить».",
    iptal: "Отмена",
    tekrar_dene: "Повторить",
    klasorde_goster: "Показать в папке",
    kaldir: "Убрать",
    sirada: "В очереди",
    liste_cozumleniyor: "Чтение плейлиста…",
    liste_onek: "Плейлист: ",
    tamamlandi: "Готово",
    iptal_edildi: "Отменено",
    hata_onek: "Ошибка: ",
    baslatiliyor: "Запуск…",
    iptal_ediliyor: "Отмена…",
    bilgi_aliniyor: "Получение информации…",
    indiriliyor: "Загрузка",
    birlestiriliyor: "Объединение (ffmpeg)…",
    mp3_donusturuluyor: "Конвертация в MP3 (ffmpeg)…",
    mp4_aktariliyor: "Перепаковка в MP4 (ffmpeg)…",
    tasiniyor: "Перемещение файла…",
    isleniyor: "Обработка…",
    kalan: "осталось {t}",
    mb_sn: "МБ/с",
    kb_sn: "КБ/с",
    cikilsin_mi: "Выйти?",
    cikis_aciklama: "Текущие и ожидающие загрузки будут отменены.",
    evet_cik: "Да, выйти",
    vazgec: "Остаться",
    ytdlp_baslatilamadi: "Не удалось запустить yt-dlp: ",
    ytdlp_kod: "yt-dlp завершился с кодом {k}",
    ytdlp_beklenmedik: "yt-dlp неожиданно завершился",
    liste_bos: "В плейлисте не найдено видео для загрузки",
    ondalik: ',',
    yuzde_once: false,
};

static ZH: Metinler = Metinler {
    baslik: "YT 下载器",
    ipucu: "粘贴视频或播放列表链接 (Ctrl+V)",
    ekle: "添加",
    bicim: "格式：",
    mp4_video: "MP4 视频",
    mp3_ses: "MP3 音频",
    kalite: "质量：",
    en_iyi: "最佳",
    tum_liste: "如果视频链接中还包含播放列表，则下载整个播放列表",
    klasor: "文件夹：",
    degistir: "更改…",
    ac: "打开",
    klasor_sec: "选择保存文件夹",
    dil: "语言：",
    ytdlp_yok: "未找到 yt-dlp.exe：请将其放在程序旁边，或添加到 PATH。",
    ffmpeg_yok: "PATH 中未找到 ffmpeg：合并和 MP3 转换将无法使用。",
    kuyruk: "下载队列",
    sayac: "已完成 {b}/{t}",
    bitenleri_temizle: "清除已完成",
    kuyruk_bos: "队列为空。请在上方粘贴链接，然后点击“添加”。",
    iptal: "取消",
    tekrar_dene: "重试",
    klasorde_goster: "在文件夹中显示",
    kaldir: "移除",
    sirada: "排队中",
    liste_cozumleniyor: "正在读取播放列表…",
    liste_onek: "播放列表：",
    tamamlandi: "已完成",
    iptal_edildi: "已取消",
    hata_onek: "错误：",
    baslatiliyor: "正在启动…",
    iptal_ediliyor: "正在取消…",
    bilgi_aliniyor: "正在获取信息…",
    indiriliyor: "正在下载",
    birlestiriliyor: "正在合并 (ffmpeg)…",
    mp3_donusturuluyor: "正在转换为 MP3 (ffmpeg)…",
    mp4_aktariliyor: "正在封装为 MP4 (ffmpeg)…",
    tasiniyor: "正在移动文件…",
    isleniyor: "正在处理…",
    kalan: "剩余 {t}",
    mb_sn: "MB/秒",
    kb_sn: "KB/秒",
    cikilsin_mi: "要退出吗？",
    cikis_aciklama: "正在进行和排队中的下载将被取消。",
    evet_cik: "是，退出",
    vazgec: "不退出",
    ytdlp_baslatilamadi: "无法启动 yt-dlp：",
    ytdlp_kod: "yt-dlp 已退出，代码 {k}",
    ytdlp_beklenmedik: "yt-dlp 意外终止",
    liste_bos: "播放列表中未找到可下载的视频",
    ondalik: '.',
    yuzde_once: false,
};
