# ClipKeeper

剪贴板历史工具，支持 Windows 和 macOS。自动保存复制过的文本和图片，用快捷键呼出窗口查看和再次粘贴。

## 使用

| 操作 | Windows | macOS |
| --- | --- | --- |
| 打开 / 关闭历史窗口 | `Ctrl+Shift+V` | `⌘⇧V` |
| 选择 | `↑` `↓` | `↑` `↓` |
| 复制选中项并关闭窗口 | `Enter` | `Enter` |
| 直接复制第 1–9 条 | `Ctrl+1`…`Ctrl+9` | `⌘1`…`⌘9` |
| 置顶 / 取消置顶 | `Ctrl+P` | `⌘P` |
| 删除 | `Ctrl+Backspace` | `⌘⌫` |
| 切换分类 | `Tab` | `Tab` |
| 清除搜索 / 关闭窗口 | `Esc` | `Esc` |

复制后窗口会自动关闭，回到原来的应用按 `Ctrl+V` / `⌘V` 粘贴即可。点击窗口外的地方也会关闭窗口。

应用常驻在系统托盘（macOS 菜单栏）。托盘菜单里可以打开历史、开启或关闭开机自启、退出应用。

- 最多保留 500 条未置顶记录，置顶的不会被自动删除
- 同样的内容再次复制只会移到最前面，不会重复保存
- 密码管理器（1Password、KeePass 等）标记为“不要记录”的内容会被跳过
- 所有数据只保存在本机：Windows 在 `%APPDATA%\com.wilber.clipkeeper`，macOS 在 `~/Library/Application Support/com.wilber.clipkeeper`

### macOS 首次打开

安装包没有经过 Apple 公证，第一次打开时系统会拦截。在“访达”里右键点击 ClipKeeper，选择“打开”，再确认一次即可。如果提示“已损坏”，在终端运行：

```bash
xattr -cr /Applications/ClipKeeper.app
```

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

推送到 GitHub 后，`.github/workflows/build.yml` 会自动在 Windows 和 macOS 上打包，安装包在该次运行的 Artifacts 里下载。
