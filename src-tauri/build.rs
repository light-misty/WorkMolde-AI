/// 构建脚本：检测内置 Provider 配置文件是否存在
/// 如果项目根目录下存在 builtin_provider.json，则启用 builtin_provider cfg 标志
fn main() {
    // 声明 builtin_provider 为合法的 cfg 条件，消除 unexpected_cfgs 警告
    println!("cargo::rustc-check-cfg=cfg(builtin_provider)");

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let project_root = std::path::Path::new(&manifest_dir)
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let builtin_config = project_root.join("builtin_provider.json");

    if builtin_config.exists() {
        println!("cargo:rustc-cfg=builtin_provider");
        // 信息性输出（不使用 cargo:warning 以避免被误判为编译警告）
        println!("builtin_provider.json detected, builtin_provider cfg enabled");
    }

    // Windows: 嵌入 ComCtl32 v6 清单依赖
    // tauri 依赖链中的 windows crate 导入了 TaskDialogIndirect 等 ComCtl32 v6 函数，
    // 但测试 exe 默认加载 ComCtl32 v5（不含这些函数），导致 STATUS_ENTRYPOINT_NOT_FOUND。
    // 通过 /MANIFESTINPUT + /MANIFEST:EMBED 嵌入清单文件，请求加载 v6。
    //
    // 注意：cargo:rustc-link-arg-tests 仅应用于集成测试(tests/)目标，不应用于 lib 测试目标，
    // 因此必须使用 cargo:rustc-link-arg（所有目标）来覆盖 lib 测试。
    //
    // bin 目标由 tauri-build 的 resource.lib 已提供清单（含 ComCtl32 v6 依赖），
    // 若再嵌入会导致 CVT1100（重复 MANIFEST 资源）。
    // 解决方案：对 bin 目标额外传递 /MANIFEST:NO，覆盖 /MANIFEST:EMBED，
    // resource.lib 中的清单资源仍会被链接（/MANIFEST:NO 仅阻止链接器生成清单，不影响资源链接）。
    #[cfg(target_os = "windows")]
    {
        let manifest = std::path::Path::new(&manifest_dir).join("comctl32_v6.manifest");
        if manifest.exists() {
            // 所有目标：嵌入清单
            println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
            println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
            // bin 目标：禁用清单生成（resource.lib 已提供清单资源）
            println!("cargo:rustc-link-arg-bins=/MANIFEST:NO");
            println!("cargo:rerun-if-changed=comctl32_v6.manifest");
        }
    }

    // tauri-build 需要在 build.rs 中调用
    tauri_build::build();
}
