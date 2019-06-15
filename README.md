# actix cors

Proxies GET-requests.

## usage
```sh
cargo run --release
```

then fetch https://httpbin.org/get:
```sh
curl -v localhost:8080/https://httpbin.org/get
```

## development
```sh
cargo install cargo-watch
cargo watch -x run
```
