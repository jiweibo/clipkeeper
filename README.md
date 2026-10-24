# ClipKeeper

剪贴板历史工具，支持 Windows 和 macOS。自动保存复制过的文本和图片，用快捷键呼出窗口查看和再次粘贴。

## 技术栈

- Tauri 2（Rust 后端）+ React + TypeScript（界面）
- SQLite 存储历史记录，图片以 PNG 文件保存在应用数据目录的 `images/` 下

## 开发

```bash
pnpm install
pnpm tauri dev                                        # 本地运行
cargo test --manifest-path src-tauri/Cargo.toml --lib # 运行测试
pnpm tauri build                                      # 打包安装包
```

推送到 GitHub 后，`.github/workflows/build.yml` 会自动在 Windows 和 macOS 上打包，并把安装包作为构建产物上传。
