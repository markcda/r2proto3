use regex::Regex;
use std::collections::BTreeSet;

use crate::utils::{MResult, R2Proto3Error};

pub(crate) struct TypesParser {
  inner_vec_type_re: Regex,
  inner_option_type_re: Regex,
  inner_map_type_re: Regex,
}

impl TypesParser {
  pub(crate) fn new() -> MResult<Self> {
    Ok(Self {
      inner_vec_type_re: Regex::new(r#"Vec<([a-zA-Z0-9<>()\[\],:_ ]*)>"#)
        .map_err(|e| R2Proto3Error::new(Some(Box::new(e)), "Не удалось собрать регулярное выражение для внутренних типов данных вектора"))?,
      inner_option_type_re: Regex::new(r#"Option<([a-zA-Z0-9<>()\[\],:_ ]*)>"#)
        .map_err(|e| R2Proto3Error::new(Some(Box::new(e)), "Не удалось собрать регулярное выражение для внутренних типов данных опционального типа"))?,
      inner_map_type_re: Regex::new(r#"(HashMap<([a-zA-Z0-9<>()\[\],:_ ]*)>)|(BTreeMap<([a-zA-Z0-9<>()\[\],:_ ]*)>)"#)
        .map_err(|e| R2Proto3Error::new(Some(Box::new(e)), "Не удалось собрать регулярное выражение для внутренних типов данных словаря"))?,
    })
  }
  
  pub(crate) fn rust_type_to_protobuf<'a>(
    &self,
    rust_type: &'a str,
    known_types: &BTreeSet<String>,
    for_map_key: bool,
  ) -> MResult<String> {
    let unsupported_key_msg = if for_map_key {
      Some(format!("this key type is not supported by `proto3`: {}", rust_type))
    } else {
      None
    };
    
    match rust_type {
      "f64"                => if !for_map_key { Ok("double".into()) } else { Err(R2Proto3Error::new(None, unsupported_key_msg.unwrap())) },
      "f32" | "f16" | "f8" => if !for_map_key { Ok("float".into()) } else { Err(R2Proto3Error::new(None, unsupported_key_msg.unwrap())) },
      "i64"                => Ok("int64".into()),
      "i32" | "i16" | "i8" => Ok("int32".into()),
      "u64"                => Ok("uint64".into()),
      "u32" | "u16" | "u8" => Ok("uint32".into()),
      "bool"               => Ok("bool".into()),
      "String"             => Ok("string".into()),
      "Vec<u8>"            => if !for_map_key { Ok("bytes".into()) } else { Err(R2Proto3Error::new(None, unsupported_key_msg.unwrap())) },
      _ => {
        if let Some((_, [inner])) = self.inner_vec_type_re.captures_iter(rust_type).map(|c| c.extract()).next() {
          let inner_type = self.rust_type_to_protobuf(inner, known_types, false)?;
          if inner_type.starts_with("repeated") {
            Err(R2Proto3Error::new(None, format!("need to use `repeated` twice: consider not to use Vec<Vec<_>> etc.")))
          } else {
            Ok(format!("repeated {}", inner_type))
          }
        }
        else if let Some((_, [inner])) = self.inner_option_type_re.captures_iter(rust_type).map(|c| c.extract()).next() {
          let inner_type = self.rust_type_to_protobuf(inner, known_types, false)?;
          if inner_type.starts_with("optional") {
            Err(R2Proto3Error::new(None, format!("need to use `optional` twice: consider not to use Option<Option<_>> etc.")))
          } else {
            Ok(format!("optional {}", inner_type))
          }
        }
        else if let Some((_, [_, inner])) = self.inner_map_type_re.captures_iter(rust_type).map(|c| c.extract()).next() {
          let inners = TypesParser::split_inner_types(inner)?.iter().map(|i| TypesParser::drop_type_unnecessary_stuff(i)).collect::<Vec<_>>();
          if inners.len() != 2 {
            return Err(R2Proto3Error::new(None, format!("there is only one or more than 2 inner types of `HashMap`/`BTreeMap`")))
          }
          let (key_type, value_type) = (&inners[0], &inners[1]);
          
          let inner_key_type = self.rust_type_to_protobuf(key_type, known_types, true)?;
          let inner_value_type = self.rust_type_to_protobuf(value_type, known_types, false)?;
          
          Ok(format!("map<{}, {}>", inner_key_type, inner_value_type))
        }
        
        else if known_types.contains(rust_type) { Ok(rust_type.into()) }
        else { Err(R2Proto3Error::new(None, format!("unknown type - `{}`", rust_type))) }
      },
    }
  }
  
  pub(crate) fn drop_type_unnecessary_stuff(rust_type: impl AsRef<str>) -> String {
    let mut rust_type = rust_type.as_ref().trim().to_owned();
    if let Some(pos) = rust_type.find("//") {
      rust_type.truncate(pos);
      rust_type = rust_type.trim().to_owned();
    }
    if rust_type.ends_with(',') {
      rust_type.truncate(rust_type.len() - 1);
    }
    rust_type
  }
  
  pub(crate) fn clear_type_name(name: impl AsRef<str>) -> String {
    name.as_ref().replace("pub ", "").replace("pub(crate) ", "").replace("pub(super) ", "")
  }
  
  fn split_inner_types(inner: &str) -> MResult<Vec<&str>> {
    use std::collections::VecDeque;
    const OPENERS: [char; 3] = ['<', '(', '['];
    const CLOSERS: [char; 3] = ['>', ')', ']'];
    
    let mut stack = VecDeque::new();
    let mut types = Vec::new();
    let mut begin_index = 0usize;
    
    for (i, sym) in inner.chars().enumerate() {
      if let Some(pos) = OPENERS.iter().position(|c| *c == sym) {
        stack.push_back(OPENERS[pos]);
      }
      else if let Some(pos) = CLOSERS.iter().position(|c| *c == sym) {
        if stack.pop_back().is_none_or(|o| OPENERS.iter().position(|c| *c == o).unwrap() != pos) {
          return Err(R2Proto3Error::new(None, format!("can't parse inner types due to invalid types' openers and closers (`<([` and `>)]` stack")));
        }
      }
      else if sym == ',' && stack.is_empty() && begin_index + 1 < i {
        types.push(&inner[begin_index..i]);
        begin_index = i + 1;
      }
    }
    types.push(&inner[begin_index..]);
    
    Ok(types)
  }
}

#[cfg(test)]
mod types_parser_tests {
  use super::*;
  
  #[test]
  fn test_dropping_unnecessary_stuff() {
    assert_eq!(TypesParser::drop_type_unnecessary_stuff(" String,").as_str(), "String");
    assert_eq!(TypesParser::drop_type_unnecessary_stuff("HashMap<String, u32>").as_str(), "HashMap<String, u32>");
    assert_eq!(TypesParser::drop_type_unnecessary_stuff("HashMap<String, u32>, // this is an example").as_str(), "HashMap<String, u32>");
  }
  
  #[test]
  fn split_inner_types_test() {
    assert_eq!(TypesParser::split_inner_types(&"HashMap<String, i32>"[8..19]), Ok(vec!["String", " i32"]));
    
    let inner_map_type_re = Regex::new("(HashMap<([a-zA-Z0-9<>,:_ ]*)>)|(BTreeMap<([a-zA-Z0-9<>,:_ ]*)>)").unwrap();
    assert_eq!(inner_map_type_re.captures_iter("HashMap<String, u32>").map(|c| c.extract()).next(), Some(("HashMap<String, u32>", ["HashMap<String, u32>", "String, u32"])));
    
    assert_eq!(TypesParser::split_inner_types(inner_map_type_re.captures_iter("HashMap<String, u32>").map(|c| c.extract::<2>()).next().unwrap().1[1]), Ok(vec!["String", " u32"]));
  }
}
