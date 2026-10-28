# . "E:\Study\github program\imoto life\tools\env.ps1"
$T = 'E:\Study\github program\imoto life\tools'
$env:Path = "$T\bin;$T\cmake\bin;$T\ffmpeg\bin;$env:USERPROFILE\.cargo\bin;$env:Path"
$env:RUSTUP_DIST_SERVER = 'https://mirrors.ustc.edu.cn/rust-static'
$env:RUSTUP_UPDATE_ROOT   = 'https://mirrors.ustc.edu.cn/rust-static/rustup'
$env:CARGO_NET_GIT_FETCH_WITH_CLI = 'true'
Write-Host "[env] $(rustc --version)"
