; WorkMolde AI NSIS 安装器 Hook 脚本
; 通过 Tauri NsisConfig.installerHooks 引入，提供安装/卸载时的自定义逻辑
;
; 支持的 Hook 点（参见 tauri-utils config.rs NsisConfig.installer_hooks 文档）：
;   NSIS_HOOK_PREINSTALL    - 安装前（复制文件、写注册表、创建快捷方式之前）
;   NSIS_HOOK_POSTINSTALL   - 安装后（所有文件、注册表、快捷方式都已就绪）
;   NSIS_HOOK_PREUNINSTALL  - 卸载前（删除文件、注册表、快捷方式之前）
;   NSIS_HOOK_POSTUNINSTALL - 卸载后（所有文件、注册表、快捷方式已删除）
;
; 本脚本用途：在卸载前清理 sidecar 运行时产生的文件和目录
; 原因：NSIS 默认只删除安装时记录的文件，运行时 Python 自动生成的
;       __pycache__ 目录和 code_audit.log 不会被删除，导致安装目录残留

!macro NSIS_HOOK_PREUNINSTALL
  ; 清理 sidecar 运行时产生的 __pycache__ 目录（Python 加载 .py 自动生成）
  ; 这些目录不是安装时打包的，NSIS 默认卸载逻辑不会删除
  RMDir /r "$INSTDIR\sidecar_dist\sidecar\handlers\__pycache__"
  RMDir /r "$INSTDIR\sidecar_dist\sidecar\handlers\doc_helpers\__pycache__"
  RMDir /r "$INSTDIR\sidecar_dist\sidecar\__pycache__"

  ; 清理 sidecar 运行时可能产生的日志目录
  ; （注意：日志文件写到 WORKMOLDE_LOG_DIR，但防止异常情况下安装目录残留）
  RMDir /r "$INSTDIR\sidecar_dist\sidecar\log"
  RMDir /r "$INSTDIR\sidecar_dist\log"
!macroend
