[![Workflow Status](https://github.com/japanese-law-analysis/analysis_ryakusyou/workflows/Rust%20CI/badge.svg)](https://github.com/japanese-law-analysis/analysis_ryakusyou/actions?query=workflow%3A%22Rust%2BCI%22)

# analysis_ryakusyou

## analysis_ryakusyou

略称・定義規定の「～という。」・「～をいう。」文の解析をして、その中身を取り出すソフトウェア

## install

```sh
cargo install --git "https://github.com/japanese-law-analysis/analysis_ryakusyou.git"
```

### その他の依存

動かすためには日本語の係り受け解析を行うための[japanese-law-analysis/parse_japanese_dependency](https://github.com/japanese-law-analysis/parse_japanese_dependency)というソフトウェアが必要です。手元などにcloneをし、このPythonスクリプトを動かすために必要なライブラリ・Python実行環境を整えてください。

## Use

```sh
 analysis_ryakusyou --output output.json --py-path "path/to/parse_japanese_dependency.py" --work "path/to/law_xml_directory" --index-file "path/to/law_list.json" --article-info-file "path/to/words_law_info.json"
```

で起動します。また、オプション引数として`--tmp-directory path/to/tmp_directory`が使えます。

それぞれのオプションの意味は以下の通りです。

- `--output`：略称・定義規定から抜き出した単語などのリストを出力するJSONファイル名
- `--py-path`：[japanese-law-analysis/parse_japanese_dependency](https://github.com/japanese-law-analysis/parse_japanese_dependency)の`parse_japanese_dependency.py`ファイルへのpath
- `--work`：[e-gov法令検索](https://elaws.e-gov.go.jp/)からダウンロードした全ファイルが入っているフォルダへのpath
- `--index-file`：[japanese-law-analysis/listup_law](https://github.com/japanese-law-analysis/listup_law)で生成した法令のリストが書かれているJSONファイルへのpath
- `--article-info-file`：[japanese-law-analysis/search_article_with_word](https://github.com/japanese-law-analysis/search_article_with_word)で生成した「という。」・「をいう。」が含まれる条文のリストが書かれたJSONファイルへのpath
- `--tmp-directory`（オプション引数）：一時ファイルの生成先フォルダへのpath（デフォルトは`.`）

---
[MIT License](https://github.com/japanese-law-analysis/analysis_ryakusyou/blob/master/LICENSE)
(c) 2023 Naoki Kaneko (a.k.a. "puripuri2100")


License: MIT
