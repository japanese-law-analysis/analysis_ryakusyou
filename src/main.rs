use anyhow::{Context, Result};
use clap::Parser;
use jplaw_text::{ArticleTargetInfo, LawContents};
use listup_law::LawData;
use search_article_with_word::{self, Chapter};
use std::collections::HashMap;
use std::path::Path;
use tokio::{
  self,
  fs::*,
  io::{AsyncReadExt, AsyncWriteExt},
  process::Command,
};
use tokio_stream::StreamExt;
use tracing::*;

use analysis_ryakusyou::{JapaneseDependency, ParseRyakusyouInfo};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
  /// 解析結果を出力するJSONファイルへのpath
  #[clap(short, long)]
  output: String,
  /// `parse_japanese_dependency.py`へのpath
  #[clap(short, long)]
  py_path: String,
  /// 一次生成ファイルを出力するディレクトリ
  #[clap(short, long, default_value_t=String::from("."))]
  tmp_directory: String,
  /// 法令XMLファイル群が置かれている作業ディレクトリへのpath
  #[clap(short, long)]
  work: String,
  /// 法令ファイルのインデックス情報が書かれたJSONファイルへのpath
  #[clap(short, long)]
  index_file: String,
  /// 解析する対象の条文のインデックスが書かれたJSONファイルへのpath
  #[clap(short, long)]
  article_info_file: String,
  /// 条項データのキャッシュを使用しないで再度生成しなおす場合に付けるフラグ
  #[clap(short, long, default_value_t = false)]
  do_not_use_cache: bool,
}

async fn init_logger() -> Result<()> {
  let subscriber = tracing_subscriber::fmt()
    .with_max_level(tracing::Level::INFO)
    .finish();
  tracing::subscriber::set_global_default(subscriber)?;
  Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
  let args = Args::parse();

  init_logger().await?;

  let tmp_directory = &args.tmp_directory;
  let info_tmp_path = format!("{tmp_directory}/analysis_ryakusyou_tmp_info.json");
  let input_tmp_path = format!("{tmp_directory}/analysis_ryakusyou_tmp_input.json");
  let japanese_dependency_tmp_file = format!("{tmp_directory}/analysis_ryakusyou_tmp_output.json");

  let is_generate_tmp_file = if args.do_not_use_cache {
    // キャッシュを使わない
    // 常に実行
    true
  } else {
    // キャッシュを使う
    // 一時ファイルが一つでも存在しなければ実行
    !Path::new(&info_tmp_path).exists() || !Path::new(&japanese_dependency_tmp_file).exists()
  };

  let mut law_text_info = HashMap::new();

  if is_generate_tmp_file {
    info!("[START] get law data: {:?}", &args.index_file);
    let raw_data_lst = listup_law::get_law_from_index(&args.index_file).await?;
    info!("[END] get law data: {:?}", &args.index_file);
    let mut raw_data_stream = tokio_stream::iter(raw_data_lst);
    let mut file_index: HashMap<String, LawData> = HashMap::new();
    while let Some(law_data) = raw_data_stream.next().await {
      file_index.insert(law_data.num.clone(), law_data);
    }

    info!("[START] get article info: {:?}", &args.article_info_file);
    let law_paragraph_lst =
      search_article_with_word::get_law_from_artcile_info(&args.article_info_file).await?;
    info!("[END] get article info: {:?}", &args.article_info_file);

    let mut law_paragraph_stream = tokio_stream::iter(law_paragraph_lst);

    let work_dir_path = Path::new(&args.work);
    let mut law_text_id = HashMap::new();
    let mut counter: usize = 0;
    while let Some(law_paragraph) = law_paragraph_stream.next().await {
      let num = law_paragraph.num;
      let file_name = &file_index
        .get(&num)
        .with_context(|| format!("not found file name with law num: {num}"))?
        .file;
      let file_path = work_dir_path.join(file_name);
      info!("[START] work file: {:?}", file_path);
      let chapter_lst = law_paragraph.chapter_data;
      let mut chapter_stream = tokio_stream::iter(chapter_lst);
      while let Some(chapter) = chapter_stream.next().await {
        info!("[DATA] chapter; {num}:{chapter:?}");
        let mut xml_file = File::open(&file_path).await?;
        let mut xml_text = Vec::new();
        xml_file.read_to_end(&mut xml_text).await?;
        let target = target_info_from_chapter_lst(&chapter).await;
        let law_text_lst = jplaw_text::search_law_text(&xml_text, &target).await?;
        let mut law_text_stream = tokio_stream::iter(law_text_lst).filter(|c| !c.is_child);
        while let Some(law_text) = law_text_stream.next().await {
          if let LawContents::Text(text) = law_text.contents {
            let mut lst = tokio_stream::iter(analysis_ryakusyou::remove_paren(&text));
            while let Some(text_info) = lst.next().await {
              counter += 1;
              law_text_id.insert(counter, text_info.clone().remove_paren_text);
              let tmp_info = ParseRyakusyouInfo {
                raw_text: text_info.raw_text.clone(),
                remove_paren_text: text_info.remove_paren_text.clone(),
                paren: text_info.paren.clone(),
                num: num.to_string(),
                chapter: chapter.clone(),
              };
              law_text_info.insert(counter, tmp_info);
            }
          }
        }
      }
      info!("[END] work file: {:?}", file_path);
    }

    let info_tmp_str = serde_json::to_string(&law_text_info)?;
    let mut info_tmp_file = File::create(&info_tmp_path).await?;
    info_tmp_file.write_all(info_tmp_str.as_bytes()).await?;
    info_tmp_file.flush().await?;
    let input_tmp_str = serde_json::to_string(&law_text_id)?;
    let mut input_tmp_file = File::create(&input_tmp_path).await?;
    input_tmp_file.write_all(input_tmp_str.as_bytes()).await?;
    input_tmp_file.flush().await?;

    info!("[START] run parse_japanese_dependency.py");
    Command::new("python")
      .arg(&args.py_path)
      .arg("--input")
      .arg(input_tmp_path)
      .arg("--output")
      .arg(&japanese_dependency_tmp_file)
      .output()
      .await?;
    info!("[END] run parse_japanese_dependency.py");
  } else {
    // 既に生成されているファイルの中身を抽出する
    info!("[START] read info tmp file: {}", &info_tmp_path);
    let mut info_tmp_file = File::open(&info_tmp_path).await?;
    let mut info_tmp_buf = Vec::new();
    info_tmp_file.read_to_end(&mut info_tmp_buf).await?;
    let info_tmp_json_str = String::from_utf8(info_tmp_buf)?;
    law_text_info = serde_json::from_str(&info_tmp_json_str)?;
    info!("[END] read info tmp file: {}", &info_tmp_path);
  }

  let mut japanese_dependency_file = File::open(&japanese_dependency_tmp_file).await?;
  let mut japanese_dependency_buf = Vec::new();
  japanese_dependency_file
    .read_to_end(&mut japanese_dependency_buf)
    .await?;
  let japanese_dependency_json_str = String::from_utf8(japanese_dependency_buf)?;
  let japanese_dependency_data: HashMap<usize, HashMap<usize, JapaneseDependency>> =
    serde_json::from_str(&japanese_dependency_json_str)?;
  let mut japanese_dependency_stream = tokio_stream::iter(japanese_dependency_data);

  let mut output = File::create(&args.output).await?;
  output.write_all(b"[").await?;
  let mut is_head = true;
  while let Some((key, japanese_dependency_lst)) = japanese_dependency_stream.next().await {
    let parse_ryakusyou_info = law_text_info.get(&key).unwrap();
    let ryakusyou_info =
      analysis_ryakusyou::find_ryakusyou(&japanese_dependency_lst, parse_ryakusyou_info).await;
    if !ryakusyou_info.ryakusyou_lst.is_empty() {
      let ryakusyou_info_str = serde_json::to_string(&ryakusyou_info)?;
      if is_head {
        output.write_all(b"\n").await?;
        is_head = false;
      } else {
        output.write_all(b",\n").await?;
      }
      output.write_all(ryakusyou_info_str.as_bytes()).await?;
    }
  }

  output.write_all(b"\n]").await?;

  Ok(())
}

async fn target_info_from_chapter_lst(chapter: &Chapter) -> ArticleTargetInfo {
  ArticleTargetInfo {
    article: chapter.article.clone(),
    paragraph: chapter.paragraph.clone(),
    item: chapter.item.clone(),
    sub_item: chapter.sub_item.clone(),
    suppl_provision_title: chapter.suppl_provision_title.clone(),
  }
}
