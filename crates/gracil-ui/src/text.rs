//! Every word the interface shows, in English and Spanish (ADR 0011).
//!
//! Gracil follows the Windows display language and has no language setting. Screens never spell
//! out text themselves; they ask [`Text`], so adding a language only touches this file.

use gracil_windows::TrayState;

use crate::work::{Failure, Sizes};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    English,
    Spanish,
}

impl Language {
    /// The Windows display language. `GRACIL_LANGUAGE=en` or `es` overrides it during development.
    pub fn detect() -> Self {
        match std::env::var("GRACIL_LANGUAGE").as_deref() {
            Ok("es") => Self::Spanish,
            Ok("en") => Self::English,
            _ if gracil_windows::display_language_is_spanish() => Self::Spanish,
            _ => Self::English,
        }
    }
}

/// The interface's words in one language.
pub struct Text {
    pub release_to_add: &'static str,
    pub drop_files_here: &'static str,
    pub or_choose_files: &'static str,
    pub add_files: &'static str,
    pub view_files: &'static str,
    pub hide_files: &'static str,
    pub replace: &'static str,
    pub save_copy: &'static str,
    pub replace_all: &'static str,
    pub save_all_copies: &'static str,
    pub clear: &'static str,
    pub getting_ready: &'static str,
    pub waiting: &'static str,
    pub needs_internet_once: &'static str,
    pub compressing: &'static str,
    pub paused: &'static str,
    pub saving: &'static str,
    pub not_smaller: &'static str,
    pub not_compressed: &'static str,
    /// A file already about as small as it gets (ADR 0029, ADR 0033).
    pub already_compressed: &'static str,
    pub cancelled: &'static str,
    pub replaced: &'static str,
    pub copy_saved: &'static str,
    pub replaced_the_original: &'static str,
    pub choose_files: &'static str,
    pub media_filter: &'static str,
    pub all_files_filter: &'static str,
    pub open_gracil: &'static str,
    pub exit: &'static str,
    pub exit_question: &'static str,
    pub keep_working: &'static str,
    pub explorer_mini_window: &'static str,
    /// "Always", before the pill that picks what to do with every finished file (ADR 0025).
    pub always: &'static str,
    /// The pill's two ends: always replace, always save a copy. Its middle, "ask each time", is a
    /// small circle rather than a word.
    pub always_choices: [&'static str; 2],
    /// The decimal mark: a point in English, a comma in Spanish.
    decimal: char,
    smaller: fn(u32) -> String,
    in_all: fn(&str, u32) -> String,
    less: fn(&str) -> String,
    files: fn(usize) -> String,
    ready: fn(usize) -> String,
    in_line: fn(usize) -> String,
    saved_as: fn(&str) -> String,
    replaced_by: fn(&str) -> String,
    tray_tip: fn(TrayState) -> String,
    exit_explanation: fn(bool, usize) -> String,
    failures: Failures,
}

struct Failures {
    no_video: &'static str,
    unreadable: &'static str,
    could_not_compress: &'static str,
    not_smaller: &'static str,
    as_small_as_it_gets: &'static str,
    could_not_save: &'static str,
    name_taken: &'static str,
    animated: &'static str,
    not_accepted: &'static str,
    hdr: &'static str,
    no_encoder: &'static str,
}

impl Text {
    pub fn get(language: Language) -> &'static Self {
        match language {
            Language::English => &ENGLISH,
            Language::Spanish => &SPANISH,
        }
    }

    /// `smaller_by` is a share of the original, from 0 to 1.
    pub fn smaller(&self, smaller_by: f64) -> String {
        (self.smaller)(rounded_percent(smaller_by))
    }

    /// A file size the way File Explorer counts it, in KB, MB or GB: `78 MB`, `1.2 GB`.
    pub fn size(&self, bytes: u64) -> String {
        const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];
        let mut value = (bytes as f64 / 1024.0).max(1.0);
        let mut unit = 0;
        while value >= 999.5 && unit < UNITS.len() - 1 {
            value /= 1024.0;
            unit += 1;
        }
        let number = if value < 9.95 && unit > 0 {
            format!("{value:.1}").replace('.', &self.decimal.to_string())
        } else {
            format!("{value:.0}")
        };
        format!("{number} {}", UNITS[unit])
    }

    /// The size before and after: `120 MB → 78 MB`.
    pub fn change(&self, sizes: Sizes) -> String {
        format!(
            "{} → {}",
            self.size(sizes.original),
            self.size(sizes.compressed)
        )
    }

    /// The whole list together: `In all: 3.4 GB → 2.2 GB, 35% smaller`.
    pub fn in_all(&self, sizes: Sizes) -> String {
        (self.in_all)(&self.change(sizes), rounded_percent(sizes.smaller_by()))
    }

    /// How much room the list saved, in a few words: `1.2 GB smaller`.
    pub fn less(&self, sizes: Sizes) -> String {
        (self.less)(&self.size(sizes.original - sizes.compressed))
    }

    /// The drawer's short form: `−35%`.
    pub fn shrunk(&self, smaller_by: f64) -> String {
        format!("−{}%", rounded_percent(smaller_by))
    }

    pub fn files(&self, count: usize) -> String {
        (self.files)(count)
    }

    pub fn ready(&self, count: usize) -> String {
        (self.ready)(count)
    }

    /// How many files are waiting their turn.
    pub fn in_line(&self, count: usize) -> String {
        (self.in_line)(count)
    }

    pub fn saved_as(&self, name: &str) -> String {
        (self.saved_as)(name)
    }

    pub fn replaced_by(&self, name: &str) -> String {
        (self.replaced_by)(name)
    }

    /// What the tray icon says when the pointer rests on it.
    pub fn tray_tip(&self, state: TrayState) -> String {
        (self.tray_tip)(state)
    }

    /// What Exit will do to the work in progress (spec §54.1): whether a compression stops, and
    /// how many files waiting won't be compressed.
    pub fn exit_explanation(&self, stops_one: bool, waiting: usize) -> String {
        (self.exit_explanation)(stops_one, waiting)
    }

    /// Why something went wrong, as the "!" explains it.
    pub fn failure(&self, failure: Failure) -> &'static str {
        let failures = &self.failures;
        match failure {
            Failure::NoVideo => failures.no_video,
            Failure::Unreadable => failures.unreadable,
            Failure::CouldNotCompress => failures.could_not_compress,
            Failure::NotSmaller => failures.not_smaller,
            Failure::AsSmallAsItGets => failures.as_small_as_it_gets,
            Failure::CouldNotSave => failures.could_not_save,
            Failure::NameTaken => failures.name_taken,
            Failure::Animated => failures.animated,
            Failure::NotAccepted => failures.not_accepted,
            Failure::Hdr => failures.hdr,
            Failure::NoEncoder => failures.no_encoder,
        }
    }
}

static ENGLISH: Text = Text {
    release_to_add: "Release to add",
    drop_files_here: "Drop videos or images here",
    or_choose_files: "or choose files",
    add_files: "Add files",
    view_files: "View files",
    hide_files: "Hide files",
    replace: "Replace",
    save_copy: "Save copy",
    replace_all: "Replace all",
    save_all_copies: "Save all copies",
    clear: "Clear",
    getting_ready: "Getting ready",
    waiting: "Waiting",
    needs_internet_once: "Gracil needs an internet connection once, to finish setting up",
    compressing: "Compressing",
    paused: "Paused",
    saving: "Saving",
    not_smaller: "Not smaller",
    not_compressed: "Not compressed",
    already_compressed: "Already compressed",
    cancelled: "Cancelled",
    replaced: "Replaced",
    copy_saved: "Copy saved",
    replaced_the_original: "Replaced the original",
    choose_files: "Choose files",
    media_filter: "Videos and images",
    all_files_filter: "All files",
    decimal: '.',
    smaller: |percent| format!("{percent}% smaller"),
    in_all: |change, percent| format!("In all: {change}, {percent}% smaller"),
    less: |amount| format!("{amount} smaller"),
    files: |count| match count {
        1 => "1 file".to_owned(),
        n => format!("{n} files"),
    },
    ready: |count| format!("{count} ready"),
    saved_as: |name| format!("Saved as {name}"),
    replaced_by: |name| format!("Replaced by {name}"),
    open_gracil: "Open Gracil",
    exit: "Exit",
    exit_question: "Exit Gracil?",
    keep_working: "Keep working",
    explorer_mini_window: "Show mini-window when compressing from File Explorer",
    always: "Always",
    always_choices: ["Replace", "Save a copy"],
    in_line: |count| format!("{count} waiting"),
    tray_tip: |state| match state {
        TrayState::Idle => "Gracil".to_owned(),
        TrayState::Processing(done) => format!("Gracil · compressing {}%", percent(done)),
        TrayState::Paused => "Gracil · paused".to_owned(),
        TrayState::Error => "Gracil · something needs your attention".to_owned(),
    },
    exit_explanation: |stops_one, waiting| {
        let mut parts = Vec::new();
        if stops_one {
            parts.push(
                "The file being compressed stops, and its original stays exactly as it was."
                    .to_owned(),
            );
        }
        match waiting {
            0 => {}
            1 => parts.push("1 file waiting won't be compressed.".to_owned()),
            n => parts.push(format!("{n} files waiting won't be compressed.")),
        }
        parts.join(" ")
    },
    failures: Failures {
        no_video: "This file has no video",
        unreadable: "This file can't be opened as a video or an image",
        could_not_compress: "This file couldn't be compressed",
        not_smaller: "This file couldn't be made smaller than the original, so it isn't saved",
        as_small_as_it_gets: "This file is already compressed about as much as it can be, so it was left as it is",
        could_not_save: "The compressed file couldn't be saved in this folder",
        name_taken: "Another file already has this file's new name, so the original was kept",
        animated: "Gracil doesn't compress moving images, so it was left as it is",
        not_accepted: "Gracil doesn't compress this kind of image, so it was left as it is",
        hdr: "This photo is stored in HDR, which Gracil doesn't compress, so it was left as it is",
        no_encoder: "Gracil's photo compressor is missing. Reinstalling Gracil fixes it",
    },
};

static SPANISH: Text = Text {
    release_to_add: "Suelta para agregar",
    drop_files_here: "Suelta videos o imágenes aquí",
    or_choose_files: "o elige archivos",
    add_files: "Agregar archivos",
    view_files: "Ver archivos",
    hide_files: "Ocultar archivos",
    replace: "Reemplazar",
    save_copy: "Guardar copia",
    replace_all: "Reemplazar todos",
    save_all_copies: "Guardar todas las copias",
    clear: "Limpiar",
    getting_ready: "Preparando",
    waiting: "En espera",
    needs_internet_once: "Gracil necesita internet una sola vez, para terminar de prepararse",
    compressing: "Comprimiendo",
    paused: "En pausa",
    saving: "Guardando",
    not_smaller: "No se redujo",
    not_compressed: "Sin comprimir",
    already_compressed: "Ya está comprimido",
    cancelled: "Cancelado",
    replaced: "Reemplazado",
    copy_saved: "Copia guardada",
    replaced_the_original: "Se reemplazó el original",
    choose_files: "Elegir archivos",
    media_filter: "Videos e imágenes",
    all_files_filter: "Todos los archivos",
    decimal: ',',
    smaller: |percent| format!("{percent}% más pequeño"),
    in_all: |change, percent| format!("En total: {change}, {percent}% más pequeño"),
    less: |amount| format!("{amount} menos"),
    files: |count| match count {
        1 => "1 archivo".to_owned(),
        n => format!("{n} archivos"),
    },
    ready: |count| match count {
        1 => "1 listo".to_owned(),
        n => format!("{n} listos"),
    },
    saved_as: |name| format!("Guardado como {name}"),
    replaced_by: |name| format!("Reemplazado por {name}"),
    open_gracil: "Abrir Gracil",
    exit: "Salir",
    exit_question: "¿Salir de Gracil?",
    keep_working: "Seguir trabajando",
    explorer_mini_window: "Mostrar miniventana al comprimir desde el Explorador de archivos",
    always: "Siempre",
    always_choices: ["Reemplazar", "Guardar como copia"],
    in_line: |count| format!("{count} en espera"),
    tray_tip: |state| match state {
        TrayState::Idle => "Gracil".to_owned(),
        TrayState::Processing(done) => format!("Gracil · comprimiendo {}%", percent(done)),
        TrayState::Paused => "Gracil · en pausa".to_owned(),
        TrayState::Error => "Gracil · algo necesita tu atención".to_owned(),
    },
    exit_explanation: |stops_one, waiting| {
        let mut parts = Vec::new();
        if stops_one {
            parts.push(
                "El archivo que se está comprimiendo se detiene, y su original queda exactamente \
                 como estaba."
                    .to_owned(),
            );
        }
        match waiting {
            0 => {}
            1 => parts.push("1 archivo en espera no se comprimirá.".to_owned()),
            n => parts.push(format!("{n} archivos en espera no se comprimirán.")),
        }
        parts.join(" ")
    },
    failures: Failures {
        no_video: "Este archivo no tiene video",
        unreadable: "Este archivo no se puede abrir como video ni como imagen",
        could_not_compress: "Este archivo no se pudo comprimir",
        not_smaller: "Este archivo no se pudo hacer más pequeño que el original, así que no se guarda",
        as_small_as_it_gets: "Este archivo ya está comprimido casi al máximo, así que se dejó como está",
        could_not_save: "El archivo comprimido no se pudo guardar en esta carpeta",
        name_taken: "Ya hay otro archivo con el nuevo nombre de este archivo, así que se conservó el original",
        animated: "Gracil no comprime imágenes en movimiento, así que se dejó como está",
        not_accepted: "Gracil no comprime este tipo de imagen, así que se dejó como está",
        hdr: "Esta foto está guardada en HDR, que Gracil no comprime, así que se dejó como está",
        no_encoder: "Falta el compresor de fotos de Gracil. Reinstalar Gracil lo arregla",
    },
};

/// A share from 0 to 1 rounded to a whole percentage, for how much smaller a file became.
fn rounded_percent(share: f64) -> u32 {
    (share * 100.0).round() as u32
}

/// A share from 0 to 1 as a whole percentage, never above 100.
fn percent(share: f32) -> u32 {
    (share.clamp(0.0, 1.0) * 100.0).floor() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_says_exactly_what_stops() {
        let en = Text::get(Language::English);
        assert!(
            en.exit_explanation(true, 0)
                .starts_with("The file being compressed stops")
        );
        assert!(!en.exit_explanation(true, 0).contains("waiting"));
        assert_eq!(
            en.exit_explanation(false, 2),
            "2 files waiting won't be compressed."
        );
        let es = Text::get(Language::Spanish);
        assert!(
            es.exit_explanation(true, 1)
                .ends_with("1 archivo en espera no se comprimirá.")
        );
    }

    #[test]
    fn the_tray_tip_says_how_far_along() {
        let en = Text::get(Language::English);
        assert_eq!(en.tray_tip(TrayState::Idle), "Gracil");
        assert_eq!(
            en.tray_tip(TrayState::Processing(0.429)),
            "Gracil · compressing 42%"
        );
    }

    #[test]
    fn counts_agree_with_their_number() {
        let (en, es) = (Text::get(Language::English), Text::get(Language::Spanish));
        assert_eq!(en.files(1), "1 file");
        assert_eq!(en.files(3), "3 files");
        assert_eq!(es.ready(1), "1 listo");
        assert_eq!(es.ready(2), "2 listos");
    }

    #[test]
    fn percentages_are_whole_numbers() {
        let es = Text::get(Language::Spanish);
        assert_eq!(es.smaller(0.756), "76% más pequeño");
        assert_eq!(Text::get(Language::English).smaller(0.004), "0% smaller");
    }

    #[test]
    fn sizes_read_like_file_explorer() {
        let (en, es) = (Text::get(Language::English), Text::get(Language::Spanish));
        assert_eq!(en.size(300), "1 KB");
        assert_eq!(en.size(78 * 1024 * 1024), "78 MB");
        assert_eq!(en.size(1_288_490_189), "1.2 GB");
        assert_eq!(es.size(1_288_490_189), "1,2 GB");
        assert_eq!(en.size(1023 * 1024), "1.0 MB", "never 1023 KB");
        let sizes = Sizes {
            original: 100 * 1024 * 1024,
            compressed: 65 * 1024 * 1024,
        };
        assert_eq!(en.change(sizes), "100 MB → 65 MB");
        assert_eq!(en.in_all(sizes), "In all: 100 MB → 65 MB, 35% smaller");
        assert_eq!(es.less(sizes), "35 MB menos");
        assert_eq!(es.shrunk(sizes.smaller_by()), "−35%");
    }
}
