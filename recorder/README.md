# cast-recorder

ssh-ops 用的 PTY 中间人 + 录像工具，基于 [asciinema](https://github.com/asciinema/asciinema) fork。

## 编译

```bash
cd /tmp
git clone --depth 1 https://github.com/asciinema/asciinema.git
cd asciinema
git apply /Users/bjarne/Code/ssh-ops/recorder/patches/01-immediate-flush-and-unix-socket.patch
cargo build --release
cp target/release/asciinema /Users/bjarne/Code/ssh-ops/bin/cast-recorder
```

## 修改点（vs upstream asciinema）

### Patch 01: immediate flush + unix socket

**1. file_writer.rs** — 每个 cast event 后立即 `writer.flush()`
- 上游 asciinema 用 buffered IO，cast 文件可能滞后 1-3 秒
- 改后 ssh-ops 能实时读到刚记录的输出

**2. unix_socket_output.rs** (新增) — PTY output 实时广播到 unix socket
- 每个 cast 文件旁建 `xxx.cast.sock`
- ssh-ops 直连 socket 读 raw PTY bytes，跳过 cast flush 等待 + jq 解析
- 用 std::thread + nonblocking IO（不依赖 tokio runtime），main 退出时 daemon thread 自动清理

**3. session.rs** — 自动启用 socket
- 任何 `--output-file` 模式都会同时建 `<file>.sock`

## 协议

socket 内容: 直接 raw PTY bytes（含 ANSI 转义），客户端自己处理。
连接: 任何 unix domain socket client（nc -U / socat / 自写）。

## ssh-ops 集成

`lib/recorder.sh` 的 `record_wait_prompt_in_cast` 和 `record_extract_output_from_byte` 仍在用 cast 文件方式。
未来可改为读 socket 实时 stream，预期再砍 ~400ms 延迟。
