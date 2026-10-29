# Package both Windows release builds into Desktop folders + zips.
# Each build ships the Mesa software-GL opengl32.dll beside DungeonMind.exe
# (needed on VMs/GPU-less hosts; harmless elsewhere because macroquad/miniquad
# only loads it when no usable hardware ICD answers WGL — in practice the
# loader prefers the copy next to the exe, and the game falls back to software
# rendering, which is what QA on this VirtIO-GPU VM requires).
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$desk = [Environment]::GetFolderPath('Desktop')

foreach ($t in @(
    @{ target = 'aarch64-pc-windows-msvc'; out = 'arm64'; mesa = 'arm64' },
    @{ target = 'x86_64-pc-windows-msvc';  out = 'x64';  mesa = 'x64'  }
)) {
    $dst = Join-Path $desk ("DungeonMind\{0}" -f $t.out)
    New-Item -ItemType Directory -Force -Path $dst | Out-Null
    Copy-Item -Force "$root\target\$($t.target)\release\dungeonmind.exe" "$dst\DungeonMind.exe"
    Copy-Item -Force "$root\qa\$($t.mesa)\opengl32.dll" "$dst\opengl32.dll"
    Copy-Item -Force "$root\README.md" "$dst\README.md"
    $zip = Join-Path $desk ("DungeonMind-{0}.zip" -f $t.out)
    if (Test-Path $zip) { Remove-Item -Force $zip }
    Compress-Archive -Path "$dst\*" -DestinationPath $zip
    Get-ChildItem $dst | ForEach-Object { "{0}  {1:N0} bytes" -f $_.FullName, $_.Length }
    "ZIP: $zip  {0:N0} bytes" -f (Get-Item $zip).Length
}
