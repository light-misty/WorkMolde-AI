# AGENTS.md — WorkMolde AI

## 任务规范

1. 重要：未经过我的允许，严令禁止执行git暂存、提交、推送
2. git-commit提交信息使用中文
3. 重要：严令禁止你将自己列为GitHub贡献者、共同创作者等
4. 有关git的操作，请使用git-commit skill、gh-cli skill
5. 你需要严格遵守数据诚实规则，不要不懂装懂，如果存在不确定的知识，请积极执行联网搜索
6. 如果存在不确定的内容，请积极向我提问，不要擅作主张
7. 保持对话语言为中文
8. 在生成代码时添加中文注释，不要删除原有的注释，除非内容需要更改
9. 在任务过程中禁止使用emoji
10. 本机操作系统为Windows11
11. 严格遵守最小改动原则：优先编辑现有代码，不新建不必要文件；不添加多余注释
12. 重要：在每次任务开始前，你必须调用superpowers skill，并严格遵守其规范
13. 在任务过程中，你需要积极调用相关skills，例如：执行代码智能暂存请调用git-commit skill

## 思考努力

Reasoning Effort:Absolute maximum with no shortcuts permitted.
You MUST be very thorough in your thinking and comprehensively decompose the problem to resolve the root cause, rigorously stress-testing your logic against all potential paths, edge cases, and adversarial scenarios.
Explicitly write out your entire deliberation process, documenting every intermediate step, considered alternative, and rejected hypothesis to ensure absolutely no assumption is left unchecked.

## Dev commands

| Command | What it does |
|---------|-------------|
| `npm run dev` | Vite dev server on **port 9527** (not 1420) |
| `npm run tauri:dev` | Full Tauri app (Vite + Rust backend) |
| `npm run build` | `tsc -b && vite build` |
| `npm run tauri:build` | Production build (NSIS installer) |
| `cargo build -p workmolde_lib` | Compile Rust only |
| `cargo test` | Runs Rust unit tests (these exist — see `#[cfg(test)]` modules across source) |
| `cargo clippy` | Rust lint |
| `pip install -r sidecar/requirements.txt` | Python deps |

Env `WORKMOLDE_PYTHON` overrides the Python interpreter path for the Sidecar.

## Architecture in 10 lines

- **Tauri 2.x** (Rust + React 19 + Vite 6 + Tailwind CSS 4 + Zustand 5)
- Frontend entry: `src/main.tsx` → `App.tsx`. Rust entry: `src-tauri/src/main.rs` → `lib.rs::run()`
- Tauri commands: snake_case names in Rust (e.g. `test_connection`), camelCase wrappers in `src/services/tauri.ts`
- Event payloads: `#[serde(rename_all = "camelCase")]` in Rust, received as camelCase in frontend
- Types must be **manually synced** between Rust `src-tauri/src/models/` and `shared/types.ts` + `src/types/`
- Python Sidecar: `sidecar/main.py` — stdin/stdout JSON line protocol (`{id, action, type, params}`), max 120s timeout per request
- Path alias: `@/` → `src/`
- CSP is strict: only `http://localhost:*` / `http://127.0.0.1:*` allowed for `connect-src`
- i18n: zh-CN default, stored via `localStorage` key `i18n-language`
- Read `docs/` before implementing features (especially `tauri_commands.md`, `handler_development.md`, `database_design.md`)

## Gotchas

- No frontend tests exist. Rust has unit tests — run with `cargo test`.
- `builtin_provider.json` contains API keys and is gitignored.
- All file operations go through Rust Tauri commands, not directly from frontend.
- Provider config includes `contextWindow`, `supportsVision`, `extraParams`.
- Conventional commits with Chinese titles: `feat(范围): 中文标题`

> 如需详细查看引导文件，请阅读根目录下`CLAUDE.md`文件
