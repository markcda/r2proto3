//! Маленькая утилита для конвертации помеченных структур, перечислений и функций в файл Protobuf 3
//! для обеспечения работы с gRPC-микросервисами.

#![feature(let_chains)]

mod utils;

mod types;
mod parser;

use clap::Parser as ArgParser;
use utils::R2Proto3Error;

use crate::parser::Parser;

/// Translates all `NOTE: ToProtobuf`-attributed structs, enums and functions from whole crate to Protobuf 3 file.
#[derive(ArgParser, Debug)]
#[command(version, about, long_about = None)]
// NOTE: ToProtobuf
struct Args {
  /// Path to selected crate
  #[arg(short, long)]
  crate_root: String,
  /// Ignore functions (rpc-services)
  #[arg(short, long, default_value = "false")]
  ignore_rpc: bool,
  /// Panic when marked type cannot be translated into Protobuf 3
  #[arg(short, long, default_value = "false")]
  panic_to_unsupported: bool,
  /// Verbose mode
  #[arg(short, long, default_value = "false")]
  verbose: bool,
  /// Output file
  #[arg(short, long, default_value = "generated.proto")]
  output_file: String,
}

fn main() {
  use std::fs::File;
  use std::io::Write;
  
  std::panic::set_hook(Box::new(|e| {
    println!("");
    println!("An error occured: {}", e.to_string());
  }));
  
  let args = Args::parse();
  let mut parser = Parser::new(&args.crate_root, args.ignore_rpc, args.panic_to_unsupported, args.verbose).unwrap();
  match parser.parse() {
    Err(err) => panic!("{}", err),
    Ok(()) => {
      let mut file = File::create(args.output_file).map_err(|e| R2Proto3Error::new(Some(Box::new(e)), "cannot truncate or create file")).unwrap();
      file.write(parser.generate().as_bytes()).map_err(|e| R2Proto3Error::new(Some(Box::new(e)), "cannot write proto contents to file")).unwrap();
    },
  }
}
