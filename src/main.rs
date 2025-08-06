use std::{env, ffi::OsString, fs::File, path::Path, process::exit};

use nom_exif::{EntryValue, ExifIter, ExifTag, MediaParser, MediaSource};
use thiserror::Error;

fn main() -> Result<(), RinominareError> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    let mut silent = false;
    if args.len() > 0 && args[0] == "silenzioso" {
        silent = true;
        args = args.into_iter().skip(1).collect();
    }
    if args.len() == 1 && args[0] == "tutti" {
        do_work(".", silent)
    } else if args.len() == 3 && args[0] == "tutti" && args[1] == "in" {
        do_work(&args[2], silent)
    } else {
        println!("Utilizza 1: rinominare [silenzioso] tutti");
        println!("Utilizza 2: rinominare [silenzioso] tutti in <PERCORSO>");
        exit(1);
    }
}

fn do_work(target_dir: &str, silent: bool) -> Result<(), RinominareError> {
    let mut parser = ImageParser::new();

    if !silent {
        println!("Controllo i file in {}", target_dir);
    }
    let paths = std::fs::read_dir(target_dir)?;

    for dir_entry in paths {
        let dir_entry = dir_entry?;
        if !dir_entry.file_type()?.is_file() {
            continue;
        }
        let path = dir_entry.path();
        let Some(file_name) = path.file_name() else {
            continue;
        };
        let f = file_name
            .to_str()
            .ok_or_else(|| RinominareError::FilenameError(file_name.to_os_string()))?;
        let mut prepend_string = match parser.get_image_date(&path) {
            Ok(Some(x)) => x,
            Ok(None) => {
                if !silent {
                    println!("Salto {} perché manca EXIF data", f);
                }
                continue;
            }
            Err(RinominareError::ExifError(nom_exif::Error::UnrecognizedFileFormat)) => {
                if !silent {
                    println!("Salto {} perché non conosco il formato del file", f);
                }
                continue;
            }
            e => {
                return e.map(|_| ());
            }
        };
        prepend_string.push('_');

        if f.starts_with(&prepend_string) {
            if !silent {
                println!("Salto {} perché ha giá il nome corretto", f);
            }
            continue;
        }
        prepend_string.push_str(f);
        let mut target_path = path
            .parent()
            .ok_or(RinominareError::PathError)?
            .to_path_buf();
        target_path.push(prepend_string);

        if !silent {
            println!("Rinomino {} come {}", path.display(), target_path.display());
        }
        std::fs::rename(&path, &target_path)?;
    }

    Ok(())
}

struct ImageParser {
    parser: MediaParser,
}

impl ImageParser {
    fn new() -> Self {
        Self {
            parser: MediaParser::new(),
        }
    }

    fn get_media_source(file_path: &Path) -> Result<Option<MediaSource<File>>, RinominareError> {
        match MediaSource::file_path(file_path) {
            Ok(ms) => Ok(Some(ms)),
            Err(nom_exif::Error::UnrecognizedFileFormat) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    fn get_image_date(&mut self, file_path: &Path) -> Result<Option<String>, RinominareError> {
        let Some(ms) = Self::get_media_source(file_path)?.filter(|ms| ms.has_exif()) else {
            return Ok(None);
        };

        let iter: ExifIter = self.parser.parse(ms)?;
        let mut time_values = iter
            .filter_map(|exif| match (exif.tag(), exif.get_value()) {
                (Some(ExifTag::DateTimeOriginal), Some(EntryValue::Time(v))) => {
                    Some((0, v.clone()))
                }
                (Some(ExifTag::CreateDate), Some(EntryValue::Time(v))) => Some((1, v.clone())),
                _ => None,
            })
            .collect::<Vec<_>>();
        time_values.sort_by_key(|(p, _)| *p);

        let Some((_, v)) = time_values.first() else {
            return Err(RinominareError::ExifDateTimeMissing);
        };

        Ok(Some(v.format("%Y%m%d").to_string()))
    }
}

#[derive(Debug, Error)]
enum RinominareError {
    #[error("EXIF error")]
    ExifError(#[from] nom_exif::Error),

    #[error("IO error")]
    IoError(#[from] std::io::Error),

    #[error("Filename contains illegal character combinations")]
    FilenameError(OsString),

    #[error("Unable to navigate filesystem properly (permission issue?)")]
    PathError,

    #[error("No original date found")]
    ExifDateTimeMissing,
}
