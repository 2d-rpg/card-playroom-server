# card-playroom-server
![Docker](https://img.shields.io/static/v1?label=Docker&message=v19.3.13&color=2496ED&logo=docker)
![Rust](https://img.shields.io/static/v1?label=Rust&message=v1.47.0&color=B7410E&logo=rust)

[![Release](https://img.shields.io/github/v/release/2d-rpg/card-playroom-server?include_prereleases)](https://github.com/2d-rpg/card-playroom-server/releases)
[![Rust](https://github.com/2d-rpg/card-playroom-server/workflows/Rust/badge.svg)](https://github.com/2d-rpg/card-playroom-server/actions?query=workflow%3ARust)
## <img src="https://user-images.githubusercontent.com/42469701/95276781-1b815500-0887-11eb-84e5-f1dc89df3efb.png" width="20px"> How to get started

VSCode の[Remote Container 拡張](https://code.visualstudio.com/docs/remote/containers)の使用を推奨します．  
このプロジェクトをクローンし，VSCode の[Remote Container 拡張](https://code.visualstudio.com/docs/remote/containers)を使用して開く．

```
git clone https://github.com/2d-rpg/card-playroom-server.git
code card-playroom-server
```

以下 Docker コンテナ上で実行

`.env.example`ファイルを`.env`にコピーする

```bash
cat .env.example > .env # cp .env.example .env も可
```

diesel_cli のインストール
```bash
cargo install diesel_cli --no-default-features --features postgres
```

`diesel`のセットアップを行う．

```bash
diesel setup
diesel migration generate create_rooms
```

`up.sql`に以下を追記．

```sql
CREATE TABLE rooms (
  id   SERIAL  PRIMARY KEY,
  name VARCHAR NOT NULL,
  players TEXT[] NOT NULL
);
```

データベース起動

```bash
diesel migration run
```

サーバー起動

```bash
cargo run # local-container間の同期が早い場合
cargo run --target-dir /tmp/target # local-container間の同期が遅い場合
```

http://0.0.0.0:8080 にサーバーが建てられる．
http://127.0.0.1:8080 にアクセス．

データ追加

- createRoom(ルーム作成)

```graphql
mutation {
  createRoom(name: "hoge", player: "fuga") {
    id, name, players
  }
}
```

- enterRoom(ルーム参加)

```graphql
mutation {
  enterRoom(player: "piyo", roomId: 1) {
    id, name, players
  }
}
```

- removeRoom(ルーム削除)

```graphql
mutation {
  removeRoom(roomId: 2) {
    id, name, players
  }
}
```

データ取得

```graphql
query {
  rooms{id, name, players}
}
```

## データベースの変更と確認方法

`up.sql`を変更した場合，以下のコマンドでデータベースの更新を行う．

```bash
diesel database reset
```

上記の変更後，`schema.rs`の変更が自動で行われ，以下のコマンドでデータベースのスキーマを確認できる．

```bash
diesel print-schema
```
