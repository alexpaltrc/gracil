//! Every word the interface shows, in English and Spanish (ADR 0011).
//!
//! Gracil follows the Windows display language and has no language setting. Screens never spell
//! out text themselves; they ask [`Text`], so adding a language only touches this file.

use gracil_windows::TrayState;

use crate::work::Failure;

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
    pub drop_videos_here: &'static str,
    pub or_choose_files: &'static str,
    pub add_videos: &'static str,
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
    pub cancelled: &'static str,
    pub replaced: &'static str,
    pub copy_saved: &'static str,
    pub replaced_the_original: &'static str,
    pub choose_videos: &'static str,
    pub videos_filter: &'static str,
    pub all_files_filter: &'static str,
    pub open_gracil: &'static str,
    pub exit: &'static str,
    pub exit_question: &'static str,
    pub keep_working: &'static str,
    smaller: fn(u32) -> String,
    videos: fn(usize) -> String,
    ready: fn(usize) -> String,
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
    could_not_save: &'static str,
    name_taken: &'static str,
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
        (self.smaller)((smaller_by * 100.0).round() as u32)
    }

    pub fn videos(&self, count: usize) -> String {
        (self.videos)(count)
    }

    pub fn ready(&self, count: usize) -> String {
        (self.ready)(count)
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
    /// how many videos waiting won't be compressed.
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
            Failure::CouldNotSave => failures.could_not_save,
            Failure::NameTaken => failures.name_taken,
        }
    }
}

static ENGLISH: Text = Text {
    release_to_add: "Release to add",
    drop_videos_here: "Drop videos here",
    or_choose_files: "or choose files",
    add_videos: "Add videos",
    view_files: "View files",
    hide_files: "Hide files",
    replace: "Replace",
    save_copy: "Save copy",
    replace_all: "Replace all",
    save_all_copies: "Save all copies",
    clear: "Clear",
    getting_ready: "Getting ready",
    waiting: "Waiting",
    needs_internet_once: "Gracil needs an internet connection once, to set up its video engine",
    compressing: "Compressing",
    paused: "Paused",
    saving: "Saving",
    not_smaller: "Not smaller",
    not_compressed: "Not compressed",
    cancelled: "Cancelled",
    replaced: "Replaced",
    copy_saved: "Copy saved",
    replaced_the_original: "Replaced the original",
    choose_videos: "Choose videos",
    videos_filter: "Videos",
    all_files_filter: "All files",
    smaller: |percent| format!("{percent}% smaller"),
    videos: |count| match count {
        1 => "1 video".to_owned(),
        n => format!("{n} videos"),
    },
    ready: |count| format!("{count} ready"),
    saved_as: |name| format!("Saved as {name}"),
    replaced_by: |name| format!("Replaced by {name}"),
    open_gracil: "Open Gracil",
    exit: "Exit",
    exit_question: "Exit Gracil?",
    keep_working: "Keep working",
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
                "The video being compressed stops, and its original stays exactly as it was."
                    .to_owned(),
            );
        }
        match waiting {
            0 => {}
            1 => parts.push("1 video waiting won't be compressed.".to_owned()),
            n => parts.push(format!("{n} videos waiting won't be compressed.")),
        }
        parts.join(" ")
    },
    failures: Failures {
        no_video: "This file has no video",
        unreadable: "This file can't be opened as a video",
        could_not_compress: "This video couldn't be compressed",
        not_smaller: "This video couldn't be made smaller than the original, so it isn't saved",
        could_not_save: "The compressed video couldn't be saved in this folder",
        name_taken: "Another file already has this video's new name, so the original was kept",
    },
};

static SPANISH: Text = Text {
    release_to_add: "Suelta para agregar",
    drop_videos_here: "Suelta videos aquí",
    or_choose_files: "o elige archivos",
    add_videos: "Agregar videos",
    view_files: "Ver archivos",
    hide_files: "Ocultar archivos",
    replace: "Reemplazar",
    save_copy: "Guardar copia",
    replace_all: "Reemplazar todos",
    save_all_copies: "Guardar todas las copias",
    clear: "Limpiar",
    getting_ready: "Preparando",
    waiting: "En espera",
    needs_internet_once: "Gracil necesita internet una sola vez, para preparar su motor de video",
    compressing: "Comprimiendo",
    paused: "En pausa",
    saving: "Guardando",
    not_smaller: "No se redujo",
    not_compressed: "Sin comprimir",
    cancelled: "Cancelado",
    replaced: "Reemplazado",
    copy_saved: "Copia guardada",
    replaced_the_original: "Se reemplazó el original",
    choose_videos: "Elegir videos",
    videos_filter: "Videos",
    all_files_filter: "Todos los archivos",
    smaller: |percent| format!("{percent}% más pequeño"),
    videos: |count| match count {
        1 => "1 video".to_owned(),
        n => format!("{n} videos"),
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
                "El video que se está comprimiendo se detiene, y su original queda exactamente \
                 como estaba."
                    .to_owned(),
            );
        }
        match waiting {
            0 => {}
            1 => parts.push("1 video en espera no se comprimirá.".to_owned()),
            n => parts.push(format!("{n} videos en espera no se comprimirán.")),
        }
        parts.join(" ")
    },
    failures: Failures {
        no_video: "Este archivo no tiene video",
        unreadable: "Este archivo no se puede abrir como video",
        could_not_compress: "Este video no se pudo comprimir",
        not_smaller: "Este video no se pudo hacer más pequeño que el original, así que no se guarda",
        could_not_save: "El video comprimido no se pudo guardar en esta carpeta",
        name_taken: "Ya hay otro archivo con el nuevo nombre de este video, así que se conservó el original",
    },
};

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
                .starts_with("The video being compressed stops")
        );
        assert!(!en.exit_explanation(true, 0).contains("waiting"));
        assert_eq!(
            en.exit_explanation(false, 2),
            "2 videos waiting won't be compressed."
        );
        let es = Text::get(Language::Spanish);
        assert!(
            es.exit_explanation(true, 1)
                .ends_with("1 video en espera no se comprimirá.")
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
        assert_eq!(en.videos(1), "1 video");
        assert_eq!(en.videos(3), "3 videos");
        assert_eq!(es.ready(1), "1 listo");
        assert_eq!(es.ready(2), "2 listos");
    }

    #[test]
    fn percentages_are_whole_numbers() {
        let es = Text::get(Language::Spanish);
        assert_eq!(es.smaller(0.756), "76% más pequeño");
        assert_eq!(Text::get(Language::English).smaller(0.004), "0% smaller");
    }
}
