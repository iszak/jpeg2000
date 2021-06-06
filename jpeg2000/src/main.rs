#![allow(dead_code)]

use clap::Clap;
use std::error;
use std::error::Error;
use std::ffi::OsStr;
use std::fmt;
use std::fs::File;
use std::io::{self, BufReader};
use std::path::Path;

use jp2::decode_jp2;
use jpxml::{encode_jpxml, Representation};

#[derive(Debug)]
enum JP2000Error {
    UnsupportedExtension { extension: String },
}

impl error::Error for JP2000Error {}
impl fmt::Display for JP2000Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::UnsupportedExtension { extension } => {
                write!(f, "unsupported extension {}", extension)
            }
        }
    }
}

#[derive(Clap)]
struct Opts {
    #[clap(subcommand)]
    subcommand: SubCommand,
}

#[derive(Clap)]
enum SubCommand {
    JPXML(JPXML),
}

#[derive(Clap)]
struct JPXML {
    input: String,

    #[clap(short, long, default_value = "skeleton")]
    representation: Representation,
}

fn run() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let opts: Opts = Opts::parse();

    match opts.subcommand {
        SubCommand::JPXML(c) => {
            let path = Path::new(&c.input);
            let filename = match path.file_name().and_then(OsStr::to_str) {
                Some(value) => value,
                None => "",
            };
            let extension = match path.extension().and_then(OsStr::to_str) {
                Some(value) => value,
                None => "",
            };

            let file = File::open(path)?;

            match extension {
                "jp2" => {
                    let jp2 = decode_jp2(&mut BufReader::new(file))?;
                    let mut writer = io::stdout();
                    encode_jpxml(&mut writer, &jp2, c.representation, filename)?;
                }
                _ => {
                    return Err(JP2000Error::UnsupportedExtension {
                        extension: extension.to_owned(),
                    }
                    .into())
                }
            }
        }
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    match run() {
        Err(e) => {
            return Err(e.to_string().into());
        }
        Ok(_) => Ok(()),
    }
}
