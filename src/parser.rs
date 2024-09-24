use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::Read;
use walkdir::WalkDir;

use crate::types::TypesParser;
use crate::utils::{MResult, R2Proto3Error};

#[derive(Debug)]
// NOTE: ToProtobuf
pub(crate) struct ProtobufField {
  pub name: String,
  pub rust_type: String,
  pub proto3_type: String,
  pub field_num: i32,
}

#[derive(Debug)]
// NOTE: ToProtobuf
pub(crate) struct ProtobufEnumVariant {
  pub name: String,
  pub value: i32,
}

// NOTE: ToProtobuf
pub(crate) enum ProtobufEntityType {
  Message(Vec<ProtobufField>),
  Enum(Vec<ProtobufEnumVariant>),
  Rpc,
}

// NOTE: ToProtobuf
pub(crate) struct ProtobufEntity {
  pub entity_type: ProtobufEntityType,
  pub name: String,
}

// NOTE: ToProtobuf
pub(crate) struct Parser<'a> {
  struct_re: Regex,
  enum_re: Regex,
  pub crate_name: &'a str,
  ignore_rpc: bool,
  panic_to_unsupported: bool,
  verbose: bool,
  types_parser: TypesParser,
  pub types: BTreeMap<String, ProtobufEntity>,
}

impl<'a> Parser<'a> {
  pub(crate) fn new(
    crate_name: &'a str,
    ignore_rpc: bool,
    panic_to_unsupported: bool,
    verbose: bool,
  ) -> MResult<Self> {
    Ok(
      Self {
        struct_re: Regex::new(r##"(// NOTE: ToProtobuf[a-z\n() ]*struct ([a-zA-Z0-9_]*)[ ]?\{([\w\n\s():<>,/'"\-_=#\[\]]*)})|(// NOTE: ToProtobuf[a-z\n() ]*struct ([a-zA-Z0-9_]*)[ ]?*\(([a-zA-Z0-9,<>:_ \n]*)\);)"##)
          .map_err(|e| R2Proto3Error::new(Some(Box::new(e)), "Не удалось собрать регулярное выражение для структур данных"))?,
        enum_re: Regex::new(r##"// NOTE: ToProtobuf[a-z\n() ]*enum ([a-zA-Z0-9_]*)[ ]?\{([\w\n\s():<>'",/\-_=#\[\]]*)}"##)
          .map_err(|e| R2Proto3Error::new(Some(Box::new(e)), "Не удалось собрать регулярное выражение для перечислений"))?,
        crate_name,
        ignore_rpc,
        panic_to_unsupported,
        verbose,
        types_parser: TypesParser::new()?,
        types: BTreeMap::default(),
      }
    )
  }

  pub(crate) fn parse(&mut self) -> MResult<()> {
    let mut messages = vec![];
    let mut enums = vec![];
    let mut known_types = BTreeSet::new();

    for entry in WalkDir::new(&self.crate_name).follow_links(true) {
      if let Ok(entry) = entry {
        if entry.file_type().is_file() && entry.file_name().as_encoded_bytes().ends_with(b"rs") {
          let mut f = File::open(entry.path()).map_err(|e| R2Proto3Error::new(Some(Box::new(e)), "Не удалось открыть файл"))?;
          let mut contents = String::new();
          f.read_to_string(&mut contents).map_err(|e| R2Proto3Error::new(Some(Box::new(e)), "Не удалось считать содержимое файла"))?;
          
          // Парсим структуры
          for (_, [_, struct_name, all_fields]) in self.struct_re.captures_iter(&contents).map(|c| c.extract()) {
            let fields = all_fields
              .split("\n")
              .map(|p| p.trim())
              .filter(|p| !p.is_empty() && !p.starts_with('#') && !p.starts_with('/'))
              .map(|s| s.to_owned())
              .collect::<Vec<String>>();
            messages.push((struct_name.to_string(), fields));
            if !known_types.insert(struct_name.to_string()) {
              println!(r#"Dublicate type: "{}""#, struct_name);
            };
          }
          
          // Парсим перечисления
          for (_, [enum_name, all_variants]) in self.enum_re.captures_iter(&contents).map(|c| c.extract()) {
            let variants = all_variants
              .split("\n")
              .map(|p| p.trim())
              .filter(|p| !p.is_empty() && !p.starts_with('#') && !p.starts_with('/'))
              .map(|s| s.to_owned())
              .collect::<Vec<_>>();
            enums.push((enum_name.to_string(), variants));
            if !known_types.insert(enum_name.to_string()) {
              println!(r#"Dublicate type: "{}""#, enum_name);
            };
          }
        }
      }
    }

    if known_types.is_empty() {
      println!("There are no data types to translate in the crate. Maybe you forgot to put a comment right before the start of the structure?");
      println!("You should write `// NOTE: ToProtobuf` right before struct/enum/function is declared.");
      
      return Ok(())
    } else if self.verbose {
      println!("Messages = {:#?}", messages);
      println!("Enums = {:#?}", enums);
      println!("Unique types: {:?}", known_types);
    }
    
    for message in messages {
      match self.parse_struct_fields(&message.1, &known_types) {
        Ok(fields) => {
          if self.verbose { println!("Parsed fields: {:?}", fields); }
          self.types.insert(message.0.to_owned(), ProtobufEntity {
            entity_type: ProtobufEntityType::Message(fields),
            name: message.0.to_owned(),
          });
        },
        Err(e) => {
          if self.panic_to_unsupported {
            return Err(R2Proto3Error::new(Some(Box::new(e)), format!("Warning: the struct `{}` won't be attached to `.proto` file", &message.0)));
          } else {
            println!("Warning: the struct `{}` won't be attached to `.proto` file due to error: {}", &message.0, e);
          }
        },
      }
    }
    
    for r#enum in enums {
      match self.parse_enum_fields(&r#enum.1) {
        Ok(variants) => {
          if self.verbose { println!("Parsed variants: {:?}", variants); }
          self.types.insert(r#enum.0.to_owned(), ProtobufEntity {
            entity_type: ProtobufEntityType::Enum(variants),
            name: r#enum.0.to_owned(),
          });
        },
        Err(e) => {
          if self.panic_to_unsupported {
            return Err(R2Proto3Error::new(Some(Box::new(e)), format!("Warning: the enum `{}` won't be attached to `.proto` file", &r#enum.0)));
          } else {
            println!("Warning: the enum `{}` won't be attached to `.proto` file due to error: {}", &r#enum.0, e);
          }
        },
      }
    }

    Ok(())
  }
  
  fn parse_struct_fields(&self, fields_str: &Vec<String>, known_types: &BTreeSet<String>) -> MResult<Vec<ProtobufField>> {
    let mut fields = vec![];
    let mut value_cntr = 1i32;
    
    for field in fields_str.iter() {
      let parts = field.split(':').map(|s| s.to_owned()).collect::<Vec<_>>();
      
      // В этот момент предполагается, что, раз длина поля структуры данных равна единице, то эта структура объявлена в скобках,
      // и её параметр анонимен.
      if parts.len() == 1 {
        let rust_type = TypesParser::drop_type_unnecessary_stuff(&parts[0]);
        fields.push(ProtobufField {
          name: format!("anonymous_value_{}", value_cntr),
          proto3_type: self.types_parser.rust_type_to_protobuf(&rust_type, known_types, false)?.to_owned(),
          rust_type,
          field_num: value_cntr,
        });
      }
      else if parts.len() >= 2 {
        let name = TypesParser::clear_type_name(parts[0].to_owned());
        let rust_type = TypesParser::drop_type_unnecessary_stuff(parts.iter().skip(1).map(|p| p.to_owned()).collect::<Vec<_>>().join(":"));
        fields.push(ProtobufField {
          name,
          proto3_type: self.types_parser.rust_type_to_protobuf(&rust_type, known_types, false)?.to_owned(),
          rust_type,
          field_num: value_cntr,
        });
      }
      value_cntr += 1;
      // See [Language Guide (proto 3) - Assigning Field Numbers](https://protobuf.dev/programming-guides/proto3/#assigning).
      if value_cntr == 19_000 {
        value_cntr = 20_000;
      } else if value_cntr == 536_870_912 {
        return Err(R2Proto3Error::new(None, "very big message! Max field number = 536_870_911"));
      }
    }
    
    Ok(fields)
  }
  
  fn parse_enum_fields(&self, variants_str: &Vec<String>) -> MResult<Vec<ProtobufEnumVariant>> {
    let mut variants = vec![];
    let mut variant_id_cntr = 0;
    
    for variant in variants_str.iter() {
      let variant = TypesParser::drop_type_unnecessary_stuff(variant);
      
      if variant.contains('(') {
        return Err(R2Proto3Error::new(None, format!("current version of `r2proto3` isn't supporting enums with values in them - in variant `{}`", variant)));
      }
      
      variants.push(ProtobufEnumVariant {
        name: variant.to_owned(),
        value: variant_id_cntr,
      });
      variant_id_cntr += 1;
    }
    
    Ok(variants)
  }
  
  pub(crate) fn generate(&self) -> String {
    let mut contents = r#"syntax = "proto3";"#.to_owned() + "\n";
    
    for (type_name, r#type) in &self.types {
      match &r#type.entity_type {
        ProtobufEntityType::Message(msg) => {
          contents += "\n";
          contents += &format!("message {} {{", type_name);
          for field in msg {
            contents += "\n";
            contents += &format!("  {} {} = {};", field.proto3_type, field.name, field.field_num);
          }
          contents += "\n}\n";
        },
        ProtobufEntityType::Enum(r#enum) => {
          contents += "\n";
          contents += &format!("enum {} {{", type_name);
          for variant in r#enum {
            contents += "\n";
            contents += &format!("  {} = {};", variant.name, variant.value);
          }
          contents += "\n}\n";
        },
        _ => unimplemented!(),
      }
    }
    
    contents
  }
}
