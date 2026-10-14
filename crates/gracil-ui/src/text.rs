//! Every word the interface shows, in English and Spanish (ADR 0011).
//!
//! Gracil follows the Windows display language and has no language setting. Screens never spell
//! out text themselves; they ask [`Text`], so adding a language only touches this file.

use gracil_windows::TrayState;

use crate::app::Finished;
use crate::numbers::Numbers;
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
    /// Folders from File Explorer are being looked through and nothing was found yet (ADR 0037).
    pub looking: &'static str,
    /// The same, once some files are on the list already.
    pub looking_for_more: &'static str,
    pub drop_files_here: &'static str,
    pub or_choose_files: &'static str,
    pub add_files: &'static str,
    pub view_files: &'static str,
    pub hide_files: &'static str,
    pub carry_on: &'static str,
    /// After Clear, for a few seconds: the list was cleared, and Undo puts it back.
    pub list_cleared: &'static str,
    pub undo: &'static str,
    pub replace: &'static str,
    pub save_copy: &'static str,
    pub replace_all: &'static str,
    pub save_all_copies: &'static str,
    pub clear: &'static str,
    pub clear_list: &'static str,
    pub getting_ready: &'static str,
    pub waiting: &'static str,
    pub ffmpeg_missing: &'static str,
    pub compressing: &'static str,
    pub paused: &'static str,
    /// What the dial's bottom button does, shown in place of its square under the pointer.
    pub stop_all: &'static str,
    pub saving: &'static str,
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
    /// The tray menu's setting for Gracil in File Explorer's Send to (ADR 0058).
    pub explorer_menu: &'static str,
    /// The tray menu's setting for Gracil in File Explorer's main menu, which asks for an
    /// administrator's permission (ADR 0058).
    pub explorer_main_menu: &'static str,
    /// File Explorer's commands: for picked files, then for a folder's videos and images, its
    /// videos only and its images only.
    pub explorer_commands: [&'static str; 4],
    pub explorer_mini_window: &'static str,
    /// The tray menu's setting for starting with Windows (ADR 0054).
    pub start_with_windows: &'static str,
    /// The tray menu's way out before deleting Gracil's folder (ADR 0054).
    pub prepare_to_delete: &'static str,
    /// "Always", before the pill that picks what to do with every finished file (ADR 0025).
    pub always: &'static str,
    /// The pill's three places: always replace, ask each time, always save a copy.
    pub always_choices: [&'static str; 3],
    /// The big line once everything is done, when nothing came out smaller and nothing failed.
    pub nothing_to_shrink: &'static str,
    /// The same, when nothing came out smaller and something failed.
    pub nothing_compressed: &'static str,
    /// What the "Always" line is for, when the pointer rests on it.
    pub always_tip: &'static str,
    /// What the "×" on the unfinished-files line does.
    pub forget_unfinished: &'static str,
    /// A file that can't start because part of Gracil is missing.
    pub cannot_start: &'static str,
    /// Exit's question when Prepare to delete Gracil was chosen.
    pub prepare_question: &'static str,
    /// The tray's tip and the window's title while the work is getting ready, before 1%.
    pub tray_getting_ready: &'static str,
    smaller: fn(u32) -> String,
    in_all: fn(&str, u32) -> String,
    less: fn(&str) -> String,
    /// The count, and the count written in the user's number format.
    compressing_files: fn(usize, &str) -> String,
    files_left: fn(usize, &str) -> String,
    unfinished: fn(usize, &str) -> String,
    ready: fn(usize, &str) -> String,
    in_line: fn(usize, &str) -> String,
    compressed: fn(usize, &str) -> String,
    already: fn(usize, &str) -> String,
    failed: fn(usize, &str) -> String,
    saved_as: fn(&str) -> String,
    replaced_by: fn(&str) -> String,
    tray_tip: fn(TrayState) -> String,
    /// What Exit stops: how many files are being worked on, how many wait, and how many results
    /// wait for a choice, each also written.
    exit_explanation: fn([usize; 3], [&str; 3]) -> String,
    failures: Failures,
}

struct Failures {
    no_video: &'static str,
    unreadable: &'static str,
    could_not_compress: &'static str,
    not_smaller: &'static str,
    as_small_as_it_gets: &'static str,
    could_not_save: &'static str,
    could_not_place: &'static str,
    name_taken: &'static str,
    in_use: &'static str,
    gone: &'static str,
    animated: &'static str,
    not_accepted: &'static str,
    hdr: &'static str,
    no_encoder: &'static str,
    no_room: &'static str,
    too_large: &'static str,
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
            Numbers::system().one_decimal(value)
        } else {
            Numbers::system().whole(value.round() as u64)
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

    /// Files on the list not finished yet.
    /// What the ring stands for while it shows the whole list (ADR 0034, item 13).
    pub fn compressing_files(&self, count: usize) -> String {
        (self.compressing_files)(count, &number(count))
    }

    pub fn files_left(&self, count: usize) -> String {
        (self.files_left)(count, &number(count))
    }

    /// Files left unfinished the last time Gracil closed.
    pub fn unfinished(&self, count: usize) -> String {
        (self.unfinished)(count, &number(count))
    }

    pub fn ready(&self, count: usize) -> String {
        (self.ready)(count, &number(count))
    }

    /// How many more files wait their turn after the one shown (the footer's count includes it).
    pub fn in_line(&self, count: usize) -> String {
        (self.in_line)(count, &number(count))
    }

    /// The big line once the whole list is done: how much room it saved, or, when nothing came
    /// out smaller, whether that is because nothing could be or because something went wrong.
    pub fn headline(&self, finished: &Finished) -> String {
        if finished.compressed > 0 {
            self.less(finished.saved)
        } else if finished.failed > 0 {
            self.nothing_compressed.to_owned()
        } else {
            self.nothing_to_shrink.to_owned()
        }
    }

    /// How the whole list went: `22 files compressed  ·  4 were already compressed`. What failed
    /// is said apart, by [`Self::failed`], since it leads to those files.
    pub fn summary(&self, finished: &Finished) -> String {
        [
            (finished.compressed, self.compressed),
            (finished.already, self.already),
        ]
        .into_iter()
        .filter(|&(count, _)| count > 0)
        .map(|(count, words)| words(count, &number(count)))
        .collect::<Vec<_>>()
        .join("  ·  ")
    }

    /// How many files couldn't be compressed: `3 couldn't be compressed`.
    pub fn failed(&self, count: usize) -> String {
        (self.failed)(count, &number(count))
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

    /// What Exit will do to the work in progress (spec §54.1): how many files being worked on
    /// stop, how many files waiting are kept for next time, and how many results nobody chose
    /// for are let go, to be made again next time (ADR 0009, ADR 0042).
    pub fn exit_explanation(&self, stopping: usize, waiting: usize, unchosen: usize) -> String {
        let counts = [stopping, waiting, unchosen];
        let written = counts.map(number);
        (self.exit_explanation)(counts, [&written[0], &written[1], &written[2]])
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
            Failure::CouldNotPlace => failures.could_not_place,
            Failure::NameTaken => failures.name_taken,
            Failure::InUse => failures.in_use,
            Failure::Gone => failures.gone,
            Failure::Animated => failures.animated,
            Failure::NotAccepted => failures.not_accepted,
            Failure::Hdr => failures.hdr,
            Failure::NoEncoder => failures.no_encoder,
            Failure::NoRoom => failures.no_room,
            Failure::TooLarge => failures.too_large,
        }
    }
}

static ENGLISH: Text = Text {
    release_to_add: "Release to add",
    looking: "Looking through the folder…",
    looking_for_more: "looking for more",
    drop_files_here: "Drop videos or images here",
    or_choose_files: "or choose files",
    add_files: "Add files",
    view_files: "View files",
    hide_files: "Hide files",
    carry_on: "Continue",
    list_cleared: "List cleared",
    undo: "Undo",
    replace: "Replace",
    save_copy: "Save copy",
    replace_all: "Replace all",
    save_all_copies: "Save all as copies",
    clear: "Clear",
    clear_list: "Clear the list",
    getting_ready: "Getting ready",
    waiting: "Waiting",
    ffmpeg_missing: "Gracil is missing its video tool. Download Gracil again from where you got it",
    compressing: "Compressing",
    paused: "Paused",
    stop_all: "Stop all",
    saving: "Saving",
    not_compressed: "Not compressed",
    already_compressed: "Already compressed",
    cancelled: "Cancelled",
    replaced: "Replaced",
    copy_saved: "Copy saved",
    replaced_the_original: "Replaced the original",
    choose_files: "Choose files",
    media_filter: "Videos and images",
    all_files_filter: "All files",
    smaller: |percent| format!("{percent}% smaller"),
    in_all: |change, percent| format!("In all: {change}, {percent}% smaller"),
    less: |amount| format!("{amount} smaller"),
    compressing_files: |count, written| match count {
        1 => "Compressing 1 file".to_owned(),
        _ => format!("Compressing {written} files"),
    },
    files_left: |count, written| match count {
        0 => "No files left".to_owned(),
        1 => "1 file left".to_owned(),
        _ => format!("{written} files left"),
    },
    unfinished: |count, written| match count {
        1 => "1 file was left unfinished".to_owned(),
        _ => format!("{written} files were left unfinished"),
    },
    ready: |_, written| format!("{written} more to decide"),
    saved_as: |name| format!("Saved as {name}"),
    replaced_by: |name| format!("Replaced by {name}"),
    open_gracil: "Open Gracil",
    exit: "Exit",
    exit_question: "Exit Gracil?",
    keep_working: "Keep working",
    explorer_menu: "Show Gracil in Send to",
    explorer_main_menu: "Show Gracil in File Explorer's menu…",
    explorer_commands: [
        "Compress with Gracil",
        "Compress videos and images",
        "Compress videos only",
        "Compress images only",
    ],
    explorer_mini_window: "Show mini-window when compressing from File Explorer",
    start_with_windows: "Start with Windows",
    prepare_to_delete: "Prepare to delete Gracil",
    always: "Always",
    always_choices: ["Replace", "Ask", "Save copy"],
    nothing_to_shrink: "Nothing to make smaller",
    nothing_compressed: "Nothing was compressed",
    always_tip: "What to do with each finished file",
    forget_unfinished: "Forget these files",
    cannot_start: "Can't start",
    prepare_question: "Prepare to delete Gracil?",
    tray_getting_ready: "Gracil · getting ready",
    compressed: |count, written| match count {
        1 => "1 file compressed".to_owned(),
        _ => format!("{written} files compressed"),
    },
    already: |count, written| match count {
        1 => "1 was already compressed".to_owned(),
        _ => format!("{written} were already compressed"),
    },
    failed: |_, written| format!("{written} couldn't be compressed"),
    in_line: |_, written| format!("{written} more waiting"),
    tray_tip: |state| match state {
        TrayState::Idle => "Gracil".to_owned(),
        TrayState::Processing(done) => format!("Gracil · compressing {}", progress(done.into())),
        TrayState::Paused => "Gracil · paused".to_owned(),
        TrayState::Error => "Gracil · something needs your attention".to_owned(),
    },
    exit_explanation: |[stopping, waiting, unchosen],
                       [stopping_written, written, unchosen_written]| {
        let mut parts = Vec::new();
        match stopping {
            0 => {}
            1 => parts.push(
                "The file being compressed stops, and its original stays exactly as it was."
                    .to_owned(),
            ),
            _ => parts.push(format!(
                "The {stopping_written} files being compressed stop, and their originals stay \
                 exactly as they were."
            )),
        }
        match waiting {
            0 => {}
            1 => parts.push("1 file waiting is kept for next time.".to_owned()),
            _ => parts.push(format!("{written} files waiting are kept for next time.")),
        }
        match unchosen {
            0 => {}
            1 => parts.push(
                "The finished file you haven't saved yet is let go, and will be compressed again \
                 next time."
                    .to_owned(),
            ),
            _ => parts.push(format!(
                "The {unchosen_written} finished files you haven't saved yet are let go, and will \
                 be compressed again next time."
            )),
        }
        parts.join(" ")
    },
    failures: Failures {
        no_video: "This file has no video",
        unreadable: "This file can't be opened as a video or an image",
        could_not_compress: "This file couldn't be compressed. Try adding it again",
        not_smaller: "This file couldn't be made smaller than the original, so it was left as it is",
        as_small_as_it_gets: "This file is already compressed about as much as it can be, so it was left as it is",
        could_not_save: "The compressed file couldn't be saved in this folder. Make sure files can be saved there, then add it again",
        could_not_place: "The compressed file couldn't be saved in this folder. Make sure files can be saved there, then choose again",
        name_taken: "Another file already has this file's new name, so the original was kept. Save a copy instead",
        in_use: "Another program is using this file. Close it, then choose again",
        gone: "This file is no longer where it was, so it was left alone",
        animated: "Gracil doesn't compress moving images, so it was left as it is",
        not_accepted: "Gracil doesn't compress this kind of image, so it was left as it is",
        hdr: "This photo is stored in HDR, which Gracil doesn't compress, so it was left as it is",
        no_encoder: "Gracil is missing its photo tool. Download Gracil again from where you got it",
        no_room: "The drive is almost full, so this file was left as it is. Free up some space and add it again",
        too_large: "This image is too large for this computer's memory, so it was left as it is",
    },
};

static SPANISH: Text = Text {
    release_to_add: "Suelta para agregar",
    looking: "Buscando en la carpeta…",
    looking_for_more: "buscando más",
    drop_files_here: "Suelta videos o imágenes aquí",
    or_choose_files: "o elige archivos",
    add_files: "Agregar archivos",
    view_files: "Ver archivos",
    hide_files: "Ocultar archivos",
    carry_on: "Continuar",
    list_cleared: "Lista vaciada",
    undo: "Deshacer",
    replace: "Reemplazar",
    save_copy: "Guardar copia",
    replace_all: "Reemplazar todos",
    save_all_copies: "Guardar todos como copias",
    clear: "Limpiar",
    clear_list: "Limpiar la lista",
    getting_ready: "Preparando",
    waiting: "En espera",
    ffmpeg_missing: "A Gracil le falta su herramienta de video. Vuelve a descargarlo de donde lo obtuviste",
    compressing: "Comprimiendo",
    paused: "En pausa",
    stop_all: "Detener todo",
    saving: "Guardando",
    not_compressed: "Sin comprimir",
    already_compressed: "Ya está comprimido",
    cancelled: "Cancelado",
    replaced: "Reemplazado",
    copy_saved: "Copia guardada",
    replaced_the_original: "Se reemplazó el original",
    choose_files: "Elegir archivos",
    media_filter: "Videos e imágenes",
    all_files_filter: "Todos los archivos",
    smaller: |percent| format!("{percent}% más pequeño"),
    in_all: |change, percent| format!("En total: {change}, {percent}% más pequeño"),
    less: |amount| format!("{amount} menos"),
    files_left: |count, written| match count {
        0 => "No queda ninguno".to_owned(),
        1 => "Queda 1 archivo".to_owned(),
        _ => format!("Quedan {written} archivos"),
    },
    unfinished: |count, written| match count {
        1 => "Quedó 1 archivo sin terminar".to_owned(),
        _ => format!("Quedaron {written} archivos sin terminar"),
    },
    ready: |count, written| match count {
        1 => "1 más por decidir".to_owned(),
        _ => format!("{written} más por decidir"),
    },
    saved_as: |name| format!("Guardado como {name}"),
    replaced_by: |name| format!("Reemplazado por {name}"),
    open_gracil: "Abrir Gracil",
    exit: "Salir",
    exit_question: "¿Salir de Gracil?",
    keep_working: "Seguir trabajando",
    explorer_menu: "Mostrar Gracil en «Enviar a»",
    explorer_main_menu: "Mostrar Gracil en el menú del Explorador de archivos…",
    explorer_commands: [
        "Comprimir con Gracil",
        "Comprimir videos e imágenes",
        "Comprimir solo los videos",
        "Comprimir solo las imágenes",
    ],
    explorer_mini_window: "Mostrar miniventana al comprimir desde el Explorador de archivos",
    start_with_windows: "Iniciar con Windows",
    prepare_to_delete: "Preparar para eliminar Gracil",
    always: "Siempre",
    always_choices: ["Reemplazar", "Preguntar", "Guardar copia"],
    nothing_to_shrink: "No hay nada que reducir",
    nothing_compressed: "No se comprimió nada",
    always_tip: "Qué hacer con cada archivo terminado",
    forget_unfinished: "Olvidar estos archivos",
    cannot_start: "No se puede iniciar",
    prepare_question: "¿Preparar para eliminar Gracil?",
    tray_getting_ready: "Gracil · preparando",
    compressed: |count, written| match count {
        1 => "1 archivo comprimido".to_owned(),
        _ => format!("{written} archivos comprimidos"),
    },
    already: |count, written| match count {
        1 => "1 ya estaba comprimido".to_owned(),
        _ => format!("{written} ya estaban comprimidos"),
    },
    failed: |count, written| match count {
        1 => "1 no se pudo comprimir".to_owned(),
        _ => format!("{written} no se pudieron comprimir"),
    },
    in_line: |count, written| match count {
        1 => "1 más en espera".to_owned(),
        _ => format!("{written} más en espera"),
    },
    compressing_files: |count, written| match count {
        1 => "Comprimiendo 1 archivo".to_owned(),
        _ => format!("Comprimiendo {written} archivos"),
    },
    tray_tip: |state| match state {
        TrayState::Idle => "Gracil".to_owned(),
        TrayState::Processing(done) => format!("Gracil · comprimiendo {}", progress(done.into())),
        TrayState::Paused => "Gracil · en pausa".to_owned(),
        TrayState::Error => "Gracil · algo necesita tu atención".to_owned(),
    },
    exit_explanation: |[stopping, waiting, unchosen],
                       [stopping_written, written, unchosen_written]| {
        let mut parts = Vec::new();
        match stopping {
            0 => {}
            1 => parts.push(
                "El archivo que se está comprimiendo se detiene, y su original queda exactamente \
                 como estaba."
                    .to_owned(),
            ),
            _ => parts.push(format!(
                "Los {stopping_written} archivos que se están comprimiendo se detienen, y sus \
                 originales quedan exactamente como estaban."
            )),
        }
        match waiting {
            0 => {}
            1 => parts.push("1 archivo en espera se guarda para la próxima vez.".to_owned()),
            _ => parts.push(format!(
                "{written} archivos en espera se guardan para la próxima vez."
            )),
        }
        match unchosen {
            0 => {}
            1 => parts.push(
                "El archivo terminado que aún no guardaste se descarta y se volverá a comprimir la \
                 próxima vez."
                    .to_owned(),
            ),
            _ => parts.push(format!(
                "Los {unchosen_written} archivos terminados que aún no guardaste se descartan y se \
                 volverán a comprimir la próxima vez."
            )),
        }
        parts.join(" ")
    },
    failures: Failures {
        no_video: "Este archivo no tiene video",
        unreadable: "Este archivo no se puede abrir como video ni como imagen",
        could_not_compress: "Este archivo no se pudo comprimir. Intenta agregarlo de nuevo",
        not_smaller: "Este archivo no se pudo hacer más pequeño que el original, así que se dejó como está",
        as_small_as_it_gets: "Este archivo ya está comprimido casi al máximo, así que se dejó como está",
        could_not_save: "El archivo comprimido no se pudo guardar en esta carpeta. Comprueba que se pueden guardar archivos ahí y vuelve a agregarlo",
        could_not_place: "El archivo comprimido no se pudo guardar en esta carpeta. Comprueba que se pueden guardar archivos ahí y vuelve a elegir",
        name_taken: "Ya hay otro archivo con el nuevo nombre de este archivo, así que se conservó el original. Guarda una copia en su lugar",
        in_use: "Otro programa está usando este archivo. Ciérralo y vuelve a elegir",
        gone: "Este archivo ya no está donde estaba, así que no se tocó",
        animated: "Gracil no comprime imágenes en movimiento, así que se dejó como está",
        not_accepted: "Gracil no comprime este tipo de imagen, así que se dejó como está",
        hdr: "Esta foto está guardada en HDR, que Gracil no comprime, así que se dejó como está",
        no_encoder: "A Gracil le falta su herramienta de fotos. Vuelve a descargarlo de donde lo obtuviste",
        no_room: "El disco está casi lleno, así que este archivo se dejó como está. Libera espacio y vuelve a agregarlo",
        too_large: "Esta imagen es demasiado grande para la memoria de esta computadora, así que se dejó como está",
    },
};

/// A count in the user's number format: `15,031`.
fn number(count: usize) -> String {
    Numbers::system().whole(count as u64)
}

/// A share from 0 to 1 rounded to a whole percentage, for how much smaller a file became.
fn rounded_percent(share: f64) -> u32 {
    (share * 100.0).round() as u32
}

/// How far the work has gone, from 0 to 1, as a whole percentage: `42%`. It rounds down, so
/// 100% only shows once the work is done.
pub fn progress(share: f64) -> String {
    format!("{}%", (share.clamp(0.0, 1.0) * 100.0).floor())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_says_exactly_what_stops() {
        let en = Text::get(Language::English);
        assert!(
            en.exit_explanation(1, 0, 0)
                .starts_with("The file being compressed stops")
        );
        assert!(!en.exit_explanation(1, 0, 0).contains("waiting"));
        assert_eq!(
            en.exit_explanation(0, 2, 0),
            "2 files waiting are kept for next time."
        );
        let es = Text::get(Language::Spanish);
        assert!(
            es.exit_explanation(1, 1, 0)
                .ends_with("1 archivo en espera se guarda para la próxima vez.")
        );
        assert!(
            es.exit_explanation(4, 0, 0)
                .starts_with("Los 4 archivos que se están comprimiendo se detienen")
        );
        assert_eq!(
            en.exit_explanation(0, 0, 1),
            "The finished file you haven't saved yet is let go, and will be compressed again \
             next time."
        );
        assert!(
            es.exit_explanation(0, 0, 3)
                .starts_with("Los 3 archivos terminados")
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
        assert_eq!(en.files_left(0), "No files left");
        assert_eq!(en.files_left(1), "1 file left");
        assert_eq!(en.files_left(3), "3 files left");
        assert_eq!(en.compressing_files(24), "Compressing 24 files");
        assert_eq!(es.compressing_files(1), "Comprimiendo 1 archivo");
        assert_eq!(es.unfinished(1), "Quedó 1 archivo sin terminar");
        assert_eq!(en.unfinished(2), "2 files were left unfinished");
        assert_eq!(es.ready(1), "1 más por decidir");
        assert_eq!(es.ready(2), "2 más por decidir");
        assert_eq!(
            es.files_left(15_031),
            "Quedan 15,031 archivos",
            "in the number format of Windows"
        );
    }

    #[test]
    fn a_finished_list_says_how_it_went() {
        let (en, es) = (Text::get(Language::English), Text::get(Language::Spanish));
        let finished = Finished {
            saved: Sizes {
                original: 300 * 1024 * 1024,
                compressed: 170 * 1024 * 1024,
            },
            compressed: 22,
            already: 4,
            failed: 0,
        };
        assert_eq!(es.headline(&finished), "130 MB menos");
        assert_eq!(
            es.summary(&finished),
            "22 archivos comprimidos  ·  4 ya estaban comprimidos"
        );
        let nothing_smaller = Finished {
            already: 1,
            failed: 1_204,
            ..Finished::default()
        };
        assert_eq!(en.headline(&nothing_smaller), "Nothing was compressed");
        assert_eq!(en.summary(&nothing_smaller), "1 was already compressed");
        assert_eq!(en.failed(1_204), "1,204 couldn't be compressed");
        let already = Finished {
            already: 3,
            ..Finished::default()
        };
        assert_eq!(es.headline(&already), "No hay nada que reducir");
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
        assert_eq!(en.size(5_000 * 1024 * 1024 * 1024), "4.9 TB");
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
