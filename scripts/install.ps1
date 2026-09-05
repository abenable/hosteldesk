$ErrorActionPreference = 'Stop'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$repo = 'abenable/hosteldesk'
$release = Invoke-RestMethod "https://api.github.com/repos/$repo/releases/latest"
$asset = $release.assets | Where-Object { $_.name -match '_x64-setup\.exe$' } | Select-Object -First 1

if (-not $asset) {
    throw "No Windows installer found in the latest release of $repo."
}

$out = Join-Path $env:TEMP $asset.name
Write-Host "Downloading $($asset.name)..."
curl.exe -sSL -o $out $asset.browser_download_url
if ($LASTEXITCODE -ne 0) { throw "Download failed." }

Unblock-File $out
Write-Host ("SHA256: {0}" -f (Get-FileHash $out -Algorithm SHA256).Hash)

Write-Host 'Installing HostelDesk...'
$proc = Start-Process -FilePath $out -ArgumentList '/S' -Wait -PassThru
if ($proc.ExitCode -ne 0) { throw "Installer exited with code $($proc.ExitCode)." }

Remove-Item $out -ErrorAction SilentlyContinue
Write-Host 'HostelDesk installed. Launch it from the Start Menu.'
