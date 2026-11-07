//! The web launcher's two languages: English, and Indonesian (the desktop launcher carries
//! `lang/*.json`; the web edition keeps its own few strings). Every string lives here, both
//! languages side by side, so a missing one cannot compile away.

use std::fmt::Debug;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Id,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Id => "id",
        }
    }

    /// The other of the two.
    pub fn other(self) -> Lang {
        match self {
            Lang::En => Lang::Id,
            Lang::Id => Lang::En,
        }
    }

    /// The language the browser prefers, of the two this has.
    pub fn from_browser() -> Lang {
        // The browser's list ("en-US,id,en"): the first of the two that appears in it wins.
        let asked = platform::languages().to_ascii_lowercase();
        let en = asked.find("en").unwrap_or(usize::MAX);
        let id = asked.find("id").unwrap_or(usize::MAX);
        match id < en {
            true => Lang::Id,
            false => Lang::En,
        }
    }
}

mod platform {
    #[cfg(target_arch = "wasm32")]
    pub fn languages() -> String {
        crate::glue::languages()
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn languages() -> String {
        "en".into()
    }
}

macro_rules! strings {
    ($($key:ident => $en:literal, $id:literal;)*) => {
        impl Lang {
            /// The string `key` is shown in, this language.
            pub fn t(self, key: Key) -> &'static str {
                match key {
                    $(Key::$key => match self {
                        Lang::En => $en,
                        Lang::Id => $id,
                    }),*
                }
            }
        }

        /// Every string the web launcher shows, both languages keep in step at compile time.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        #[allow(non_camel_case_types)]
        pub enum Key {
            $($key),*
        }
    };
}

strings! {
    // The pages.
    page_home => "Home", "Beranda";
    page_downloads => "Downloads", "Unduhan";
    page_minecraft => "Minecraft", "Minecraft";

    // Home.
    home_tagline => "Minecraft: Java Edition 26.3, rewritten in Rust.", "Minecraft: Java Edition 26.3, ditulis ulang dalam Rust.";
    home_web_line => "The launcher in the browser: this fork's releases, and Minecraft's files straight from Mojang — checked, resumable, no install.", "Peluncur di peramban: rilis fork ini, dan berkas Minecraft langsung dari Mojang — diperiksa, bisa dilanjutkan, tanpa pasang.";
    home_to_downloads => "Get Horizon", "Ambil Horizon";
    home_fork_title => "This fork adds", "Fork ini menambahkan";
    home_fork_direct => "Download Minecraft directly in the launcher, before playing — no other launcher, no sign-in.", "Unduh Minecraft langsung dari peluncur, sebelum bermain — tanpa peluncur lain, tanpa masuk akun.";
    home_fork_x86 => "32-bit Windows builds (i686) that run on Windows 7 and newer, for the oldest PCs.", "Build Windows 32-bit (i686) yang berjalan di Windows 7 dan seterusnya, untuk PC tertua.";
    home_fork_legacy => "The Legacy graphics preset for low-end machines, and the same care in this web build.", "Preset grafik Legacy untuk mesin kelas bawah, dan perhatian yang sama pada build web ini.";
    home_game_note => "The game itself runs natively: install the launcher for your system from Downloads.", "Game-nya sendiri berjalan natif: pasang peluncur untuk sistemmu dari Unduhan.";

    // Downloads.
    downloads_title => "Downloads", "Unduhan";
    downloads_line => "This fork's own releases, built by its workflow. The desktop launcher updates itself; people download from here once.", "Rilis fork ini sendiri, dibangun oleh workflow-nya. Peluncur desktop memperbarui dirinya sendiri; orang mengunduh dari sini sekali.";
    downloads_loading => "Loading the releases…", "Memuat rilis…";
    downloads_failed => "The releases could not be loaded", "Rilis tidak dapat dimuat";
    downloads_retry => "Try again", "Coba lagi";
    downloads_prerelease => "pre-release", "pra-rilis";
    downloads_this_device => "this device", "perangkat ini";
    downloads_get => "Download", "Unduh";
    downloads_none => "No releases yet: run the workflow with release=true, or push a v* tag.", "Belum ada rilis: jalankan workflow dengan release=true, atau dorong tag v*.";
    downloads_rate_limited => "GitHub's rate limit (60 an hour for a browser). Try again in a while, or use the releases page.", "Batas laju GitHub (60 per jam untuk peramban). Coba lagi nanti, atau pakai halaman rilis.";
    releases_page => "The releases on GitHub", "Rilis di GitHub";

    // Minecraft.
    mc_title => "Minecraft", "Minecraft";
    mc_intro => "Horizon plays the pinned version, checked against the sha1s research-lock.json pins — the same pins the desktop launcher checks.", "Horizon memainkan versi yang dipatok, diperiksa terhadap sha1 yang dipatok research-lock.json — patokan yang sama dengan peluncur desktop.";
    mc_client => "client jar", "jar klien";
    mc_version_file => "version file", "berkas versi";
    mc_asset_index => "asset index", "indeks aset";
    mc_browser_line => "A browser may download the version file, the client jar and the asset index from Mojang; the libraries and the sounds it may not (Mojang's other hosts send no cross-origin header). The desktop launcher's Download directly finishes those.", "Peramban boleh mengunduh berkas versi, jar klien, dan indeks aset dari Mojang; pustaka dan suara tidak boleh (host lain Mojang tidak mengirim header lintas-asal). Fitur Unduh langsung di peluncur desktop menyelesaikannya.";
    mc_pick_folder => "Choose a folder", "Pilih folder";
    mc_pick_again => "Choose another folder", "Pilih folder lain";
    mc_folder_ready => "Folder chosen", "Folder terpilih";
    mc_folder_none => "No folder chosen yet.", "Belum ada folder terpilih.";
    mc_folder_unsupported => "This browser cannot write into folders (Firefox, Safari). Chrome or Edge can; or use the desktop launcher.", "Peramban ini tidak bisa menulis ke folder (Firefox, Safari). Chrome atau Edge bisa; atau pakai peluncur desktop.";
    mc_download => "Download directly", "Unduh langsung";
    mc_downloading => "Downloading", "Mengunduh";
    mc_cancel => "Cancel", "Batal";
    mc_cancelled => "Stopped. Download again to go on where it stopped — the finished files are kept.", "Dihentikan. Unduh lagi untuk melanjutkan dari titik berhenti — berkas yang selesai tetap tersimpan.";
    mc_download_done => "Done. The desktop launcher's Download directly adds the libraries and the sounds.", "Selesai. Fitur Unduh langsung di peluncur desktop menambahkan pustaka dan suaranya.";
    mc_verify => "Verify the folder", "Periksa folder";
    mc_verify_again => "Verify again", "Periksa lagi";
    mc_verifying => "Reading the folder…", "Membaca folder…";
    mc_matches => "matches the pin", "cocok dengan patokan";
    mc_differs => "differs from the pin", "berbeda dari patokan";
    mc_missing => "not there yet", "belum ada";
    mc_verify_need_folder => "Choose a folder first.", "Pilih folder dulu.";
    mc_file_of_files => "of", "dari";
    mc_files_done => "files done", "berkas selesai";

    // The footer.
    footer_not_mojang => "Horizon is unofficial, free and open source (MIT), and not affiliated with Mojang. Play it with your own copy of Java Edition.", "Horizon tidak resmi, gratis dan sumber terbuka (MIT), serta tidak berafiliasi dengan Mojang. Mainkan dengan salinan Java Edition milikmu.";
}
