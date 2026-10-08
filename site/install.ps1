# Install SDF Flash GUI from the latest GitHub release (ADR 0012).
#
#   irm https://sdf-flash-gui.blahaj.uk/install.ps1 | iex
#
# Downloads the MSI, checks it against the release's SHA256SUMS file, and runs msiexec.
& {
    $ErrorActionPreference = 'Stop'
    $ProgressPreference = 'SilentlyContinue'
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

    $repo = 'thedavidweng/sdf-flash-gui'
    $releases = "https://github.com/$repo/releases/latest"
    $sumsName = 'SHA256SUMS-x86_64-pc-windows-msvc.txt'

    $release = Invoke-RestMethod -UseBasicParsing -Uri "https://api.github.com/repos/$repo/releases/latest"
    $msi = $release.assets | Where-Object { $_.name -like '*.msi' } | Select-Object -First 1
    $sums = $release.assets | Where-Object { $_.name -eq $sumsName } | Select-Object -First 1
    if (-not $msi) { throw "The latest release has no .msi installer. See $releases" }
    if (-not $sums) { throw "The latest release has no $sumsName. See $releases" }

    $dir = Join-Path ([IO.Path]::GetTempPath()) ('sdf-flash-gui-' + [guid]::NewGuid())
    New-Item -ItemType Directory -Path $dir | Out-Null
    try {
        $msiPath = Join-Path $dir $msi.name
        $sumsPath = Join-Path $dir $sumsName
        Write-Host "Downloading $($msi.name)"
        Invoke-WebRequest -UseBasicParsing -Uri $msi.browser_download_url -OutFile $msiPath
        Invoke-WebRequest -UseBasicParsing -Uri $sums.browser_download_url -OutFile $sumsPath

        $line = Get-Content $sumsPath | Where-Object { $_ -match '\.msi\s*$' } | Select-Object -First 1
        if (-not $line) { throw "$sumsName has no checksum for the .msi installer" }
        $want = ($line.Trim() -split '\s+')[0].ToLowerInvariant()
        $got = (Get-FileHash -Algorithm SHA256 -Path $msiPath).Hash.ToLowerInvariant()
        if ($got -ne $want) { throw "Checksum mismatch for $($msi.name)" }

        Write-Host "Installing $($msi.name)"
        $proc = Start-Process -FilePath 'msiexec.exe' -ArgumentList @('/i', "`"$msiPath`"", '/passive', '/norestart') -Wait -PassThru
        if ($proc.ExitCode -notin 0, 3010) { throw "msiexec failed with exit code $($proc.ExitCode)" }
        Write-Host 'Installed SDF Flash GUI.'
        $makemkvDirs = @("${env:ProgramFiles(x86)}\MakeMKV", "$env:ProgramFiles\MakeMKV")
        $hasMakemkv = (Get-Command sdftool, sdftool64, makemkvcon, makemkvcon64 -ErrorAction SilentlyContinue) -or
            ($makemkvDirs | Where-Object { $_ -and (Test-Path $_) })
        if (-not $hasMakemkv) {
            Write-Host 'SDF Flash GUI needs MakeMKV for sdftool and makemkvcon: https://www.makemkv.com/'
        }
    }
    finally {
        Remove-Item -Recurse -Force -Path $dir -ErrorAction SilentlyContinue
    }
}
