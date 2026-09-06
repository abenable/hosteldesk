$ErrorActionPreference = 'Stop'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$repo = 'abenable/hosteldesk'

try {
    $release = Invoke-RestMethod "https://api.github.com/repos/$repo/releases/latest"
} catch {
    throw "Could not reach GitHub to find the latest release. If you're on a shared " +
        "network, this can be GitHub's unauthenticated API rate limit (60 requests/hour " +
        "per IP address) - wait a few minutes and try again. Underlying error: $_"
}

$asset = $release.assets | Where-Object { $_.name -match '_x64-setup\.exe$' } | Select-Object -First 1
if (-not $asset) {
    throw "No Windows installer found in the latest release of $repo."
}

$checksumAsset = $release.assets | Where-Object { $_.name -eq 'checksums-windows.txt' } | Select-Object -First 1
if (-not $checksumAsset) {
    throw "No checksums-windows.txt found in the latest release of $repo; refusing to " +
        "install an installer that can't be verified."
}

$out = Join-Path $env:TEMP $asset.name
$checksumOut = Join-Path $env:TEMP $checksumAsset.name

Write-Host "Downloading $($asset.name)..."
curl.exe -fsSL -o $out $asset.browser_download_url
if ($LASTEXITCODE -ne 0) { throw "Download failed." }

Write-Host "Downloading checksums..."
curl.exe -fsSL -o $checksumOut $checksumAsset.browser_download_url
if ($LASTEXITCODE -ne 0) { throw "Checksum download failed." }

$expectedLine = Select-String -Path $checksumOut -Pattern ([regex]::Escape($asset.name)) | Select-Object -First 1
if (-not $expectedLine) {
    throw "$($asset.name) is not listed in checksums-windows.txt; refusing to install."
}
$expectedHash = ($expectedLine.Line -split '\s+')[0].ToLowerInvariant()
$actualHash = (Get-FileHash $out -Algorithm SHA256).Hash.ToLowerInvariant()
if ($actualHash -ne $expectedHash) {
    Remove-Item $out -ErrorAction SilentlyContinue
    throw "Checksum mismatch for $($asset.name): expected $expectedHash, got $actualHash. " +
        "The download may be corrupted or tampered with; not installing."
}
Write-Host "Checksum verified: $actualHash"

# Mark-of-the-Web is intentionally left in place: the installer is unsigned,
# and stripping MOTW would silence the SmartScreen warning a user should see
# for an unsigned binary from the internet.

Write-Host 'Installing HostelDesk...'
$proc = Start-Process -FilePath $out -ArgumentList '/S' -Wait -PassThru
if ($proc.ExitCode -ne 0) { throw "Installer exited with code $($proc.ExitCode)." }

Remove-Item $out, $checksumOut -ErrorAction SilentlyContinue
Write-Host 'HostelDesk installed. Launch it from the Start Menu.'
