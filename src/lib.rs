use regex::Regex;
use search_article_with_word::Chapter;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio_stream::StreamExt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RyakusyouInfo {
  /// 法律番号
  pub num: String,
  /// その略称規定がある条項
  pub chapter: Chapter,
  /// 略称規定のリスト
  pub ryakusyou_lst: Vec<Ryakusyou>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Ryakusyou {
  /// 略称
  ryakusyou: String,
  /// 正式名称
  seishiki: String,
}

/// parse_japanese_dependencyが生成する情報
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct JapaneseDependency {
  start: usize,
  end: usize,
  head_start: Option<usize>,
  head_end: Option<usize>,
  text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TextInfo {
  pub raw_text: String,
  pub remove_paren_text: String,
  pub paren: Vec<(usize, String)>,
}

/// 一時ファイル
#[derive(Debug, Clone)]
pub struct ParseRyakusyouInfo {
  pub raw_text: String,
  pub remove_paren_text: String,
  pub paren: Vec<(usize, String)>,
  pub num: String,
  pub chapter: Chapter,
}

/// 文から括弧書きを取り除いたテキストと、その括弧書きがどの位置に挿入されていたのかのリスト
/// 括弧書きの中に括弧書き
pub fn remove_paren(text: &str) -> Vec<TextInfo> {
  let re = Regex::new("(.+「.+」という。.*)|(.+をいう。.*)").unwrap();
  let chars = text.chars();
  let mut counter = 0;
  let mut paren_depth = 0;
  let mut paren_stack = String::new();
  let mut s = String::new();
  let mut p_lst = Vec::new();
  let mut v = Vec::new();
  for c in chars {
    match c {
      '（' => {
        if paren_depth == 0 {
        } else {
          paren_stack.push(c);
        }
        paren_depth += 1;
      }
      '）' => {
        paren_depth -= 1;
        if paren_depth == 0 {
          let mut t = remove_paren(&paren_stack)
            .iter()
            .filter(|t| (t.raw_text != t.remove_paren_text) && re.is_match(&paren_stack))
            .cloned()
            .collect::<Vec<_>>();
          v.append(&mut t);
          p_lst.push((counter, paren_stack));
          paren_stack = String::new();
        } else {
          paren_stack.push(c);
        }
      }
      _ => {
        if paren_depth == 0 {
          counter += 1;
          s.push(c);
        } else {
          paren_stack.push(c)
        }
      }
    }
  }

  let p_lst = p_lst
    .iter()
    .filter(|(_, s)| re.is_match(s))
    .cloned()
    .collect::<Vec<_>>();
  if (text != s) && !p_lst.is_empty() {
    v.push(TextInfo {
      raw_text: text.to_string(),
      remove_paren_text: s,
      paren: p_lst,
    });
  }

  v
}

pub async fn find_ryakusyou(
  japanese_dependency_lst: &HashMap<usize, JapaneseDependency>,
  parse_ryakusyou_info: &ParseRyakusyouInfo,
) -> RyakusyouInfo {
  let mut v = Vec::new();
  let mut ryakusyo_pos_stream = tokio_stream::iter(&parse_ryakusyou_info.paren);
  let re =
    Regex::new("([^「]+「(?P<ryakusyou>[^」]+)」という。.*)|((?P<seishiki>.+)をいう。.*)").unwrap();

  while let Some((pos, text)) = ryakusyo_pos_stream.next().await {
    println!("{text} pos: {pos}");
    if let Some((target_start_token_index, _)) = japanese_dependency_lst
      .iter()
      .find(|(_, d)| d.start < *pos && pos - 1 <= d.end)
    {
      let mut index_lst = vec![*target_start_token_index];
      println!("{text} target_start_token_index: {target_start_token_index}");
      let mut tmp_index_lst = Vec::new();
      loop {
        for i in index_lst.clone().iter() {
          let mut l = japanese_dependency_lst
            .iter()
            .filter(|(_, d)| match (d.head_start, d.head_end) {
              (Some(s), Some(e)) => s <= *i && *i < e,
              _ => false,
            })
            .map(|(i, _)| *i)
            .collect::<Vec<_>>();
          tmp_index_lst.append(&mut l);
        }
        let old_len = index_lst.len();
        index_lst.append(&mut tmp_index_lst);
        index_lst.sort();
        index_lst.dedup();
        if old_len == index_lst.len() {
          // 変化が無くなったため終了
          break;
        }
        tmp_index_lst = Vec::new();
      }
      println!("{text} index_lst: {index_lst:?}");
      index_lst.sort();
      if let Some(head) = index_lst.first() {
        if let Some(head_token) = japanese_dependency_lst.get(head) {
          let start = head_token.start;
          println!("{text} start: {start}, pos: {pos}");
          let mut t = String::new();
          for (i, c) in parse_ryakusyou_info.remove_paren_text.chars().enumerate() {
            if start <= i && i < *pos {
              t.push(c)
            }
          }
          if let Some(ryakusyou_or_seishiki) = re.captures(text) {
            if let Some(ryakusyou) = ryakusyou_or_seishiki.name("ryakusyou") {
              let ryakusyou = ryakusyou.as_str().to_string();
              let ryakusyou_v = Ryakusyou {
                ryakusyou,
                seishiki: t,
              };
              v.push(ryakusyou_v)
            } else if let Some(seishiki) = ryakusyou_or_seishiki.name("seishiki") {
              let seishiki = seishiki.as_str().to_string();
              let ryakusyou_v = Ryakusyou {
                ryakusyou: t,
                seishiki,
              };
              v.push(ryakusyou_v)
            }
          }
        }
      }
    }
  }

  RyakusyouInfo {
    num: parse_ryakusyou_info.num.clone(),
    chapter: parse_ryakusyou_info.chapter.clone(),
    ryakusyou_lst: v,
  }
}

#[test]
fn check_remove_paren_1() {
  let s1 = "テスト（以下単に「ほげ」という。）テスト";
  let v = vec![TextInfo {
    raw_text: s1.to_string(),
    remove_paren_text: "テストテスト".to_string(),
    paren: vec![(3, "以下単に「ほげ」という。".to_string())],
  }];
  assert_eq!(v, remove_paren(s1))
}

#[test]
fn check_remove_paren_2() {
  let s1 = "テスト（以下単に「ほげ」という。（ふがほげぴよとなっているものをいう。）テスト）テスト";
  let v = vec![
    TextInfo {
      raw_text: "以下単に「ほげ」という。（ふがほげぴよとなっているものをいう。）テスト"
        .to_string(),
      remove_paren_text: "以下単に「ほげ」という。テスト".to_string(),
      paren: vec![(12, "ふがほげぴよとなっているものをいう。".to_string())],
    },
    TextInfo {
      raw_text: s1.to_string(),
      remove_paren_text: "テストテスト".to_string(),
      paren: vec![(
        3,
        "以下単に「ほげ」という。（ふがほげぴよとなっているものをいう。）テスト".to_string(),
      )],
    },
  ];
  assert_eq!(v, remove_paren(s1))
}

#[test]
fn check_remove_paren_3() {
  let s1 = "テスト（テスト（テスト）テスト）テスト";
  assert_eq!(remove_paren(s1), Vec::new())
}
