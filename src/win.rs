//! Windows'a özgü küçük yardımcılar: tek kopya kilidi, pencereyi öne getirme,
//! iş nesnesi (job object) ile süreç ağacını iptal etme, açılış ölçümü.

use std::os::windows::io::AsRawHandle;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, Ordering};

use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, FILETIME, GetLastError, HANDLE};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation, SetInformationJobObject,
    TerminateJobObject,
};
use windows_sys::Win32::System::SystemInformation::GetSystemTimeAsFileTime;
use windows_sys::Win32::System::Threading::{CreateMutexW, GetCurrentProcess, GetProcessTimes};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowW, IsIconic, SW_RESTORE, SW_SHOW, SetForegroundWindow, ShowWindow,
};

fn genis(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Tek kopya kilidi. `None` dönerse başka bir kopya zaten çalışıyor demektir.
/// Dönen tutamaç süreç boyunca açık kalmalıdır.
pub struct TekKopya(#[allow(dead_code)] HANDLE);

pub fn tek_kopya_al(ad: &str) -> Option<TekKopya> {
    let ad = genis(ad);
    unsafe {
        let h = CreateMutexW(null(), 0, ad.as_ptr());
        if h.is_null() {
            // Kilit oluşturulamadıysa engel olmayalım; uygulama yine açılsın.
            return Some(TekKopya(null_mut()));
        }
        if GetLastError() == ERROR_ALREADY_EXISTS {
            CloseHandle(h);
            return None;
        }
        Some(TekKopya(h))
    }
}

/// Başlıklardan biriyle (pencere başlığı seçili dile göre değişir) açık pencereyi
/// bulup öne getirir. İlk kopya henüz penceresini açmadıysa birkaç saniye bekler.
pub fn pencereyi_one_getir(basliklar: &[&str]) -> bool {
    let genis_basliklar: Vec<Vec<u16>> = basliklar.iter().map(|b| genis(b)).collect();
    for _ in 0..50 {
        unsafe {
            let hwnd = genis_basliklar
                .iter()
                .map(|b| FindWindowW(null(), b.as_ptr()))
                .find(|h| !h.is_null())
                .unwrap_or(null_mut());
            if !hwnd.is_null() {
                if IsIconic(hwnd) != 0 {
                    ShowWindow(hwnd, SW_RESTORE);
                } else {
                    ShowWindow(hwnd, SW_SHOW);
                }
                SetForegroundWindow(hwnd);
                return true;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    false
}

/// Bir indirmenin süreç ağacı (yt-dlp, onun Python alt süreci, ffmpeg) tek bir
/// iş nesnesinde tutulur; iptal edilince hepsi birlikte sonlandırılır. Uygulama
/// kapanınca tutamaç kapanır ve yarım kalan süreçler de otomatik ölür.
pub struct Is {
    h: HANDLE,
    iptal: AtomicBool,
}

unsafe impl Send for Is {}
unsafe impl Sync for Is {}

impl Is {
    pub fn yeni() -> Is {
        unsafe {
            let h = CreateJobObjectW(null(), null());
            if !h.is_null() {
                let mut bilgi: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                bilgi.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                SetInformationJobObject(
                    h,
                    JobObjectExtendedLimitInformation,
                    &bilgi as *const _ as *const core::ffi::c_void,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                );
            }
            Is { h, iptal: AtomicBool::new(false) }
        }
    }

    pub fn ata(&self, cocuk: &std::process::Child) {
        if !self.h.is_null() {
            unsafe {
                AssignProcessToJobObject(self.h, cocuk.as_raw_handle() as HANDLE);
            }
        }
        if self.iptal_edildi() {
            self.sonlandir();
        }
    }

    pub fn iptal(&self) {
        self.iptal.store(true, Ordering::SeqCst);
        self.sonlandir();
    }

    pub fn iptal_edildi(&self) -> bool {
        self.iptal.load(Ordering::SeqCst)
    }

    fn sonlandir(&self) {
        if !self.h.is_null() {
            unsafe {
                TerminateJobObject(self.h, 1);
            }
        }
    }
}

impl Drop for Is {
    fn drop(&mut self) {
        if !self.h.is_null() {
            unsafe {
                CloseHandle(self.h);
            }
        }
    }
}

fn ft(f: FILETIME) -> u64 {
    ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64
}

/// Sürecin oluşturulmasından bu yana geçen süre (ms). Açılış ölçümü için.
pub fn surec_yasi_ms() -> Option<f64> {
    unsafe {
        let mut olusma: FILETIME = std::mem::zeroed();
        let mut z1: FILETIME = std::mem::zeroed();
        let mut z2: FILETIME = std::mem::zeroed();
        let mut z3: FILETIME = std::mem::zeroed();
        if GetProcessTimes(GetCurrentProcess(), &mut olusma, &mut z1, &mut z2, &mut z3) == 0 {
            return None;
        }
        let mut simdi: FILETIME = std::mem::zeroed();
        GetSystemTimeAsFileTime(&mut simdi);
        Some(ft(simdi).saturating_sub(ft(olusma)) as f64 / 10_000.0)
    }
}
