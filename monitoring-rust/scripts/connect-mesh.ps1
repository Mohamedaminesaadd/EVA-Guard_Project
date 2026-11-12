
$ErrorActionPreference = "Stop"

$enclave = "eva-guard-net"

Write-Host "EVA-GUARD P2P TOPOLOGY CONFIGURATION" -ForegroundColor Cyan

# 1. Discover the RPC ports of all 8 Geth nodes

$inspection = @(
    wsl.exe -d Ubuntu -- kurtosis enclave inspect $enclave
)

if ($LASTEXITCODE -ne 0) {
    throw "Cannot inspect Kurtosis enclave."
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
        $nodeUrls[$currentNode] =
            "http://127.0.0.1:$($Matches[1])"

        $currentNode = $null
    }
}

if ($nodeUrls.Count -ne 8) {
    throw "Expected 8 Geth RPC endpoints, found $($nodeUrls.Count)."
}

# 2. JSON-RPC helper

function Invoke-GethRpc {
    param(
        [int]$Node,
        [string]$Method,
        [object[]]$Arguments = @()
    )

    $body = @{
        jsonrpc = "2.0"
        method = $Method
        params = $Arguments
        id = 1
    } | ConvertTo-Json -Compress -Depth 8

    $response = Invoke-RestMethod `
        -Uri $nodeUrls[$Node] `
        -Method Post `
        -ContentType "application/json" `
        -Body $body `
        -TimeoutSec 10

    if ($response.error) {
        throw "Node $Node RPC error: $($response.error.message)"
    }

    return $response.result
}

# 3. Retrieve each node's internal P2P enode address

$nodeInfo = @{}

foreach ($i in 1..8) {
    $info = Invoke-GethRpc -Node $i -Method "admin_nodeInfo"

    if (-not $info.enode -or
        $info.enode -notmatch '^enode://[0-9a-fA-F]+@') {
        throw "Invalid enode address for Node $i"
    }

    $nodeInfo[$i] = $info
    Write-Host "Node $i enode discovered"
}

# 4. Create connections around the outer ring
# These supplement the existing connections to Node 1.

$links = @(
    "2,3",
    "3,4",
    "4,5",
    "5,6",
    "6,7",
    "7,8",
    "8,2"
)

foreach ($link in $links) {

    $pair = $link.Split(",")
    $source = [int]$pair[0]
    $destination = [int]$pair[1]

    $accepted = Invoke-GethRpc `
        -Node $source `
        -Method "admin_addPeer" `
        -Arguments @($nodeInfo[$destination].enode)

    if ($accepted -eq $true) {
        Write-Host "Requested connection: Node $source -> Node $destination" -ForegroundColor Green
    }
    else {
        Write-Warning "Connection request not accepted: $source -> $destination"
    }
}

# 5. Verify actual peer connections

Start-Sleep -Seconds 10

Write-Host "`nCURRENT P2P TOPOLOGY" -ForegroundColor Cyan

foreach ($i in 1..8) {

    $peers = @(Invoke-GethRpc -Node $i -Method "admin_peers")
    $peerIds = @($peers | ForEach-Object { $_.id })

    $neighbors = @(
        foreach ($j in 1..8) {
            if ($j -ne $i -and
                $peerIds -contains $nodeInfo[$j].id) {
                $j
            }
        }
    )

    Write-Host "Node $i | $($peers.Count) peers | Neighbors: $($neighbors -join ', ')"
}

Write-Host "`nTopology configuration finished."
