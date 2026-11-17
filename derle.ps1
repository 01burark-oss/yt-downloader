# Derleme betiği (GNU araç zinciri + Smart App Control açık makine için).
# Kullanım:
#   powershell -ExecutionPolicy Bypass -File derle.ps1 [-AraclarKlasoru <klasör>] [-Kur] [-KurulumKlasoru <klasör>]
# - AraclarKlasoru: GNU dlltool.exe + as.exe içeren klasör (ör. WinLibs/MinGW-w64 binutils). rustc
#   dlltool'u PATH'te arar; dlltool da as.exe'yi yalnızca kendi klasöründe arar. windows-link
#   (raw-dylib) için gerekli. Varsayılan: $env:YTI_MINGW_AS, yoksa C:\Users\Windows\Apps\mingw-as.
#   Klasör yoksa PATH'e eklenmez (ör. MSVC araç zinciriyle gerekmez).
# - Dev profili (Cargo.toml'da opt-level = 3) kullanılır: SAC'ın izin verdiği build script'ler yeniden
#   kullanılır. Çıktı: target\debug\yt-indirici.exe
# - Derleme düşük öncelikle ve çekirdeklerin yarısıyla çalışır.
# - -Kur: exe'yi KurulumKlasoru'na kopyalar (varsayılan C:\Users\Windows\Apps\yt-indirici).
param(
    [string]$AraclarKlasoru = $(if ($env:YTI_MINGW_AS) { $env:YTI_MINGW_AS } else { 'C:\Users\Windows\Apps\mingw-as' }),
    [switch]$Kur,
    [string]$KurulumKlasoru = 'C:\Users\Windows\Apps\yt-indirici'
)
$ErrorActionPreference = 'Stop'
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
if ($AraclarKlasoru -and (Test-Path -LiteralPath $AraclarKlasoru -PathType Container)) {
    $env:Path = "$AraclarKlasoru;$env:Path"
}
Set-Location $PSScriptRoot
$j = [Math]::Max(1, [int]([Environment]::ProcessorCount / 2))
$p = Start-Process -FilePath cargo -ArgumentList 'build', '-j', $j -NoNewWindow -PassThru
$null = $p.Handle  # ExitCode'un okunabilmesi için tutamaç önbelleğe alınır
try { $p.PriorityClass = 'BelowNormal' } catch {}
$p.WaitForExit()
if ($p.ExitCode -ne 0) { exit $p.ExitCode }
if ($Kur) {
    New-Item -ItemType Directory -Force $KurulumKlasoru | Out-Null
    Copy-Item 'target\debug\yt-indirici.exe' $KurulumKlasoru -Force
    "Kuruldu: $(Join-Path $KurulumKlasoru 'yt-indirici.exe')"
}
