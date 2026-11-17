#![windows_subsystem = "windows"]

mod app;
mod i18n;
mod settings;
mod win;
mod worker;

use eframe::egui;

const KILIT_ADI: &str = "Local\\YtIndirici-TekKopya-7c1e";

fn main() -> eframe::Result {
    // Tek kopya: kilit başkasındaysa onun penceresini (hangi dilde açıksa) öne getir ve çık.
    let Some(_kilit) = win::tek_kopya_al(KILIT_ADI) else {
        win::pencereyi_one_getir(&i18n::tum_basliklar());
        return Ok(());
    };

    let ayarlar = settings::Ayarlar::yukle();
    let dil = ayarlar.dil.unwrap_or_else(i18n::Dil::sistemden);
    let baslik = dil.m().baslik;
    let secenekler = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(baslik)
            .with_inner_size([780.0, 560.0])
            .with_min_inner_size([520.0, 360.0]),
        centered: true,
        ..Default::default()
    };
    let argumanlar: Vec<String> = std::env::args().skip(1).collect();
    eframe::run_native(
        baslik,
        secenekler,
        Box::new(move |cc| {
            let mut uygulama = app::Uygulama::new(&cc.egui_ctx, ayarlar);
            uygulama.komut_satiri(&cc.egui_ctx, &argumanlar);
            Ok(Box::new(uygulama))
        }),
    )
}
