# WorkMolde AI Python Sidecar 构建脚本
# 下载 Python Embeddable Distribution + 安装依赖 + 复制 sidecar 源码 + 编译 .pyc 隐藏源码
# 最终产物 sidecar_dist/ 通过 tauri.conf.json 的 bundle.resources 打包到 NSIS 安装包
#
# 业务代码保护：除 main.py 外，所有 .py 编译为 .pyc 并删除源文件，避免源码直接暴露
#
# 用法：
#   powershell -ExecutionPolicy Bypass -File scripts/build_sidecar.ps1
#
# 缓存策略：Python Embeddable zip 和 get-pip.py 下载后缓存到 scripts/.cache/，
#   后续构建自动复用缓存，如需强制重新下载请删除 scripts/.cache/ 中的对应文件

# 严格错误模式：任何错误都终止脚本
$ErrorActionPreference = "Stop"

# ============================================
# 配置项
# ============================================

# Python 版本（项目要求 3.12+，3.12.7 是 3.12 系列稳定版本）
$PythonVersion = "3.12.7"
# Python Embeddable 下载 URL（微软官方 FTP）
$PythonDownloadUrl = "https://www.python.org/ftp/python/$PythonVersion/python-$PythonVersion-embed-amd64.zip"
# get-pip.py 下载 URL（官方 bootstrap）
$GetPipUrl = "https://bootstrap.pypa.io/get-pip.py"
# PyPI 镜像源（国内用户加速，清华 TUNA 镜像）
# 如需使用官方源，改为 "https://pypi.org/simple"
$PyMirrorUrl = "https://pypi.tuna.tsinghua.edu.cn/simple"

# 路径配置（基于脚本所在位置推导项目根目录）
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent $ScriptDir
$DistDir = Join-Path $ProjectRoot "sidecar_dist"
$PythonDir = Join-Path $DistDir "python"
$SidecarSourceDir = Join-Path $ProjectRoot "sidecar"
$SidecarTargetDir = Join-Path $DistDir "sidecar"
$CacheDir = Join-Path $ScriptDir ".cache"
$PythonZipPath = Join-Path $CacheDir "python-$PythonVersion-embed-amd64.zip"
$GetPipPath = Join-Path $CacheDir "get-pip.py"
$RequirementsPath = Join-Path $SidecarSourceDir "requirements.txt"

# Python 可执行文件路径
$PythonExe = Join-Path $PythonDir "python.exe"
# python312._pth 文件路径（版本号需与 PythonVersion 主次版本一致）
$PythonMinorVersion = ($PythonVersion -split '\.')[0..1] -join '.'
$PthFile = Join-Path $PythonDir "python$($PythonMinorVersion.Replace('.', ''))._pth"

# ============================================
# 工具函数
# ============================================

function Write-Step {
    # 输出步骤标题（绿色）
    param([string]$Message)
    Write-Host ""
    Write-Host "===========================================" -ForegroundColor Green
    Write-Host "  $Message" -ForegroundColor Green
    Write-Host "===========================================" -ForegroundColor Green
}

function Write-Info {
    # 输出普通信息（白色）
    param([string]$Message)
    Write-Host "[INFO] $Message" -ForegroundColor White
}

function Write-Warn {
    # 输出警告（黄色）
    param([string]$Message)
    Write-Host "[WARN] $Message" -ForegroundColor Yellow
}

function Test-CommandSuccess {
    # 检查上次命令退出码，失败则终止脚本
    param([string]$Context)
    if ($LASTEXITCODE -ne 0) {
        Write-Host "[ERROR] $Context 失败，退出码: $LASTEXITCODE" -ForegroundColor Red
        exit 1
    }
}

function Get-DirSizeMB {
    # 计算目录体积（MB）
    param([string]$Path)
    if (-not (Test-Path $Path)) { return 0 }
    $size = (Get-ChildItem $Path -Recurse -File -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum
    return [math]::Round($size / 1MB, 2)
}

# ============================================
# 步骤 1：环境检查
# ============================================

Write-Step "步骤 1/9：环境检查"

# 镜像源连通性检测：如果镜像不可达（如 CI 环境被屏蔽），回退到官方 PyPI
try {
    $req = [System.Net.HttpWebRequest]::Create("$PyMirrorUrl/pip/")
    $req.Timeout = 5000
    $req.Method = "HEAD"
    $req.GetResponse().Close()
    Write-Info "镜像源可达: $PyMirrorUrl"
} catch {
    Write-Warn "镜像源不可达 ($($_.Exception.Message))，回退到官方 PyPI"
    $PyMirrorUrl = "https://pypi.org/simple"
}

# 检查 sidecar 源码目录
if (-not (Test-Path $SidecarSourceDir)) {
    Write-Host "[ERROR] sidecar 源码目录不存在: $SidecarSourceDir" -ForegroundColor Red
    exit 1
}
Write-Info "sidecar 源码目录: $SidecarSourceDir"

# 检查 requirements.txt
if (-not (Test-Path $RequirementsPath)) {
    Write-Host "[ERROR] requirements.txt 不存在: $RequirementsPath" -ForegroundColor Red
    exit 1
}
Write-Info "依赖清单: $RequirementsPath"

# 创建缓存目录
if (-not (Test-Path $CacheDir)) {
    New-Item -ItemType Directory -Path $CacheDir -Force | Out-Null
    Write-Info "创建缓存目录: $CacheDir"
}

# ============================================
# 步骤 2：清理旧的构建产物
# ============================================

Write-Step "步骤 2/9：清理旧的 sidecar_dist/"

if (Test-Path $DistDir) {
    Write-Info "删除旧的 $DistDir"
    Remove-Item -Path $DistDir -Recurse -Force
}

New-Item -ItemType Directory -Path $DistDir -Force | Out-Null
Write-Info "创建 $DistDir"

# ============================================
# 步骤 3：下载 Python Embeddable Distribution
# ============================================

Write-Step "步骤 3/9：下载 Python $PythonVersion Embeddable"

# 缓存策略：如果 zip 已存在则复用，避免重复下载
# 如需强制重新下载，请删除 scripts/.cache/ 中的缓存文件
$NeedDownload = $true
if (Test-Path $PythonZipPath) {
    Write-Info "缓存已存在，跳过下载: $PythonZipPath"
    Write-Info "如需强制重新下载，请先删除该缓存文件"
    $NeedDownload = $false
}

if ($NeedDownload) {
    Write-Info "下载: $PythonDownloadUrl"
    Write-Info "目标: $PythonZipPath"
    try {
        # 使用 .NET HttpClient 下载，支持大文件和进度
        $ProgressPreference = 'Continue'
        Invoke-WebRequest -Uri $PythonDownloadUrl -OutFile $PythonZipPath -UseBasicParsing
        $ProgressPreference = 'SilentlyContinue'
    } catch {
        Write-Host "[ERROR] 下载失败: $_" -ForegroundColor Red
        exit 1
    }
    if (-not (Test-Path $PythonZipPath)) {
        Write-Host "[ERROR] 下载后文件不存在: $PythonZipPath" -ForegroundColor Red
        exit 1
    }
    $zipSize = [math]::Round((Get-Item $PythonZipPath).Length / 1MB, 2)
    Write-Info "下载完成，体积: $zipSize MB"
}

# ============================================
# 步骤 4：解压 Python Embeddable
# ============================================

Write-Step "步骤 4/9：解压 Python Embeddable"

Write-Info "解压到: $PythonDir"
# 使用 Expand-Archive 解压（PowerShell 内置）
Expand-Archive -Path $PythonZipPath -DestinationPath $PythonDir -Force

# 验证 python.exe 存在
if (-not (Test-Path $PythonExe)) {
    Write-Host "[ERROR] 解压后未找到 python.exe: $PythonExe" -ForegroundColor Red
    exit 1
}
Write-Info "Python 解释器: $PythonExe"

# 验证版本
$pythonVersionOutput = & $PythonExe --version 2>&1
Write-Info "Python 版本: $pythonVersionOutput"

# ============================================
# 步骤 5：修改 python312._pth 启用 site-packages
# ============================================

Write-Step "步骤 5/9：配置 python._pth 启用 site-packages"

# _pth 文件名格式：python312._pth（去掉点的主次版本号）
if (-not (Test-Path $PthFile)) {
    Write-Host "[ERROR] _pth 文件不存在: $PthFile" -ForegroundColor Red
    Write-Info "实际文件列表:"
    Get-ChildItem $PythonDir -Filter "python*._pth" | ForEach-Object { Write-Info "  $($_.Name)" }
    exit 1
}

Write-Info "_pth 文件: $PthFile"
$pthContent = Get-Content $PthFile -Raw
Write-Info "原内容:"
Write-Host $pthContent

# 取消注释 #import site（启用 site-packages 自动加载）
# Python Embeddable 默认禁用 site，导致 pip 安装的第三方库无法 import
# 启用方式：取消注释 "import site" 行（去掉行首的 "# "）
$newPthContent = $pthContent -replace '(?m)^#\s*import\s+site\s*$', 'import site'
$pthChanged = ($newPthContent -ne $pthContent)
if ($pthChanged) {
    Write-Info "已取消注释 'import site'"
} elseif ($pthContent -match '(?m)^import\s+site\s*$') {
    Write-Info "site 已启用（无需修改）"
} else {
    Write-Warn "未找到 '#import site' 行，尝试追加 'import site'"
    $newPthContent = $pthContent.TrimEnd() + "`r`nimport site`r`n"
}

Set-Content -Path $PthFile -Value $newPthContent -NoNewline:$false
Write-Info "新内容:"
Write-Host $newPthContent

# ============================================
# 步骤 6：安装 pip 并安装依赖
# ============================================

Write-Step "步骤 6/9：安装 pip 与第三方依赖"

# 下载 get-pip.py（如未缓存）
if (-not (Test-Path $GetPipPath)) {
    Write-Info "下载 get-pip.py: $GetPipUrl"
    try {
        Invoke-WebRequest -Uri $GetPipUrl -OutFile $GetPipPath -UseBasicParsing
    } catch {
        Write-Host "[ERROR] 下载 get-pip.py 失败: $_" -ForegroundColor Red
        exit 1
    }
} else {
    Write-Info "使用缓存的 get-pip.py: $GetPipPath"
}

# 定义安装函数，支持镜像源不可用时自动回退到官方 PyPI
function Install-WithFallback {
    param([ScriptBlock]$ScriptBlock)
    # 尝试使用镜像源安装
    $originalUrl = $PyMirrorUrl
    try {
        & $ScriptBlock
        if ($LASTEXITCODE -ne 0) { throw "退出码: $LASTEXITCODE" }
    } catch {
        if ($PyMirrorUrl -ne "https://pypi.org/simple") {
            Write-Warn "镜像源安装失败 ($($_.Exception.Message))，回退到官方 PyPI"
            $script:PyMirrorUrl = "https://pypi.org/simple"
            & $ScriptBlock
            Test-CommandSuccess "安装（官方源）"
            $script:PyMirrorUrl = $originalUrl
        } else {
            throw
        }
    }
}

# 使用嵌入式 Python 执行 get-pip.py 安装 pip
# 通过 --index-url 指定镜像源加速 pip 自身下载
Write-Info "安装 pip..."
Install-WithFallback -ScriptBlock {
    & $PythonExe $GetPipPath --no-warn-script-location --index-url $PyMirrorUrl
}
Test-CommandSuccess "安装 pip"

# 验证 pip 可用
Write-Info "验证 pip..."
& $PythonExe -m pip --version
Test-CommandSuccess "验证 pip"

# 安装 requirements.txt 中的依赖
# 通过 -i 指定镜像源加速依赖下载
Write-Info "安装依赖: $RequirementsPath"
Write-Info "使用镜像源: $PyMirrorUrl"
Install-WithFallback -ScriptBlock {
    & $PythonExe -m pip install -r $RequirementsPath --no-warn-script-location -i $PyMirrorUrl
}
Test-CommandSuccess "安装 Python 依赖"

# ============================================
# 步骤 7：复制 sidecar 源码
# ============================================

Write-Step "步骤 7/9：复制 sidecar 源码到 sidecar_dist/sidecar/"

Write-Info "源: $SidecarSourceDir"
Write-Info "目标: $SidecarTargetDir"

# 使用 robocopy 复制（自动排除缓存目录和测试目录，比 Copy-Item 更高效）
# /E 复制子目录（包括空目录）
# /XD 排除目录（__pycache__/.pytest_cache/.cache 缓存；tests 测试代码不应打包）
# /XF 排除文件（.pyc/.pyo 缓存；requirements.txt 是构建时依赖清单，安装目录无需保留）
# /NFL /NDL 不显示文件/目录名（减少输出）
# /NJH /NJS 不显示作业头/摘要
# /NP 不显示进度百分比
$robocopyArgs = @(
    $SidecarSourceDir,
    $SidecarTargetDir,
    "/E",
    "/XD", "__pycache__", ".pytest_cache", ".cache", "tests",
    "/XF", "*.pyc", "*.pyo", "requirements.txt",
    "/NFL", "/NDL", "/NJH", "/NJS", "/NP"
)
& robocopy @robocopyArgs

# robocopy 退出码 0-7 都是成功的（<8 表示成功，>=8 表示失败）
# 0: 无变化 1: 复制成功 2: 额外文件 3: 1+2 4: 不匹配 5-7: 组合
if ($LASTEXITCODE -ge 8) {
    Write-Host "[ERROR] robocopy 失败，退出码: $LASTEXITCODE" -ForegroundColor Red
    exit 1
}
# 重置 $LASTEXITCODE（robocopy 非零退出码可能影响后续判断）
$global:LASTEXITCODE = 0

Write-Info "sidecar 源码复制完成"

# 验证 main.py 存在
$MainPy = Join-Path $SidecarTargetDir "main.py"
if (-not (Test-Path $MainPy)) {
    Write-Host "[ERROR] 复制后未找到 main.py: $MainPy" -ForegroundColor Red
    exit 1
}

# ============================================
# 步骤 8：编译业务 .py 为 .pyc 并删除 .py 源文件
# ============================================

Write-Step "步骤 8/9：编译业务代码为 .pyc（隐藏源码）"

# 使用 compileall 编译所有 .py 为 .pyc
# -b 参数：将 .pyc 文件放在与 .py 同级的目录（而不是 __pycache__ 子目录）
# 这样删除 .py 后，Python 会直接加载同级的 .pyc（Python 2 遗留行为，3.x 仍支持）
# -q 参数：静默模式，只输出错误
Write-Info "编译 .py 为 .pyc..."
& $PythonExe -m compileall -b -q $SidecarTargetDir
Test-CommandSuccess "编译 .pyc"

# 统计编译生成的 .pyc 文件数量
$pycCount = (Get-ChildItem -Path $SidecarTargetDir -Recurse -Filter "*.pyc" -File).Count
Write-Info "已生成 $pycCount 个 .pyc 文件"

# 删除 .py 源文件，保留 main.py（入口）和 __init__.py（Python 3 包初始化必需）
# main.py：入口文件，python.exe main.py 需要它，只是简单调度逻辑
# __init__.py：Python 3 识别普通包的必需文件，只有 __init__.pyc 无法被识别为包
#   - handlers/__init__.py：只有一行注释
# 核心业务逻辑都在 handlers/*.pyc 中（字节码，无法用记事本查看）
Write-Info "删除业务 .py 源文件（保留 main.py 和 __init__.py）..."
$deletedCount = 0
Get-ChildItem -Path $SidecarTargetDir -Recurse -Filter "*.py" -File | Where-Object { $_.Name -notin @("main.py", "__init__.py") } | ForEach-Object {
    Remove-Item -Path $_.FullName -Force
    $deletedCount++
}
Write-Info "已删除 $deletedCount 个 .py 文件（保留 main.py 和 __init__.py）"

# 清理 compileall 可能生成的 __pycache__ 目录（使用 -b 后 .pyc 已在同级，__pycache__ 为空或冗余）
$pycacheDir = Join-Path $SidecarTargetDir "__pycache__"
if (Test-Path $pycacheDir) {
    Remove-Item -Path $pycacheDir -Recurse -Force
}
# 递归清理子目录下的 __pycache__
Get-ChildItem -Path $SidecarTargetDir -Recurse -Directory -Filter "__pycache__" | ForEach-Object {
    Remove-Item -Path $_.FullName -Recurse -Force
}
Write-Info "已清理 __pycache__ 目录"

# 验证 .pyc 文件结构（列出关键文件）
Write-Info "业务代码文件结构:"
Get-ChildItem -Path $SidecarTargetDir -Recurse -File | Where-Object { $_.Extension -in ".py", ".pyc" } | ForEach-Object {
    $relPath = $_.FullName.Replace($SidecarTargetDir, "").TrimStart("\")
    Write-Info "  $relPath"
}

# ============================================
# 步骤 9：验证 sidecar 可启动
# ============================================

Write-Step "步骤 9/9：验证 sidecar 通信"

# 启动 sidecar 进程，发送 ping 请求验证通信
Write-Info "启动 sidecar 进程进行验证..."
$process = New-Object System.Diagnostics.Process
$process.StartInfo.FileName = $PythonExe
$process.StartInfo.Arguments = "`"$MainPy`""
$process.StartInfo.UseShellExecute = $false
$process.StartInfo.RedirectStandardInput = $true
$process.StartInfo.RedirectStandardOutput = $true
$process.StartInfo.RedirectStandardError = $true
$process.StartInfo.CreateNoWindow = $true

try {
    $process.Start() | Out-Null
} catch {
    Write-Host "[ERROR] 启动 sidecar 进程失败: $_" -ForegroundColor Red
    exit 1
}

# 发送 ping 请求
$pingRequest = '{"id":"buildtest","action":"ping","type":"health"}'
Write-Info "发送 ping 请求: $pingRequest"
$process.StandardInput.WriteLine($pingRequest)
$process.StandardInput.Flush()

# 读取响应（5 秒超时）
$process.WaitForExit(5000) | Out-Null
$response = $null
if (-not $process.HasExited) {
    # 尝试读取响应行（带超时）
    $readTask = $process.StandardOutput.ReadLineAsync()
    $readTask.Wait(5000) | Out-Null
    if ($readTask.IsCompleted) {
        $response = $readTask.Result
    }
}

# 终止 sidecar 进程
# 必须等待进程完全退出后再清理日志文件，否则文件句柄未释放会导致清理失败
if (-not $process.HasExited) {
    try {
        $process.Kill()
        # WaitForExit 等待进程完全退出并释放文件句柄（超时 3 秒保护）
        $process.WaitForExit(3000) | Out-Null
    } catch { }
}

if ($null -eq $response) {
    Write-Host "[ERROR] 未收到 sidecar 响应（超时）" -ForegroundColor Red
    Write-Info "stderr 输出:"
    $stderr = $process.StandardError.ReadToEnd()
    Write-Host $stderr
    exit 1
}

Write-Info "响应: $response"

# 验证响应包含 success: true
if ($response -match '"success"\s*:\s*true') {
    Write-Info "sidecar 通信验证通过"
} else {
    Write-Host "[ERROR] sidecar 响应异常: $response" -ForegroundColor Red
    exit 1
}

# 清理验证通信时 sidecar 进程在 sidecar_dist/log/ 下生成的日志文件
# 这些日志是构建验证产物，不应打包到安装包中
# （生产环境运行时，sidecar 日志由 Rust 端通过 WORKMOLDE_LOG_DIR 环境变量
#   指向 %LOCALAPPDATA%\workmolde\logs\，不会写到安装目录）
$buildLogDir = Join-Path $DistDir "log"
if (Test-Path $buildLogDir) {
    Write-Info "清理构建验证日志目录: $buildLogDir"
    # 重试机制：sidecar 进程的 FileHandler 可能延迟释放文件句柄
    # 等待 500ms 后重试，最多 3 次
    $removed = $false
    for ($i = 1; $i -le 3; $i++) {
        try {
            Remove-Item -Path $buildLogDir -Recurse -Force -ErrorAction Stop
            $removed = $true
            break
        } catch {
            if ($i -lt 3) {
                Write-Info "清理日志目录失败（第 $i 次），等待 500ms 后重试: $_"
                Start-Sleep -Milliseconds 500
            }
        }
    }
    if (-not $removed) {
        Write-Warn "清理日志目录失败（已重试 3 次），跳过清理: $buildLogDir"
        Write-Warn "下次构建步骤 2 会清理整个 sidecar_dist 目录，不影响构建正确性"
    }
}

# 清理验证通信时 Python 加载 __init__.py 自动生成的 __pycache__ 目录
# （步骤 8 已清理过 compileall 产生的 __pycache__，但步骤 9 启动 sidecar 验证时
#   Python 会重新生成 handlers/__pycache__/__init__.cpython-312.pyc）
$pycacheDirs = Get-ChildItem -Path $SidecarTargetDir -Recurse -Directory -Filter "__pycache__" -ErrorAction SilentlyContinue
if ($pycacheDirs) {
    Write-Info "清理验证产生的 __pycache__ 目录..."
    $pycacheDirs | ForEach-Object {
        Remove-Item -Path $_.FullName -Recurse -Force
    }
}

# ============================================
# 构建结果统计
# ============================================

Write-Step "构建完成"

$distSize = Get-DirSizeMB -Path $DistDir
$pythonSize = Get-DirSizeMB -Path $PythonDir
$sidecarSize = Get-DirSizeMB -Path $SidecarTargetDir

Write-Info "sidecar_dist 总体积: $distSize MB"
Write-Info "  - python/ (解释器+依赖): $pythonSize MB"
Write-Info "  - sidecar/ (业务代码): $sidecarSize MB"
Write-Info "产物路径: $DistDir"
Write-Host ""
Write-Host "下一步：运行 'npm run tauri:build' 构建 NSIS 安装包" -ForegroundColor Cyan
