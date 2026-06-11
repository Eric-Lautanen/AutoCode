param(
    [Parameter(Position=0, Mandatory=$true)]
    [string]$Path,

    [switch]$DryRun
)

$repaired = 0
$skipped = 0
$kept = 0
$result = [System.Collections.Generic.List[string]]::new()

$lines = Get-Content -LiteralPath $Path

if ($lines.Count -eq 0) {
    Write-Host "Empty file: $Path"
    exit 0
}

for ($i = 0; $i -lt $lines.Count; $i++) {
    $line = $lines[$i].Trim()
    if ($line.Length -eq 0) {
        continue
    }

    try {
        $null = $line | ConvertFrom-Json -ErrorAction Stop
        $result.Add($line)
        $kept++
    } catch {
        if ($i -eq $lines.Count - 1) {
            Write-Host "  Truncated last line - removing (likely crash during append)"
            $repaired++
        } else {
            Write-Host "  Bad line $($i+1): $($_.Exception.Message)"
            $skipped++
        }
    }
}

Write-Host "File: $Path"
Write-Host "  Valid lines: $kept"
Write-Host "  Repaired (truncated last line): $repaired"
Write-Host "  Skipped: $skipped"

if ($repaired -gt 0 -or $skipped -gt 0) {
    Write-Host "  Issues found!"
}

if (($repaired -gt 0 -or $skipped -gt 0) -and !$DryRun) {
    $result -join "`r`n" | Set-Content -LiteralPath $Path -NoNewline
    Write-Host "  Written fixed file."
}

if ($DryRun) {
    Write-Host "  (dry run - no changes written)"
}
