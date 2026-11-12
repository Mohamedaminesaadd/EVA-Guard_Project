
param(
    [int]$IntervalSeconds = 5
)

Write-Host "EVA-GUARD COLLECTOR STARTED" -ForegroundColor Green

$ErrorActionPreference = "Stop"
$enclave = "eva-guard-net"

# ========================================
# 1. Find the 8 Ethereum RPC endpoints
# ========================================

Write-Host "Searching for Ethereum nodes..."

$inspection = @(
    wsl.exe -d Ubuntu -- kurtosis enclave inspect $enclave
)

if ($LASTEXITCODE -ne 0) {
    throw "Failed to inspect Kurtosis network."
}

$nodeUrls = @{}
$currentNode = $null

foreach ($line in $inspection) {

    if ($line -match '\bel-(\d+)-geth-lodestar\b') {
        $currentNode = [int]$Matches[1]
    }

    if (
        $null -ne $currentNode -and
        $line -match '^\s*rpc:\s*8545/tcp\s*->\s*(?:https?://)?127\.0\.0\.1:(\d+)'
    ) {
        $port = $Matches[1]
        $nodeUrls[$currentNode] = "http://127.0.0.1:$port"
        $currentNode = $null
    }
}

Write-Host "Found $($nodeUrls.Count) Ethereum nodes."

foreach ($id in ($nodeUrls.Keys | Sort-Object)) {
    Write-Host "Node $id : $($nodeUrls[$id])"
}

if ($nodeUrls.Count -ne 8) {
    throw "Expected 8 nodes but found $($nodeUrls.Count)."
}

# ========================================
# 2. Ethereum JSON-RPC function
# ========================================

function Invoke-EthRpc($url, $method) {

    $body = @{
        jsonrpc = "2.0"
        method  = $method
        params  = @()
        id      = 1
    } | ConvertTo-Json -Compress

    $response = Invoke-RestMethod `
        -Uri $url `
        -Method Post `
        -ContentType "application/json" `
        -Body $body `
        -TimeoutSec 5

    if ($response.error) {
        throw $response.error.message
    }

    return $response.result
}

# Convert hexadecimal Ethereum numbers
function Convert-HexNumber($value) {
    return [Convert]::ToInt64(
        $value.Substring(2), 16
    )
}

# ========================================
# 3. Prepare the CSV dataset
# ========================================

$projectRoot = Split-Path -Parent $PSScriptRoot
$dataDir = Join-Path $projectRoot "data"

New-Item -ItemType Directory `
    -Force -Path $dataDir | Out-Null

$runId = Get-Date -Format "yyyyMMdd_HHmmss"
$output = Join-Path $dataDir "ethereum_baseline_$runId.csv"

Write-Host ""
Write-Host "Saving metrics to: $output"
Write-Host "Press Ctrl+C to stop."
Write-Host ""

# ========================================
# 4. Collect metrics continuously
# ========================================

while ($true) {

    foreach ($nodeId in ($nodeUrls.Keys | Sort-Object)) {

        $url = $nodeUrls[$nodeId]

        try {

            # Get Ethereum metrics
            $peers = Invoke-EthRpc $url "net_peerCount"
            $block = Invoke-EthRpc $url "eth_blockNumber"
            $sync = Invoke-EthRpc $url "eth_syncing"

            # Convert values
            $peerCount = Convert-HexNumber $peers
            $blockNumber = Convert-HexNumber $block

            $isSyncing = -not (
                $sync -is [bool] -and $sync -eq $false
            )

            # Create one dataset row
            $record = [PSCustomObject]@{
                timestamp    = (Get-Date).ToUniversalTime().ToString("o")
                run_id       = $runId
                node_id      = "node$nodeId"
                total_nodes  = 8
                peer_count   = $peerCount
                block_number = $blockNumber
                syncing      = $isSyncing
                scenario     = "normal_candidate"
            }

            # Save to CSV
            $record | Export-Csv `
                -Path $output `
                -NoTypeInformation `
                -Append `
                -Encoding UTF8

            # Display live metrics
            Write-Host "Node $nodeId | Peers: $peerCount | Block: $blockNumber | Syncing: $isSyncing"

        }
        catch {
            Write-Warning "Node $nodeId ERROR: $($_.Exception.Message)"
        }
    }

    Write-Host "-------------------------------"

    Start-Sleep -Seconds $IntervalSeconds
}
